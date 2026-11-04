//! Unlocking encryption on a new device: recovery key entry, and verification with another
//! device by comparing emoji (SAS) or with QR codes (one device shows, the other scans).

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use futures_util::{StreamExt, stream};
use matrix_sdk::{
    Client,
    encryption::{
        VerificationState,
        recovery::RecoveryState,
        verification::{
            QrVerification, QrVerificationData, QrVerificationState, SasState, SasVerification,
            VerificationRequest, VerificationRequestState,
        },
    },
    ruma::events::key::verification::{
        VerificationMethod, request::ToDeviceKeyVerificationRequestEvent,
    },
};
use serde::Serialize;
use tracing::warn;

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct EncryptionStatus {
    /// This device is signed by our cross-signing identity (verified).
    pub device_verified: bool,
    pub recovery: RecoveryStatus,
    /// Another device of ours could verify this one (emoji or QR code).
    pub can_verify_with_device: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStatus {
    Unknown,
    /// All secrets are here: old messages can be decrypted from the key backup.
    Enabled,
    /// Recovery is set up on the account but this device lacks the secrets.
    Incomplete,
    /// The account has no recovery set up.
    Disabled,
}

/// Progress of the emoji verification, pushed to the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VerificationUpdate {
    /// We asked; waiting for the other device to accept.
    Requested,
    /// Another device asked to verify; accept or cancel.
    Incoming {
        from: String,
    },
    /// Both sides agreed; emoji are being prepared.
    Ready,
    /// Both sides agreed and QR codes are possible: scan the other device's code (`can_scan`)
    /// and/or let it scan ours (`qr`, an SVG), or compare emoji instead.
    Choose {
        qr: Option<String>,
        can_scan: bool,
    },
    /// The other device scanned our code: does it say it worked?
    QrScanned,
    /// Compare these with the other device, then confirm or report a mismatch.
    Emojis {
        emojis: Vec<VerificationEmoji>,
    },
    /// We confirmed; waiting for the other device.
    Confirmed,
    Done,
    Cancelled {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct VerificationEmoji {
    pub symbol: String,
    pub description: String,
}

type Sink = Arc<dyn Fn(VerificationUpdate) + Send + Sync>;

/// Verification state shared with the incoming-request event handler.
#[derive(Default)]
pub(crate) struct Verifications {
    sink: Mutex<Option<Sink>>,
    current: Mutex<Option<Flow>>,
    /// The latest update of a verification still going on. A request can arrive before the UI
    /// subscribes (at startup, or when a push started the app in the background): it's
    /// replayed on subscribing, else the request would never show.
    last: Mutex<Option<VerificationUpdate>>,
    /// This device has a camera to scan QR codes (the UI says so).
    can_scan: AtomicBool,
}

/// The single verification in progress.
struct Flow {
    request: VerificationRequest,
    sas: Arc<Mutex<Option<SasVerification>>>,
    qr: Arc<Mutex<Option<QrVerification>>>,
    task: crate::rt::AbortHandle,
}

impl Drop for Flow {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Verifications {
    /// Create the hub and listen for verification requests sent by our other devices.
    pub(crate) fn install(client: &Client) -> Arc<Self> {
        let hub = Arc::new(Self::default());
        let weak = Arc::downgrade(&hub);
        client.add_event_handler(
            move |ev: ToDeviceKeyVerificationRequestEvent, client: Client| {
                let weak = weak.clone();
                async move {
                    let Some(hub) = weak.upgrade() else { return };
                    let Some(request) = client
                        .encryption()
                        .get_verification_request(&ev.sender, &ev.content.transaction_id)
                        .await
                    else {
                        return;
                    };
                    let from = match client
                        .encryption()
                        .get_device(&ev.sender, &ev.content.from_device)
                        .await
                    {
                        Ok(Some(device)) => device
                            .display_name()
                            .map(ToOwned::to_owned)
                            .unwrap_or_else(|| ev.content.from_device.to_string()),
                        _ => ev.sender.to_string(),
                    };
                    hub.start(request, Some(from));
                }
            },
        );
        hub
    }

    fn emit(&self, update: VerificationUpdate) {
        *self.last.lock().unwrap() = match update {
            VerificationUpdate::Done | VerificationUpdate::Cancelled { .. } => None,
            ref going_on => Some(going_on.clone()),
        };
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink(update);
        }
    }

    /// Track `request` (replacing any other) and drive it to completion.
    fn start(self: &Arc<Self>, request: VerificationRequest, incoming_from: Option<String>) {
        let sas = Arc::new(Mutex::new(None));
        let qr = Arc::new(Mutex::new(None));
        let task = crate::rt::spawn(drive(
            Arc::downgrade(self),
            request.clone(),
            sas.clone(),
            qr.clone(),
        ));
        let old = self.current.lock().unwrap().replace(Flow {
            request,
            sas,
            qr,
            task: task.abort_handle(),
        });
        if let Some(old) = old {
            let request = old.request.clone();
            crate::rt::spawn(async move { request.cancel().await });
        }
        self.emit(match incoming_from {
            Some(from) => VerificationUpdate::Incoming { from },
            None => VerificationUpdate::Requested,
        });
    }

    fn request(&self) -> Result<VerificationRequest> {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .map(|f| f.request.clone())
            .ok_or(Error::NoVerification)
    }

    fn qr(&self) -> Result<QrVerification> {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|f| f.qr.lock().unwrap().clone())
            .ok_or(Error::NoVerification)
    }

    /// The methods we offer: emoji, showing a QR code, and scanning one if there's a camera.
    fn methods(&self) -> Vec<VerificationMethod> {
        let mut methods = vec![
            VerificationMethod::SasV1,
            VerificationMethod::QrCodeShowV1,
            VerificationMethod::ReciprocateV1,
        ];
        if self.can_scan.load(Ordering::Relaxed) {
            methods.push(VerificationMethod::QrCodeScanV1);
        }
        methods
    }

    fn sas(&self) -> Result<SasVerification> {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|f| f.sas.lock().unwrap().clone())
            .ok_or(Error::NoVerification)
    }
}

