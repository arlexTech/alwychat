//! Message effects, compatible with Element: its `/confetti`-style commands send custom
//! msgtypes, and plain messages with certain emoji trigger the same effects.

/// Effect name, the msgtype Element sends for it, and the emoji that trigger it.
const EFFECTS: &[(&str, &str, &[&str])] = &[
    ("confetti", "nic.custom.confetti", &["🎊", "🎉"]),
    ("fireworks", "nic.custom.fireworks", &["🎆"]),
    ("hearts", "io.element.effect.hearts", &["💝"]),
    ("snowfall", "io.element.effect.snowfall", &["❄", "🌨"]),
    ("rainfall", "io.element.effect.rainfall", &["🌧", "⛈", "🌦"]),
    (
        "space_invaders",
        "io.element.effects.space_invaders",
        &["👾"],
    ),
];

/// The effect a message plays, from its msgtype or (for text) its emoji.
pub(crate) fn effect_of(msgtype: &str, body: &str) -> Option<&'static str> {
    EFFECTS
        .iter()
        .find(|(_, t, emoji)| {
            *t == msgtype
                || (matches!(msgtype, "m.text" | "m.emote")
                    && emoji.iter().any(|e| body.contains(e)))
        })
        .map(|(name, ..)| *name)
}

#[cfg(test)]
mod tests {
    use super::effect_of;

    #[test]
    fn from_msgtype_and_emoji() {
        assert_eq!(effect_of("nic.custom.confetti", "x"), Some("confetti"));
        assert_eq!(effect_of("m.text", "bravo 🎉"), Some("confetti"));
        assert_eq!(effect_of("m.text", "il neige ❄️"), Some("snowfall"));
        assert_eq!(effect_of("m.text", "🌧️"), Some("rainfall"));
        assert_eq!(effect_of("m.text", "hello"), None);
        // Notices (bots) don't play effects.
        assert_eq!(effect_of("m.notice", "🎉"), None);
    }
}
