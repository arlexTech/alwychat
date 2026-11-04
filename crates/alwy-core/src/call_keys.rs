//! End-to-end encryption of call media, as Element Call does it (MSC4143 per-participant keys,
//! also what FluffyChat runs): every device in a call encrypts its audio/video frames with its own
//! random key, and sends that key to each other device in the call as an Olm-encrypted to-device
//! message (`io.element.call.encryption_keys`). LiveKit (the SFU) only ever sees encrypted frames.
//!
//! - We encrypt our own media in encrypted rooms (Element's rule: in unencrypted rooms Element
//!   Call doesn't, and couldn't read ours), but we always decrypt anyone who sends us a key.
//! - When someone leaves, we roll a new key (index + 1) so they can't follow what comes after,
//!   send it, and switch to it a second later (time for the others to receive it). Someone who
//!   joins gets the current key.
//! - The media side (webview `livekit-client`, or the native engine on Linux) gets keys through
//!   [`Core::on_call_key`] / [`Core::call_keys`]; each key comes with the LiveKit identities it
//!   applies to (`@user:server:DEVICE`, and the hashed one newer Element Calls use).

use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, Mutex},
    time::Duration,
};

use base64::{
    Engine,
    engine::{
        DecodePaddingMode,
        general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD},
    },
};
use matrix_sdk::{
    Client, Room,
    deserialized_responses::{EncryptionInfo, RawAnySyncOrStrippedState},
    ruma::{
        MilliSecondsSinceUnixEpoch, OwnedDeviceId, OwnedRoomId, OwnedUserId,
        events::{AnySyncStateEvent, AnyToDeviceEvent, AnyToDeviceEventContent, StateEventType},
        serde::Raw,
    },
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::rtc::CALL_MEMBER;

/// The to-device event type carrying a media key (unstable name, as Element Call sends it).
const KEYS_EVENT: &str = "io.element.call.encryption_keys";
/// How long the others get to receive a new key before we encrypt with it.
const USE_KEY_DELAY: Duration = Duration::from_secs(1);
/// Membership changes wake the key task; this catches expiries and failed sends.
const CHECK_EVERY: Duration = Duration::from_secs(5);

/// A media key for the call's media layer.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CallKey {
    pub room_id: String,
    /// The LiveKit participant identities it decrypts (or, for ours, encrypts).
    pub identities: Vec<String>,
    /// Key slot (0–255): a new key takes the next one, so both work during the switch.
    pub index: u8,
    /// The raw key, base64.
    pub key: String,
    /// Our own key (to encrypt with).
    pub own: bool,
}

pub(crate) type CallKeyHandler = Box<dyn Fn(CallKey) + Send + Sync>;

#[derive(Default)]
struct Keys {
    /// Received keys by room, then (first identity, index): a newer key replaces an older one.
    received: HashMap<OwnedRoomId, HashMap<(String, u8), CallKey>>,
    /// Ours, for the call we're in.
    own: Option<CallKey>,
}

static KEYS: LazyLock<Mutex<Keys>> = LazyLock::new(Default::default);
static HANDLER: LazyLock<Mutex<Option<CallKeyHandler>>> = LazyLock::new(Default::default);
static MEMBERS_CHANGED: LazyLock<tokio::sync::Notify> = LazyLock::new(tokio::sync::Notify::new);

fn emit(key: CallKey) {
    if let Some(handler) = HANDLER.lock().unwrap().as_ref() {
        handler(key);
    }
}

/// Base64 as Element sends it (padded) or as the MSC says (unpadded): both are read.
const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// The LiveKit identities of a call member: `user:device` (what the homeserver's LiveKit token
/// service gives with the `livekit_service_url` transport, ours), and the pseudonymous one of
/// newer Element Calls, unpadded base64 of SHA-256 over the JSON array `[user, member id]`.
pub(crate) fn identities(user: &str, device: &str, member_id: Option<&str>) -> Vec<String> {
    let legacy = format!("{user}:{device}");
    let member_id = member_id.unwrap_or(&legacy);
    let json = serde_json::to_string(&[user, member_id]).unwrap_or_default();
    let hashed = base64::engine::general_purpose::STANDARD_NO_PAD.encode(Sha256::digest(json));
    vec![legacy, hashed]
}

#[derive(Deserialize)]
struct MediaKey {
    index: u32,
    key: String,
}

#[derive(Deserialize)]
struct KeyMember {
    #[serde(default)]
    id: Option<String>,
    claimed_device_id: String,
}

#[derive(Deserialize)]
struct KeysContent {
    room_id: OwnedRoomId,
    #[serde(default)]
    member_id: Option<String>,
    #[serde(default)]
    media_key: Option<MediaKey>,
    /// Older name of `media_key`.
    #[serde(default)]
    keys: Option<MediaKey>,
    member: KeyMember,
}