/// Follow the request until it becomes an emoji verification, then follow that.
async fn drive(
    hub: std::sync::Weak<Verifications>,
    request: VerificationRequest,
    slot: Arc<Mutex<Option<SasVerification>>>,
    qr_slot: Arc<Mutex<Option<QrVerification>>>,
) {
    let emit = |update| {
        if let Some(hub) = hub.upgrade() {
            hub.emit(update);
        }
    };
    let mut changes = request.changes();
    let mut state = request.state();
    loop {
        match state {
            VerificationRequestState::Created { .. }
            | VerificationRequestState::Requested { .. } => {}
            VerificationRequestState::Ready { .. } => {
                // What we offered (asked with, or accepted with).
                let ours = hub.upgrade().map(|h| h.methods()).unwrap_or_default();
                let theirs = request.their_supported_methods().unwrap_or_default();
                let both = |a: VerificationMethod, b: VerificationMethod| {
                    ours.contains(&a) && theirs.contains(&b)
                };
                let can_show = both(
                    VerificationMethod::QrCodeShowV1,
                    VerificationMethod::QrCodeScanV1,
                );
                let can_scan = both(
                    VerificationMethod::QrCodeScanV1,
                    VerificationMethod::QrCodeShowV1,
                );
                if can_show || can_scan {
                    // QR codes possible: the person picks (or the other side starts emoji).
                    let shown = if can_show {
                        match request.generate_qr_code().await {
                            Ok(Some(qr)) => {
                                let svg = qr_svg(&qr);
                                *qr_slot.lock().unwrap() = Some(qr);
                                svg
                            }
                            Ok(None) => None,
                            Err(e) => {
                                warn!("couldn't make the verification QR code: {e}");
                                None
                            }
                        }
                    } else {
                        None
                    };
                    emit(VerificationUpdate::Choose {
                        qr: shown,
                        can_scan,
                    });
                } else {
                    emit(VerificationUpdate::Ready);
                    // Only emoji. If both sides start it at once, the spec drops one of the two,
                    // so the side that asked starts it; the other only steps in when nothing
                    // came (some clients wait for the other side to start).
                    if !request.we_started() {
                        tokio::select! {
                            next = changes.next() => {
                                state = match next {
                                    Some(s) => s,
                                    None => return,
                                };
                                continue;
                            }
                            () = crate::rt::sleep(std::time::Duration::from_secs(3)) => {}
                        }
                    }
                    if let Err(e) = request.start_sas().await {
                        warn!("couldn't start emoji verification: {e}");
                    }
                }
            }
            VerificationRequestState::Transitioned { verification, .. } => {
                if let Some(qr) = verification.clone().qr() {
                    *qr_slot.lock().unwrap() = Some(qr.clone());
                    tokio::select! {
                        () = drive_qr(qr, &emit) => return,
                        next = changes.next() => {
                            state = match next {
                                Some(s) => s,
                                None => return,
                            };
                            continue;
                        }
                    }
                }
                let Some(sas) = verification.sas() else {
                    let _ = request.cancel().await;
                    return emit(VerificationUpdate::Cancelled {
                        reason: "Unsupported verification method".to_owned(),
                    });
                };
                *slot.lock().unwrap() = Some(sas.clone());
                // Follow it, but switch when the request moves to another one (both sides
                // started at once and the spec kept the other side's).
                tokio::select! {
                    () = drive_sas(sas, &emit) => return,
                    next = changes.next() => {
                        state = match next {
                            Some(s) => s,
                            None => return,
                        };
                        continue;
                    }
                }
            }
            VerificationRequestState::Done => return emit(VerificationUpdate::Done),
            VerificationRequestState::Cancelled(info) => {
                return emit(VerificationUpdate::Cancelled {
                    reason: info.reason().to_owned(),
                });
            }
        }
        state = match changes.next().await {
            Some(s) => s,
            None => return,
        };
    }
}

