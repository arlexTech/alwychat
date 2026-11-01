//! Unlocking encryption on a new device: recovery key entry and emoji verification (SAS).

use std::sync::{Arc, Mutex};

use futures_util::{StreamExt, stream};
use matrix_sdk::{
    Client,
    encryption::{
        VerificationState,
        recovery::RecoveryState,
        verification::{SasState, SasVerification, VerificationRequest, VerificationRequestState},
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
    /// Another device of ours could verify this one with emoji.
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
}

/// The single verification in progress.
struct Flow {
    request: VerificationRequest,
    sas: Arc<Mutex<Option<SasVerification>>>,
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
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink(update);
        }
    }

    /// Track `request` (replacing any other) and drive it to completion.
    fn start(self: &Arc<Self>, request: VerificationRequest, incoming_from: Option<String>) {
        let sas = Arc::new(Mutex::new(None));
        let task = crate::rt::spawn(drive(Arc::downgrade(self), request.clone(), sas.clone()));
        let old = self.current.lock().unwrap().replace(Flow {
            request,
            sas,
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
                emit(VerificationUpdate::Ready);
                // We only offer emoji, so whoever gets here first starts it (the spec resolves races).
                if let Err(e) = request.start_sas().await {
                    warn!("couldn't start emoji verification: {e}");
                }
            }
            VerificationRequestState::Transitioned { verification, .. } => {
                match verification.sas() {
                    Some(sas) => {
                        *slot.lock().unwrap() = Some(sas.clone());
                        drive_sas(sas, emit).await;
                    }
                    None => {
                        let _ = request.cancel().await;
                        emit(VerificationUpdate::Cancelled {
                            reason: "Only emoji verification is supported".to_owned(),
                        });
                    }
                }
                return;
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
        *hub.sink.lock().unwrap() = Some(Arc::new(on_verification));

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
            .request_verification_with_methods(vec![VerificationMethod::SasV1])
            .await
            .map_err(|e| Error::Encryption(e.to_string()))?;
        hub.start(request, None);
        Ok(())
    }

    /// Accept a verification requested by another device.
    pub async fn accept_verification(&self) -> Result<()> {
        let request = self.with_active(|a| a.verifications.request())??;
        request
            .accept_with_methods(vec![VerificationMethod::SasV1])
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
