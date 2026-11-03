//! AlwyChat's app ID and the names of its own Matrix data.

/// App ID: Android package, desktop identifiers, sign-in link scheme, push app ID.
pub const APP_ID: &str = "com.alwy.chat";

macro_rules! name {
    ($id:ident, $suffix:literal) => {
        pub const $id: &str = concat!("com.alwy.chat.", $suffix);
    };
}

// Room state.
name!(QUICK_REACTION, "room.quick_reaction");
name!(THEME, "room.theme");
name!(NICKNAMES, "room.nicknames");
// Room account data.
name!(DRAFT, "draft");
name!(MEDIA_PREFS, "room.media_prefs");
// Global account data.
name!(SAVED_MESSAGES, "saved_messages");
name!(CHAT_LIST, "chat_list");
// In a photo's content: the album it belongs to.
name!(GALLERY, "gallery");
// Key in a homeserver's `/.well-known/matrix/client` naming its AlwyChat push gateway.
name!(PUSH_WELL_KNOWN, "push");

#[cfg(test)]
mod tests {
    #[test]
    fn names() {
        assert_eq!(super::THEME, "com.alwy.chat.room.theme");
        assert_eq!(super::PUSH_WELL_KNOWN, "com.alwy.chat.push");
    }
}