/// Follow a QR verification: ours being scanned (ask to confirm), or theirs that we scanned
/// (wait for them to confirm).
async fn drive_qr(qr: QrVerification, emit: impl Fn(VerificationUpdate)) {
    let mut changes = qr.changes();
    let mut state = qr.state();
    loop {
        match state {
            // Our code is on screen (already sent with `Choose`).
            QrVerificationState::Started => {}
            QrVerificationState::Scanned => emit(VerificationUpdate::QrScanned),
            QrVerificationState::Confirmed | QrVerificationState::Reciprocated => {
                emit(VerificationUpdate::Confirmed)
            }
            QrVerificationState::Done { .. } => return emit(VerificationUpdate::Done),
            QrVerificationState::Cancelled(info) => {
                return emit(VerificationUpdate::Cancelled {
                    reason: info.reason().to_owned(),
                });
            }
        }
        state = match changes.next().await {
            Some(s) => s,
            None => return,
        };
    }
}

/// The QR code as an SVG (black on white, with the quiet zone scanners need).
fn qr_svg(qr: &QrVerification) -> Option<String> {
    let code = qr.to_qr_code().ok()?;
    let dark: Vec<bool> = code
        .to_colors()
        .iter()
        .map(|c| c.select(true, false))
        .collect();
    Some(modules_svg(code.width(), &dark))
}

/// An SVG of a square grid of modules (`dark`, row by row, `width` per row).
fn modules_svg(width: usize, dark: &[bool]) -> String {
    let quiet = 4;
    let side = width + 2 * quiet;
    let mut path = String::new();
    for (i, _) in dark.iter().enumerate().filter(|(_, d)| **d) {
        let (x, y) = (i % width + quiet, i / width + quiet);
        path.push_str(&format!("M{x} {y}h1v1h-1z"));
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {side} {side}" shape-rendering="crispEdges"><rect width="{side}" height="{side}" fill="#fff"/><path d="{path}" fill="#000"/></svg>"##
    )
}

/// Find a QR code in a greyscale picture (`width` × `height`, one byte per pixel) and return
/// its raw bytes (Matrix verification codes are binary).
pub(crate) fn find_qr(width: usize, height: usize, luma: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || luma.len() < width * height {
        return None;
    }
    let mut image =
        rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| luma[y * width + x]);
    image.detect_grids().into_iter().find_map(|grid| {
        let mut bytes = Vec::new();
        grid.decode_to(&mut bytes).ok()?;
        Some(bytes)
    })
}

async fn drive_sas(sas: SasVerification, emit: impl Fn(VerificationUpdate)) {
    let mut changes = sas.changes();
    let mut state = sas.state();
    loop {
        match state {
            // The other side started: accept so keys get exchanged.
            SasState::Started { .. } => {
                if let Err(e) = sas.accept().await {
                    warn!("couldn't accept emoji verification: {e}");
                }
            }
            SasState::KeysExchanged { .. } => {
                if let Some(emojis) = sas.emoji() {
                    emit(VerificationUpdate::Emojis {
                        emojis: emojis
                            .iter()
                            .map(|e| VerificationEmoji {
                                symbol: e.symbol.to_owned(),
                                description: e.description.to_owned(),
                            })
                            .collect(),
                    });
                }
            }
            SasState::Confirmed => emit(VerificationUpdate::Confirmed),
            SasState::Done { .. } => return emit(VerificationUpdate::Done),
            SasState::Cancelled(info) => {
                return emit(VerificationUpdate::Cancelled {
                    reason: info.reason().to_owned(),
                });
            }
            SasState::Created { .. } | SasState::Accepted { .. } => {}
        }
        state = match changes.next().await {
            Some(s) => s,
            None => return,
        };
    }
}

