//! "Saved messages": a private, encrypted chat with only ourselves, for notes and things kept
//! for later. Its room ID is global account data `com.alwy.chat.saved_messages`, so every device
//! (and a reinstall) finds the same chat; other clients just see a chat named "Saved messages".

use matrix_sdk::{
    RoomState,
    ruma::{
        OwnedRoomId, api::client::room::create_room, events::GlobalAccountDataEventType, serde::Raw,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::warn;

use crate::{Error, Result};

use crate::names::SAVED_MESSAGES;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Pointer {
    room_id: Option<String>,
}

impl crate::Core {
    /// Our saved-messages chat, if it exists and we're still in it.
    pub async fn saved_messages_id(&self) -> Result<Option<String>> {
        let client = self.client()?;
        let account = client.account();
        let kind = GlobalAccountDataEventType::from(SAVED_MESSAGES);
        let raw = match account.fetch_account_data(kind.clone()).await {
            Ok(raw) => raw,
            Err(e) => {
                warn!("saved messages pointer from the server: {e}");
                account.account_data_raw(kind).await?
            }
        };
        let id = raw
            .and_then(|r| serde_json::from_str::<Pointer>(r.json().get()).ok())
            .and_then(|p| p.room_id)
            .and_then(|id| OwnedRoomId::try_from(id).ok());
        Ok(id
            .filter(|id| {
                client
                    .get_room(id)
                    .is_some_and(|r| r.state() == RoomState::Joined)
            })
            .map(|id| id.to_string()))
    }

    /// Our saved-messages chat, created on first use.
    pub async fn open_saved_messages(&self) -> Result<String> {
        if let Some(id) = self.saved_messages_id().await? {
            return Ok(id);
        }
        let client = self.client()?;
        let mut request = create_room::v3::Request::new();
        request.name = Some("Saved messages".to_owned());
        request.topic = Some("Notes and messages kept for later. Only you are here.".to_owned());
        request.preset = Some(create_room::v3::RoomPreset::PrivateChat);
        request.initial_state = vec![
            Raw::new(&json!({
                "type": "m.room.encryption",
                "state_key": "",
                "content": { "algorithm": "m.megolm.v1.aes-sha2" },
            }))?
            .cast_unchecked(),
            Raw::new(&json!({
                "type": "m.room.history_visibility",
                "state_key": "",
                "content": { "history_visibility": "joined" },
            }))?
            .cast_unchecked(),
        ];
        let room = client.create_room(request).await?;
        let id = room.room_id().to_string();
        client
            .account()
            .set_account_data_raw(
                GlobalAccountDataEventType::from(SAVED_MESSAGES),
                Raw::new(&Pointer {
                    room_id: Some(id.clone()),
                })?
                .cast_unchecked(),
            )
            .await
            .map_err(Error::from)?;
        Ok(id)
    }
}