/// Follow incoming keys and call membership changes (once per signed-in client).
pub(crate) fn install(client: &Client) {
    client.add_event_handler(
        |event: Raw<AnyToDeviceEvent>, encryption: Option<EncryptionInfo>| async move {
            if event.get_field::<String>("type").ok().flatten().as_deref() != Some(KEYS_EVENT) {
                return;
            }
            // A key sent in clear could come from anyone (Element Call ignores those too).
            let Some(encryption) = encryption else {
                warn!("call key sent unencrypted, ignored");
                return;
            };
            let Ok(Some(content)) = event.get_field::<KeysContent>("content") else {
                warn!("malformed call key");
                return;
            };
            receive(encryption, content);
        },
    );
    client.add_event_handler(|event: Raw<AnySyncStateEvent>| async move {
        if event.get_field::<String>("type").ok().flatten().as_deref() == Some(CALL_MEMBER) {
            MEMBERS_CHANGED.notify_waiters();
        }
    });
}

fn receive(encryption: EncryptionInfo, content: KeysContent) {
    let Some(key) = parse(
        encryption.sender.as_str(),
        encryption.sender_device.as_ref().map(|d| d.as_str()),
        content,
    ) else {
        return;
    };
    info!(sender = %encryption.sender, index = key.index, "call key received");
    let Ok(room_id) = OwnedRoomId::try_from(key.room_id.as_str()) else {
        return;
    };
    KEYS.lock()
        .unwrap()
        .received
        .entry(room_id)
        .or_default()
        .insert((key.identities[0].clone(), key.index), key.clone());
    emit(key);
}

/// The key in a key message from `sender` (`sender_device`: the device Olm says it came from,
/// when known).
fn parse(sender: &str, sender_device: Option<&str>, content: KeysContent) -> Option<CallKey> {
    let device = content.member.claimed_device_id;
    // Olm's word and the claim must agree.
    if sender_device.is_some_and(|d| d != device) {
        warn!(%sender, "call key claims another device, ignored");
        return None;
    }
    let media = content.media_key.or(content.keys)?;
    let index = u8::try_from(media.index).ok()?;
    let raw = LENIENT.decode(&media.key).ok().filter(|k| !k.is_empty())?;
    let member_id = content.member_id.or(content.member.id);
    Some(CallKey {
        room_id: content.room_id.to_string(),
        identities: identities(sender, &device, member_id.as_deref()),
        index,
        key: STANDARD.encode(raw),
        own: false,
    })
}

/// Devices in the room's call now (from the call membership state), except ours.
pub(crate) async fn call_devices(room: &Room) -> HashSet<(OwnedUserId, OwnedDeviceId)> {
    let own_user = room.own_user_id().to_owned();
    let own_device = room.client().device_id().map(ToOwned::to_owned);
    let now: u64 = MilliSecondsSinceUnixEpoch::now().get().into();
    let mut out = HashSet::new();
    let events = room
        .get_state_events(StateEventType::from(CALL_MEMBER))
        .await
        .unwrap_or_default();
    for event in events {
        let RawAnySyncOrStrippedState::Sync(raw) = event else {
            continue;
        };
        if let Some((user, device)) = call_device(&raw, now)
            && !(user == own_user && Some(&device) == own_device.as_ref())
        {
            out.insert((user, device));
        }
    }
    out
}

/// The device a call membership event puts in the call (`now`: ms), if still current.
fn call_device(raw: &Raw<AnySyncStateEvent>, now: u64) -> Option<(OwnedUserId, OwnedDeviceId)> {
    let sender = raw.get_field::<OwnedUserId>("sender").ok()??;
    let content = raw.get_field::<serde_json::Value>("content").ok()??;
    // Left: empty content.
    let device = content.get("device_id")?.as_str()?;
    let sent = raw
        .get_field::<u64>("origin_server_ts")
        .ok()
        .flatten()
        .unwrap_or(now);
    let created = content
        .get("created_ts")
        .and_then(|t| t.as_u64())
        .unwrap_or(sent);
    let expires = content
        .get("expires")
        .and_then(|t| t.as_u64())
        .unwrap_or(4 * 3600 * 1000);
    (created + expires >= now).then(|| (sender, OwnedDeviceId::from(device)))
}

fn new_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::rng().fill_bytes(&mut key);
    key
}

