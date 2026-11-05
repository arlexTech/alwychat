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
name!(DISAPPEARING, "room.disappearing");
name!(COUNTDOWNS, "room.countdowns");
name!(SOUNDS, "room.sounds");
name!(BANNER, "room.banner");
// Room account data.
name!(DRAFT, "draft");
name!(MEDIA_PREFS, "room.media_prefs");
// Global account data.
name!(SAVED_MESSAGES, "saved_messages");
name!(CHAT_LIST, "chat_list");
// Timed mutes and muted spaces (mutes.rs).
name!(MUTES, "mutes");
// In a photo's content: the album it belongs to.
name!(GALLERY, "gallery");
// In a photo's or video's content: show it once (view_once.rs).
name!(VIEW_ONCE, "view_once");
// Room event: we opened a view-once message (references it).
name!(VIEW_ONCE_VIEWED, "view_once.viewed");
// Global account data: the view-once messages we opened.
name!(VIEWED_ONCE, "viewed_once");
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
