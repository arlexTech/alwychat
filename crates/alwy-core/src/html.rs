//! Sanitising `org.matrix.custom.html` message bodies so the UI can display them as-is.

use std::{collections::HashSet, sync::LazyLock};

use ammonia::Builder;
use matrix_sdk::ruma::events::room::message::{FormattedBody, MessageFormat};

/// Tags and attributes from the Matrix spec's recommended allowlist (client-server API,
/// "m.room.message msgtypes"), minus `img` until inline media is supported.
static SANITIZER: LazyLock<Builder<'static>> = LazyLock::new(|| {
    let mut b = Builder::empty();
    b.tags(HashSet::from([
        "font",
        "del",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "blockquote",
        "p",
        "a",
        "ul",
        "ol",
        "sup",
        "sub",
        "li",
        "b",
        "i",
        "u",
        "strong",
        "em",
        "s",
        "code",
        "hr",
        "br",
        "div",
        "table",
        "thead",
        "tbody",
        "tr",
        "th",
        "td",
        "caption",
        "pre",
        "span",
        "details",
        "summary",
    ]))
    // Reply fallbacks duplicate the quoted message, which the UI already shows.
    .clean_content_tags(HashSet::from(["mx-reply"]))
    .add_tag_attributes("a", ["href"])
    .add_tag_attributes("ol", ["start"])
    .add_tag_attributes("code", ["class"])
    .add_tag_attributes("font", ["data-mx-color", "data-mx-bg-color", "color"])
    .add_tag_attributes(
        "span",
        ["data-mx-color", "data-mx-bg-color", "data-mx-spoiler"],
    )
    .url_schemes(HashSet::from(["http", "https", "ftp", "mailto", "magnet"]))
    .link_rel(Some("noopener noreferrer"))
    // Only `language-*` classes on code blocks (for future syntax highlighting).
    .attribute_filter(|_tag, attr, value| match attr {
        "class" => value
            .split_whitespace()
            .all(|c| c.starts_with("language-"))
            .then_some(value.into()),
        _ => Some(value.into()),
    });
    b
});

/// Safe HTML for a message, or `None` when it has no HTML version.
pub(crate) fn formatted(body: Option<&FormattedBody>) -> Option<String> {
    let body = body.filter(|f| f.format == MessageFormat::Html)?;
    let html = SANITIZER.clean(&body.body).to_string();
    (!html.trim().is_empty()).then_some(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(html: &str) -> Option<String> {
        formatted(Some(&FormattedBody::html(html)))
    }

    #[test]
    fn keeps_formatting() {
        assert_eq!(
            clean("<strong>gras</strong> et <em>italique</em>").as_deref(),
            Some("<strong>gras</strong> et <em>italique</em>")
        );
    }

    #[test]
    fn strips_scripts_handlers_and_bad_links() {
        let out = clean(
            r#"<script>alert(1)</script><b onclick="x()">hi</b><a href="javascript:x()">l</a>"#,
        )
        .unwrap();
        assert!(!out.contains("script") && !out.contains("onclick") && !out.contains("javascript"));
    }

    #[test]
    fn removes_reply_fallback_and_keeps_spoilers() {
        let out = clean(
            r#"<mx-reply><blockquote>old</blockquote></mx-reply><span data-mx-spoiler="">secret</span>"#,
        )
        .unwrap();
        assert_eq!(out, r#"<span data-mx-spoiler="">secret</span>"#);
    }

    #[test]
    fn links_get_safe_rel() {
        let out = clean(r#"<a href="https://example.org">w</a>"#).unwrap();
        assert!(out.contains(r#"rel="noopener noreferrer""#));
    }
}