impl crate::Core {
    /// Stream encryption status and verification progress until signed out.
    pub fn subscribe_encryption(
        &self,
        on_status: impl Fn(EncryptionStatus) + Send + Sync + 'static,
        on_verification: impl Fn(VerificationUpdate) + Send + Sync + 'static,
    ) -> Result<()> {
        let (client, hub) = self.with_active(|a| (a.client.clone(), a.verifications.clone()))?;
        let sink: Sink = Arc::new(on_verification);
        if let Some(update) = hub.last.lock().unwrap().clone() {
            sink(update);
        }
        *hub.sink.lock().unwrap() = Some(sink);

        let task = crate::rt::spawn(async move {
            let encryption = client.encryption();
            let verified = encryption.verification_state().map(|_| ());
            let recovery = encryption.recovery().state_stream().map(|_| ());
            let mut changes = stream::select(verified, recovery);
            loop {
                on_status(status(&client).await);
                if changes.next().await.is_none() {
                    return;
                }
            }
        });
        self.with_active(|a| {
            if let Some(old) = a.encryption_task.replace(task.abort_handle()) {
                old.abort();
            }
        })
    }

    /// Unlock with the recovery key (or passphrase) set up in Element.
    pub async fn recover(&self, key: &str) -> Result<()> {
        let client = self.client()?;
        client
            .encryption()
            .recovery()
            .recover(key.trim())
            .await
            .map_err(|e| Error::Encryption(e.to_string()))
    }

    /// Ask our other devices to verify this one with emoji.
    pub async fn verify_this_device(&self) -> Result<()> {
        let (client, hub) = self.with_active(|a| (a.client.clone(), a.verifications.clone()))?;
        let own_id = client.user_id().ok_or(Error::NotLoggedIn)?.to_owned();
        let identity = client
            .encryption()
            .get_user_identity(&own_id)
            .await
            .map_err(|e| Error::Encryption(e.to_string()))?
            .ok_or_else(|| {
                Error::Encryption("this account has no cross-signing identity".into())
            })?;
        let request = identity
            .request_verification_with_methods(hub.methods())
            .await
            .map_err(|e| Error::Encryption(e.to_string()))?;
        hub.start(request, None);
        Ok(())
    }

    /// Accept a verification requested by another device.
    pub async fn accept_verification(&self) -> Result<()> {
        let (request, methods) =
            self.with_active(|a| (a.verifications.request(), a.verifications.methods()))?;
        request?
            .accept_with_methods(methods)
            .await
            .map_err(|e| Error::Encryption(e.to_string()))
    }

    /// The emoji match (`true`) or not (`false`, which cancels).
    pub async fn confirm_emojis(&self, matching: bool) -> Result<()> {
        let sas = self.with_active(|a| a.verifications.sas())??;
        if matching {
            sas.confirm().await
        } else {
            sas.mismatch().await
        }
        .map_err(|e| Error::Encryption(e.to_string()))
    }

    /// Whether this device can scan QR codes (has a camera); offered from then on.
    pub fn set_can_scan_qr(&self, can: bool) -> Result<()> {
        self.with_active(|a| a.verifications.can_scan.store(can, Ordering::Relaxed))
    }

    /// Compare emoji instead of QR codes.
    pub async fn start_emoji_verification(&self) -> Result<()> {
        let request = self.with_active(|a| a.verifications.request())??;
        request
            .start_sas()
            .await
            .map_err(|e| Error::Encryption(e.to_string()))?;
        Ok(())
    }

    /// A camera frame (greyscale, one byte per pixel) while scanning the other device's code:
    /// `Ok(false)` while no code is seen, `Ok(true)` once it's read and the verification goes on.
    pub async fn scan_verification_frame(
        &self,
        width: usize,
        height: usize,
        luma: &[u8],
    ) -> Result<bool> {
        let Some(bytes) = find_qr(width, height, luma) else {
            return Ok(false);
        };
        let data = QrVerificationData::from_bytes(&bytes)
            .map_err(|_| Error::Encryption("that's not a Matrix verification code".into()))?;
        let request = self.with_active(|a| a.verifications.request())??;
        request
            .scan_qr_code(data)
            .await
            .map_err(|e| Error::Encryption(e.to_string()))?
            .ok_or_else(|| {
                Error::Encryption("this code doesn't belong to this verification".into())
            })?;
        Ok(true)
    }

