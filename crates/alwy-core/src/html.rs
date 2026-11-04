//! Sanitising `org.matrix.custom.html` message bodies so the UI can display them as-is.

use std::{collections::HashSet, sync::LazyLock};

use ammonia::Builder;
use matrix_sdk::ruma::events::room::message::{FormattedBody, MessageFormat};

/// Tags and attributes from the Matrix spec's recommended allowlist (client-server API,
/// "m.room.message msgtypes"). `img` only with an `mxc://` source (custom emoji, MSC2545),
/// which the UI loads from the homeserver: never an outside address (tracking pixels).
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
        "img",
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
    .add_tag_attributes(
        "img",
        ["src", "alt", "title", "width", "height", "data-mx-emoticon"],
    )
    .url_schemes(HashSet::from([
        "http", "https", "ftp", "mailto", "magnet", "mxc",
    ]))
    .link_rel(Some("noopener noreferrer"))
    // Only `language-*` classes on code blocks (for future syntax highlighting).
    .attribute_filter(|tag, attr, value| match (tag, attr) {
        (_, "class") => value
            .split_whitespace()
            .all(|c| c.starts_with("language-"))
            .then_some(value.into()),
        // Images only from the homeserver; links never to mxc:.
        ("img", "src") => value.starts_with("mxc://").then_some(value.into()),
        (_, "href") => (!value.starts_with("mxc:")).then_some(value.into()),
        ("img", "width" | "height") => value
            .parse::<u16>()
            .ok()
            .map(|n| n.min(256).to_string().into()),
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

/// Byte ranges of the http(s) URLs in plain text.
fn find_urls(text: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find("http") {
        let start = from + offset;
        let rest = &text[start..];
        let at_word_start = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        if !(at_word_start && (rest.starts_with("https://") || rest.starts_with("http://"))) {
            from = start + 4;
            continue;
        }
        let mut end = start
            + rest
                .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"'))
                .unwrap_or(rest.len());
        // Sentence punctuation after a link isn't part of it; a ")" only if unbalanced.
        while let Some(last) = text[start..end].chars().next_back() {
            let url = &text[start..end];
            let unbalanced_paren =
                last == ')' && url.matches('(').count() < url.matches(')').count();
            if matches!(last, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '*' | '_')
                || unbalanced_paren
            {
                end -= last.len_utf8();
            } else {
                break;
            }
        }
        if text[start..end].len() > "https://".len() {
            found.push((start, end));
        }
        from = end.max(start + 4);
    }
    found
}

/// Plain text as HTML (escaped, line breaks kept).
pub(crate) fn escape_text(text: &str) -> String {
    escape(text)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "<br>")
}

/// Plain text with its URLs as links (safe HTML), or `None` when it has no URL.
pub(crate) fn linkify(text: &str) -> Option<String> {
    let urls = find_urls(text);
    if urls.is_empty() {
        return None;
    }
    let mut html = String::new();
    let mut last = 0;
    for (start, end) in urls {
        let url = escape(&text[start..end]);
        html.push_str(&escape(&text[last..start]));
        html.push_str(&format!(
            "<a href=\"{url}\" rel=\"noopener noreferrer\">{url}</a>"
        ));
        last = end;
    }
    html.push_str(&escape(&text[last..]));
    Some(html)
}

/// The URL to preview for a message: its first http(s) link outside reply quotes.
pub(crate) fn first_link(body: &str) -> Option<String> {
    body.lines()
        .filter(|line| !line.starts_with('>'))
        .find_map(|line| find_urls(line).first().map(|&(s, e)| line[s..e].to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(html: &str) -> Option<String> {
        formatted(Some(&FormattedBody::html(html)))
    }

    #[test]
    fn linkifies_plain_urls() {
        assert_eq!(linkify("no links here"), None);
        assert_eq!(
            linkify("see https://example.org/a?b=1&c=2. <ok>").as_deref(),
            Some(
                "see <a href=\"https://example.org/a?b=1&amp;c=2\" rel=\"noopener noreferrer\">\
                 https://example.org/a?b=1&amp;c=2</a>. &lt;ok&gt;"
            )
        );
        // Not a link start inside a word; parentheses kept only when balanced.
        assert_eq!(find_urls("xhttp://a.b"), vec![]);
        let text = "(https://en.wikipedia.org/wiki/Rust_(language))";
        assert_eq!(
            &text[find_urls(text)[0].0..find_urls(text)[0].1],
            "https://en.wikipedia.org/wiki/Rust_(language)"
        );
    }

    #[test]
    fn first_link_skips_quotes() {
        assert_eq!(
            first_link("> <@a:b> https://old.example\nlook http://new.example/x!").as_deref(),
            Some("http://new.example/x")
        );
        assert_eq!(first_link("nothing"), None);
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

#[cfg(test)]
mod emoticon_tests {
    use matrix_sdk::ruma::events::room::message::FormattedBody;

    fn clean(html: &str) -> String {
        super::formatted(Some(&FormattedBody::html(html))).unwrap_or_default()
    }

    #[test]
    fn keeps_custom_emoji() {
        let out = clean(
            r#"hi <img data-mx-emoticon src="mxc://x.org/abc" alt=":blob:" title=":blob:" height="32">"#,
        );
        assert!(out.contains(r#"src="mxc://x.org/abc""#), "{out}");
        assert!(out.contains("data-mx-emoticon"), "{out}");
        assert!(out.contains(r#"height="32""#), "{out}");
    }

    #[test]
    fn drops_outside_images_and_scripts() {
        let out = clean(
            r#"<img src="https://tracker.example/p.gif"><img src="mxc://x/y" onerror="alert(1)" height="9999">"#,
        );
        assert!(!out.contains("tracker"), "{out}");
        assert!(!out.contains("onerror"), "{out}");
        assert!(out.contains(r#"height="256""#), "{out}");
        let link = clean(r#"<a href="mxc://x/y">x</a>"#);
        assert!(!link.contains("mxc"), "{link}");
    }
}