/// Send our key to these devices (Olm-encrypted); returns the ones it reached.
async fn send(
    room: &Room,
    targets: &HashSet<(OwnedUserId, OwnedDeviceId)>,
    index: u8,
    key: &[u8],
) -> HashSet<(OwnedUserId, OwnedDeviceId)> {
    if targets.is_empty() {
        return HashSet::new();
    }
    let client = room.client();
    let (Some(user), Some(device)) = (client.user_id(), client.device_id()) else {
        return HashSet::new();
    };
    let member_id = format!("{user}:{device}");
    let media_key = json!({ "index": index, "key": STANDARD.encode(key) });
    let content = json!({
        "room_id": room.room_id(),
        "member_id": member_id,
        "media_key": media_key,
        "keys": media_key,
        "member": { "claimed_device_id": device, "id": member_id },
        "session": { "application": "m.call", "call_id": "", "scope": "m.room" },
        "sent_ts": MilliSecondsSinceUnixEpoch::now(),
    });
    let encryption = client.encryption();
    let mut devices = Vec::new();
    for (user, device) in targets {
        match encryption.get_device(user, device).await {
            Ok(Some(d)) => devices.push(d),
            // Not known yet (keys not fetched): asking fetches them for the next round.
            _ => {
                let _ = encryption.request_user_identity(user).await;
            }
        }
    }
    let Ok(content) = Raw::new(&content) else {
        return HashSet::new();
    };
    let content = content.cast_unchecked::<AnyToDeviceEventContent>();
    let failed = match encryption
        .encrypt_and_send_raw_to_device(
            devices.iter().collect(),
            KEYS_EVENT,
            content,
            matrix_sdk_base::crypto::CollectStrategy::AllDevices,
        )
        .await
    {
        Ok(failed) => failed,
        Err(e) => {
            warn!("couldn't send the call key: {e}");
            return HashSet::new();
        }
    };
    let reached: HashSet<_> = devices
        .iter()
        .map(|d| (d.user_id().to_owned(), d.device_id().to_owned()))
        .filter(|d| !failed.contains(d))
        .collect();
    info!(
        index,
        sent = reached.len(),
        of = targets.len(),
        "call key sent"
    );
    reached
}

fn set_own(room: &Room, index: u8, key: &[u8]) {
    let client = room.client();
    let (Some(user), Some(device)) = (client.user_id(), client.device_id()) else {
        return;
    };
    let own = CallKey {
        room_id: room.room_id().to_string(),
        identities: identities(user.as_str(), device.as_str(), None),
        index,
        key: STANDARD.encode(key),
        own: true,
    };
    KEYS.lock().unwrap().own = Some(own.clone());
    emit(own);
}

/// Our key for this call, kept with the others in the call while we're in it (see the module
/// docs). Runs until aborted (leaving the call).
pub(crate) async fn run(room: Room) {
    let mut index: u8 = 0;
    let mut key = new_key();
    set_own(&room, index, &key);
    let mut sent: HashSet<(OwnedUserId, OwnedDeviceId)> = HashSet::new();
    loop {
        let members = call_devices(&room).await;
        if sent.iter().any(|d| !members.contains(d)) {
            // Someone left: a new key they don't get.
            index = index.wrapping_add(1);
            key = new_key();
            sent = send(&room, &members, index, &key).await;
            crate::rt::sleep(USE_KEY_DELAY).await;
            set_own(&room, index, &key);
        } else {
            let new: HashSet<_> = members.difference(&sent).cloned().collect();
            if !new.is_empty() {
                sent.extend(send(&room, &new, index, &key).await);
            }
        }
        let wait = crate::rt::sleep(CHECK_EVERY);
        let changed = MEMBERS_CHANGED.notified();
        futures_util::pin_mut!(wait, changed);
        futures_util::future::select(wait, changed).await;
    }
}

/// Forget the keys of a call we left.
pub(crate) fn forget(room_id: &OwnedRoomId) {
    let mut keys = KEYS.lock().unwrap();
    keys.received.remove(room_id);
    if keys
        .own
        .as_ref()
        .is_some_and(|k| k.room_id == room_id.as_str())
    {
        keys.own = None;
    }
}

impl crate::Core {
    /// `handler` gets every new call media key (ours and received ones).
    pub fn on_call_key(&self, handler: impl Fn(CallKey) + Send + Sync + 'static) {
        *HANDLER.lock().unwrap() = Some(Box::new(handler));
    }

    /// The keys known for the call in `room_id` (ours first), for a media layer that starts
    /// after some arrived.
    pub fn call_keys(&self, room_id: &str) -> Vec<CallKey> {
        let keys = KEYS.lock().unwrap();
        keys.own
            .iter()
            .filter(|k| k.room_id == room_id)
            .cloned()
            .chain(
                keys.received
                    .iter()
                    .filter(|(room, _)| room.as_str() == room_id)
                    .flat_map(|(_, keys)| keys.values().cloned()),
            )
            .collect()
    }
}