    /// The other device scanned our code: it says so (`true`) or not (`false`, which cancels).
    pub async fn confirm_qr_scanned(&self, ok: bool) -> Result<()> {
        let qr = self.with_active(|a| a.verifications.qr())??;
        if ok {
            qr.confirm().await
        } else {
            qr.cancel().await
        }
        .map_err(|e| Error::Encryption(e.to_string()))
    }

    pub async fn cancel_verification(&self) -> Result<()> {
        let flow = self.with_active(|a| a.verifications.current.lock().unwrap().take())?;
        if let Some(flow) = flow {
            flow.request
                .cancel()
                .await
                .map_err(|e| Error::Encryption(e.to_string()))?;
        }
        Ok(())
    }
}

async fn status(client: &Client) -> EncryptionStatus {
    let encryption = client.encryption();
    EncryptionStatus {
        device_verified: matches!(
            encryption.verification_state().get(),
            VerificationState::Verified
        ),
        recovery: match encryption.recovery().state() {
            RecoveryState::Unknown => RecoveryStatus::Unknown,
            RecoveryState::Enabled => RecoveryStatus::Enabled,
            RecoveryState::Incomplete => RecoveryStatus::Incomplete,
            RecoveryState::Disabled => RecoveryStatus::Disabled,
        },
        can_verify_with_device: encryption
            .has_devices_to_verify_against()
            .await
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use matrix_sdk::encryption::verification::QrVerificationData;

    /// A valid self-verification code (from matrix-sdk-qrcode's docs): binary keys inside.
    const SAMPLE: &[u8] = b"MATRIX\
        \x02\x02\x00\x07\
        FLOW_ID\
        kS /\x92i\x1e6\xcd'g\xf9#\x11\xd8\x8a\xa2\xf61\x05\x1b6\xef\xfc\xa4%\x80\x1a\x0c\xd2\xe8\x04\
        \xbdR|\xf8n\x07\xa4\x1f\xb4\xcc3\x0eBT\xe7[~\xfd\x87\xd06B\xdfoVv%\x9b\x86\xae\xbcM\
        SHARED_SECRET";

    /// The code as a camera would see it: `scale` px per module, grey levels, a quiet zone.
    fn picture(scale: usize) -> (usize, Vec<u8>) {
        let data = QrVerificationData::from_bytes(SAMPLE).unwrap();
        let code = data.to_qr_code().unwrap();
        let width = code.width();
        let colors = code.to_colors();
        let side = (width + 8) * scale;
        let mut luma = vec![210u8; side * side];
        for (i, c) in colors.iter().enumerate() {
            if c.select(true, false) {
                let (mx, my) = (i % width + 4, i / width + 4);
                for y in my * scale..(my + 1) * scale {
                    for x in mx * scale..(mx + 1) * scale {
                        luma[y * side + x] = 35;
                    }
                }
            }
        }
        (side, luma)
    }

    #[test]
    fn scans_binary_matrix_code() {
        for scale in [3, 6] {
            let (side, luma) = picture(scale);
            let bytes = super::find_qr(side, side, &luma).expect("code found");
            assert_eq!(bytes, SAMPLE, "scale {scale}");
            let data = QrVerificationData::from_bytes(&bytes).unwrap();
            assert_eq!(data.flow_id(), "FLOW_ID");
        }
    }

    #[test]
    fn svg() {
        let svg = super::modules_svg(2, &[true, false, false, true]);
        assert!(svg.contains(r#"viewBox="0 0 10 10""#));
        assert!(svg.contains(r#"d="M4 4h1v1h-1zM5 5h1v1h-1z""#));
        let code = QrVerificationData::from_bytes(SAMPLE)
            .unwrap()
            .to_qr_code()
            .unwrap();
        let dark: Vec<bool> = code
            .to_colors()
            .iter()
            .map(|c| c.select(true, false))
            .collect();
        let svg = super::modules_svg(code.width(), &dark);
        assert_eq!(
            svg.matches('M').count(),
            dark.iter().filter(|d| **d).count()
        );
    }

    #[test]
    fn no_code() {
        assert_eq!(super::find_qr(64, 64, &[128; 64 * 64]), None);
        assert_eq!(super::find_qr(64, 64, &[0; 10]), None);
    }
}
