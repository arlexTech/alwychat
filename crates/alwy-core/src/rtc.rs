//! Calls and voice channels over MatrixRTC + LiveKit, compatible with Element Call / Element X.
//!
//! Signalling lives here; audio/video run in the webview with `livekit-client`.
//! Joining a call in a room:
//! 1. find the LiveKit service in the homeserver's well-known (`org.matrix.msc4143.rtc_foci`);
//! 2. trade an OpenID token for a LiveKit JWT at `<service>/sfu/get` (lk-jwt-service);
//! 3. publish our `org.matrix.msc3401.call.member` state event, plus a *delayed* empty one
//!    (MSC4140) that the server sends by itself if we stop refreshing it: a crashed or killed
//!    app leaves the call after `LEAVE_DELAY` instead of haunting it for hours.
//!
//! Not yet: media end-to-end encryption (Element Call's per-participant keys). Media is
//! encrypted to the SFU (DTLS-SRTP) and the SFU is Alex's server.

use std::time::Duration;

use matrix_sdk::{
    Room,
    ruma::{
        OwnedRoomId,
        api::client::{
            delayed_events::{
                DelayParameters, delayed_state_event,
                update_delayed_event::{UpdateAction, unstable_v1 as update_delayed},
            },
            rtc::RtcTransport,
        },
        events::{AnyStateEventContent, StateEventType},
        serde::Raw,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use crate::{Error, Result, mentions::Member};

const CALL_MEMBER: &str = "org.matrix.msc3401.call.member";
/// How long after our last heartbeat the server drops us from the call.
const LEAVE_DELAY: Duration = Duration::from_secs(20);
const HEARTBEAT: Duration = Duration::from_secs(8);

/// What the webview needs to connect to the call's LiveKit room.
#[derive(Debug, Clone, Serialize)]
pub struct CallConnection {
    pub url: String,
    pub jwt: String,
}

/// The call we're in (one at a time).
pub(crate) struct ActiveCall {
    room: Room,
    state_key: String,
    delay_id: Option<String>,
    heartbeat: tokio::task::AbortHandle,
}

impl Drop for ActiveCall {
    fn drop(&mut self) {
        self.heartbeat.abort();
    }
}

#[derive(Deserialize)]
struct SfuResponse {
    url: String,
    jwt: String,
}

impl crate::Core {
    /// Join (or start) the call in `room_id`. `video` only sets the advertised intent.
    pub async fn join_call(&self, room_id: &str, video: bool) -> Result<CallConnection> {
        self.leave_call().await?;
        let room = self.room(room_id)?;
        let client = room.client();
        let user = client.user_id().ok_or(Error::NotLoggedIn)?.to_owned();
        let device = client.device_id().ok_or(Error::NotLoggedIn)?.to_owned();

        let service = client
            .well_known_rtc_transports()
            .await?
            .into_iter()
            .find_map(|t| match t {
                RtcTransport::LiveKit(lk) => Some(lk.service_url),
                _ => None,
            })
            .ok_or_else(|| Error::Call("this server has no call service (LiveKit)".into()))?;
        let service = service.trim_end_matches('/').to_owned();

        // OpenID token → LiveKit JWT.
        let openid = client.account().request_openid_token().await?;
        let body = json!({
            "room": room.room_id(),
            "device_id": device,
            "openid_token": {
                "access_token": openid.access_token,
                "token_type": openid.token_type,
                "matrix_server_name": openid.matrix_server_name,
                "expires_in": openid.expires_in.as_secs(),
            },
        });
        let response = reqwest::Client::new()
            .post(format!("{service}/sfu/get"))
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Call(format!("call service unreachable: {e}")))?;
        if !response.status().is_success() {
            return Err(Error::Call(format!(
                "call service refused ({})",
                response.status()
            )));
        }
        let sfu: SfuResponse = response
            .json()
            .await
            .map_err(|e| Error::Call(format!("unexpected call service answer: {e}")))?;

        // Membership, as Element Call writes it (one state key per device).
        let state_key = format!("_{user}_{device}");
        let membership = json!({
            "application": "m.call",
            "call_id": "",
            "scope": "m.room",
            "device_id": device,
            "expires": 4 * 3600 * 1000,
            "focus_active": { "type": "livekit", "focus_selection": "oldest_membership" },
            "foci_preferred": [{
                "type": "livekit",
                "livekit_alias": room.room_id(),
                "livekit_service_url": service,
            }],
            "m.call.intent": if video { "video" } else { "audio" },
        });

        // The delayed leave goes first: if anything below fails, we still end up out.
        let delay_id = schedule_leave(&room, &state_key).await;
        room.send_state_event_raw(CALL_MEMBER, &state_key, membership)
            .await?;
        info!(room = %room.room_id(), "joined call");

        let heartbeat = tokio::spawn(heartbeat(room.clone(), delay_id.clone())).abort_handle();
        *self.call.lock().await = Some(ActiveCall {
            room,
            state_key,
            delay_id,
            heartbeat,
        });
        Ok(CallConnection {
            url: sfu.url,
            jwt: sfu.jwt,
        })
    }

    /// Leave the current call, if any.
    pub async fn leave_call(&self) -> Result<()> {
        let Some(call) = self.call.lock().await.take() else {
            return Ok(());
        };
        call.heartbeat.abort();
        // Fire the delayed leave now; if that fails, send the empty membership ourselves.
        let fired = match &call.delay_id {
            Some(id) => call
                .room
                .client()
                .send(update_delayed::Request::new(id.clone(), UpdateAction::Send))
                .await
                .is_ok(),
            None => false,
        };
        if !fired {
            call.room
                .send_state_event_raw(CALL_MEMBER, &call.state_key, json!({}))
                .await?;
        }
        info!(room = %call.room.room_id(), "left call");
        Ok(())
    }

    /// The room of the call we're in.
    pub async fn current_call(&self) -> Option<OwnedRoomId> {
        self.call
            .lock()
            .await
            .as_ref()
            .map(|c| c.room.room_id().to_owned())
    }

    /// People in the room's call right now (for "join" banners and voice-channel avatars).
    pub async fn call_participants(&self, room_id: &str) -> Result<Vec<Member>> {
        let room = self.room(room_id)?;
        let own = room.own_user_id().to_owned();
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for user in room.active_room_call_participants() {
            if !seen.insert(user.clone()) {
                continue;
            }
            let member = room.get_member_no_sync(&user).await.ok().flatten();
            out.push(Member {
                is_me: user == own,
                name: member
                    .as_ref()
                    .and_then(|m| m.display_name().map(ToOwned::to_owned))
                    .unwrap_or_else(|| user.localpart().to_owned()),
                avatar: member.and_then(|m| m.avatar_url().map(ToString::to_string)),
                user_id: user.to_string(),
            });
        }
        Ok(out)
    }
}

/// Schedule the empty membership the server sends if our heartbeats stop.
async fn schedule_leave(room: &Room, state_key: &str) -> Option<String> {
    let request = delayed_state_event::unstable::Request::new_raw(
        room.room_id().to_owned(),
        state_key.to_owned(),
        StateEventType::from(CALL_MEMBER),
        DelayParameters::Timeout {
            timeout: LEAVE_DELAY,
        },
        Raw::<AnyStateEventContent>::from_json_string("{}".into()).expect("valid JSON"),
    );
    match room.client().send(request).await {
        Ok(r) => Some(r.delay_id),
        Err(e) => {
            warn!("server doesn't take delayed events, a crash would leave us in the call: {e}");
            None
        }
    }
}

/// Keep pushing the delayed leave back while we're in the call.
async fn heartbeat(room: Room, delay_id: Option<String>) {
    let Some(id) = delay_id else { return };
    loop {
        tokio::time::sleep(HEARTBEAT).await;
        let request = update_delayed::Request::new(id.clone(), UpdateAction::Restart);
        if let Err(e) = room.client().send(request).await {
            warn!("call heartbeat failed: {e}");
        }
    }
}