#[cfg(test)]
mod tests {
    /// Margot's membership as Element Call wrote it in "voix" (2026-11-03), then her leaving.
    #[test]
    fn call_members() {
        let joined: matrix_sdk::ruma::serde::Raw<_> = serde_json::from_value(serde_json::json!({
            "content": {"application":"m.call","call_id":"","created_ts":1793742008293u64,"device_id":"b8gYJelmMI","expires":14400000,"foci_preferred":[{"livekit_alias":"!c:x.org","livekit_service_url":"https://rtc.x.org","type":"livekit"}],"focus_active":{"focus_selection":"oldest_membership","type":"livekit"},"m.call.intent":"video","membershipID":"@m:x.org:b8gYJelmMI","scope":"m.room"},
            "event_id":"$e","origin_server_ts":1793742010000u64,"sender":"@m:x.org","state_key":"_@m:x.org_b8gYJelmMI_m.call","type":"org.matrix.msc3401.call.member",
        })).unwrap();
        let now = 1793742100000;
        assert_eq!(
            super::call_device(&joined, now),
            Some(("@m:x.org".try_into().unwrap(), "b8gYJelmMI".into()))
        );
        // Expired (4 h later).
        assert_eq!(super::call_device(&joined, now + 5 * 3600 * 1000), None);
        let left: matrix_sdk::ruma::serde::Raw<_> = serde_json::from_value(serde_json::json!({
            "content": {}, "event_id":"$f","origin_server_ts":1793742193626u64,"sender":"@m:x.org","state_key":"_@m:x.org_b8gYJelmMI_m.call","type":"org.matrix.msc3401.call.member",
        })).unwrap();
        assert_eq!(super::call_device(&left, now), None);
    }

    fn content(value: serde_json::Value) -> super::KeysContent {
        serde_json::from_value(value).unwrap()
    }

    /// As Element Call's ToDeviceKeyTransport sends it (padded base64 key, member id).
    #[test]
    fn element_call_key() {
        let key = super::parse(
            "@m:x.org",
            Some("DEV"),
            content(serde_json::json!({
                "room_id": "!r:x.org",
                "member_id": "@m:x.org:DEV",
                "media_key": { "index": 3, "key": "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=" },
                "keys": { "index": 3, "key": "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=" },
                "member": { "claimed_device_id": "DEV", "id": "@m:x.org:DEV" },
            })),
        )
        .unwrap();
        assert_eq!(key.room_id, "!r:x.org");
        assert_eq!(key.index, 3);
        assert_eq!(key.identities[0], "@m:x.org:DEV");
        assert_eq!(key.key, "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=");
        assert!(!key.own);
    }

    /// Older senders: only `keys` (unpadded), no member id.
    #[test]
    fn older_key_format() {
        let key = super::parse(
            "@m:x.org",
            None,
            content(serde_json::json!({
                "room_id": "!r:x.org",
                "keys": { "index": 0, "key": "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA" },
                "member": { "claimed_device_id": "DEV" },
            })),
        )
        .unwrap();
        assert_eq!(key.identities, super::identities("@m:x.org", "DEV", None));
        assert_eq!(key.key, "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=");
    }

    #[test]
    fn rejected_keys() {
        let ok = serde_json::json!({ "index": 0, "key": "AQID" });
        // Olm says another device sent it.
        let spoof = content(serde_json::json!({
            "room_id": "!r:x.org", "media_key": ok, "member": { "claimed_device_id": "DEV" },
        }));
        assert!(super::parse("@m:x.org", Some("OTHER"), spoof).is_none());
        // Index out of range, empty key, no key.
        for media in [
            serde_json::json!({ "index": 256, "key": "AQID" }),
            serde_json::json!({ "index": 0, "key": "" }),
        ] {
            let c = content(serde_json::json!({
                "room_id": "!r:x.org", "media_key": media, "member": { "claimed_device_id": "DEV" },
            }));
            assert!(super::parse("@m:x.org", None, c).is_none());
        }
        let none = content(serde_json::json!({
            "room_id": "!r:x.org", "member": { "claimed_device_id": "DEV" },
        }));
        assert!(super::parse("@m:x.org", None, none).is_none());
    }

    #[test]
    fn identities() {
        let ids = super::identities("@a:x.org", "DEV", None);
        assert_eq!(ids[0], "@a:x.org:DEV");
        // sha256('["@a:x.org","@a:x.org:DEV"]'), unpadded base64 (checked with Python).
        assert_eq!(ids[1], "zuErAE8wHGAN59SN2QbZKKGblGFUW6iG0Z1OBEOw9nw");
        let other = super::identities("@a:x.org", "DEV", Some("@a:x.org:DEV"));
        assert_eq!(ids, other);
    }
}
