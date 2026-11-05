//! Local message search and the "Shared media" browser, over every chat including encrypted
//! ones (the server can't search those).
//!
//! Messages go into SQLite FTS5 (`store/search.sqlite3`, removed on sign-out) as they arrive
//! through sync, and a background backfill reads older history with `/messages` (which
//! decrypts), a few pages per chat in turn, resuming where it stopped on the next run.
//! The file is not encrypted: it relies on the app's private storage, like the event cache.
//!
//! Two full-text tables over the same rows: words (accents and case folded, every word
//! matched as a prefix, so results come while typing) and trigrams (matches inside words).

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use matrix_sdk::{
    Client, Room,
    room::MessagesOptions,
    ruma::{
        OwnedRoomId, UInt,
        events::{
            AnySyncTimelineEvent,
            room::message::{
                AudioMessageEventContent, FileMessageEventContent, ImageMessageEventContent,
                VideoMessageEventContent,
            },
        },
        serde::Raw,
    },
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::{
    Error, Result,
    media::{EventMedia, MediaRegistry},
};

#[cfg(not(target_family = "wasm"))]
const DB_FILE: &str = "search.sqlite3";
/// Events per `/messages` request while backfilling.
const PAGE: u32 = 100;
/// Pages read from one chat before moving on to the next (round-robin).
const PAGES_PER_TURN: usize = 5;
/// Pause between backfill requests, to stay light on the server and the battery.
const PAUSE: Duration = Duration::from_millis(400);
/// Big public rooms only get their recent history indexed.
const BIG_ROOM_MEMBERS: u64 = 200;
const BIG_ROOM_MAX_EVENTS: i64 = 3_000;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS msg (
    id INTEGER PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    room_id TEXT NOT NULL,
    sender TEXT NOT NULL,
    ts INTEGER NOT NULL,
    kind TEXT NOT NULL,
    body TEXT NOT NULL,
    link INTEGER NOT NULL DEFAULT 0,
    media TEXT
);
CREATE INDEX IF NOT EXISTS msg_room_ts ON msg(room_id, ts);
CREATE INDEX IF NOT EXISTS msg_ts ON msg(ts);
CREATE VIRTUAL TABLE IF NOT EXISTS msg_words USING fts5(
    body, content='msg', content_rowid='id', tokenize='unicode61 remove_diacritics 2'
);
CREATE VIRTUAL TABLE IF NOT EXISTS msg_tri USING fts5(
    body, content='msg', content_rowid='id', tokenize='trigram remove_diacritics 1'
);
CREATE TRIGGER IF NOT EXISTS msg_ai AFTER INSERT ON msg BEGIN
    INSERT INTO msg_words(rowid, body) VALUES (new.id, new.body);
    INSERT INTO msg_tri(rowid, body) VALUES (new.id, new.body);
END;
CREATE TRIGGER IF NOT EXISTS msg_ad AFTER DELETE ON msg BEGIN
    INSERT INTO msg_words(msg_words, rowid, body) VALUES ('delete', old.id, old.body);
    INSERT INTO msg_tri(msg_tri, rowid, body) VALUES ('delete', old.id, old.body);
END;
CREATE TRIGGER IF NOT EXISTS msg_au AFTER UPDATE ON msg BEGIN
    INSERT INTO msg_words(msg_words, rowid, body) VALUES ('delete', old.id, old.body);
    INSERT INTO msg_tri(msg_tri, rowid, body) VALUES ('delete', old.id, old.body);
    INSERT INTO msg_words(rowid, body) VALUES (new.id, new.body);
    INSERT INTO msg_tri(rowid, body) VALUES (new.id, new.body);
END;
CREATE TABLE IF NOT EXISTS backfill (
    room_id TEXT PRIMARY KEY,
    token TEXT,
    done INTEGER NOT NULL DEFAULT 0,
    events INTEGER NOT NULL DEFAULT 0
);
-- Encrypted messages we couldn't read yet (their key hadn't arrived: a new device fetches keys
-- from the backup a little after the first sync). Indexed once the key comes.
CREATE TABLE IF NOT EXISTS utd (
    event_id TEXT PRIMARY KEY,
    room_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    raw TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS utd_session ON utd (room_id, session_id);
";

/// What a message is, for filters and the shared media tabs.
#[derive(Debug, Clone, PartialEq)]
struct Indexable {
    event_id: String,
    sender: String,
    ts: i64,
    kind: &'static str,
    body: String,
    link: bool,
    /// The message content, for media (to show it again without the timeline).
    media: Option<Value>,
}

#[derive(Debug, PartialEq)]
enum Change {
    Add(Indexable),
    /// An edit: the original's new text (and content).
    Replace {
        target: String,
        body: String,
        kind: &'static str,
        link: bool,
        media: Option<Value>,
    },
    Remove(String),
}

/// Strip the legacy reply fallback (`> <@user> quoted…` lines then a blank line).
fn strip_reply_fallback(body: &str) -> &str {
    if !body.starts_with("> ") {
        return body;
    }
    match body.find("\n\n") {
        Some(i) if body[..i].lines().all(|l| l.starts_with('>')) => &body[i + 2..],
        _ => body,
    }
}

/// Searchable text and kind of an `m.room.message` content.
fn describe(content: &Value) -> Option<(&'static str, String, bool, Option<Value>)> {
    let body = content.get("body")?.as_str()?;
    let msgtype = content.get("msgtype")?.as_str()?;
    let media_text = || {
        let filename = content.get("filename").and_then(Value::as_str);
        match filename {
            Some(f) if f != body => format!("{f} {body}"),
            _ => body.to_owned(),
        }
    };
    Some(match msgtype {
        "m.image" => ("image", media_text(), false, Some(content.clone())),
        "m.video" => ("video", media_text(), false, Some(content.clone())),
        "m.file" => ("file", media_text(), false, Some(content.clone())),
        "m.audio" => {
            let voice = content.get("org.matrix.msc3245.voice").is_some()
                || content.get("m.voice").is_some();
            let kind = if voice { "voice" } else { "audio" };
            (kind, media_text(), false, Some(content.clone()))
        }
        "m.location" => ("location", body.to_owned(), false, None),
        _ => {
            let text = strip_reply_fallback(body).to_owned();
            let link = crate::html::first_link(&text).is_some();
            ("text", text, link, None)
        }
    })
}

/// What an event changes in the index, if anything.
fn change_of(raw: &Raw<AnySyncTimelineEvent>) -> Option<Change> {
    let event: Value = raw.deserialize_as().ok()?;
    let kind = event.get("type")?.as_str()?;
    let event_id = event.get("event_id")?.as_str()?.to_owned();
    let content = event.get("content")?;
    if kind == "m.room.redaction" {
        let target = content
            .get("redacts")
            .or_else(|| event.get("redacts"))?
            .as_str()?;
        return Some(Change::Remove(target.to_owned()));
    }
    // Redacted events arrive with an empty content.
    if event.pointer("/unsigned/redacted_because").is_some() {
        return Some(Change::Remove(event_id));
    }
    let sender = event.get("sender")?.as_str()?.to_owned();
    let ts = event.get("origin_server_ts")?.as_i64()?;
    let (kind, body, link, media) = match kind {
        "m.room.message" => {
            if content
                .pointer("/m.relates_to/rel_type")
                .and_then(Value::as_str)
                == Some("m.replace")
            {
                let target = content.pointer("/m.relates_to/event_id")?.as_str()?;
                let (kind, body, link, media) = describe(content.get("m.new_content")?)?;
                return Some(Change::Replace {
                    target: target.to_owned(),
                    body,
                    kind,
                    link,
                    media,
                });
            }
            describe(content)?
        }
        "m.sticker" => (
            "sticker",
            content.get("body")?.as_str()?.to_owned(),
            false,
            None,
        ),
        "org.matrix.msc3381.poll.start" | "m.poll.start" => {
            let poll = content
                .get("org.matrix.msc3381.poll.start")
                .or_else(|| content.get("m.poll"))?;
            let text = |v: &Value| -> Option<String> {
                v.get("org.matrix.msc1767.text")
                    .or_else(|| v.get("m.text"))
                    .and_then(|t| match t {
                        Value::String(s) => Some(s.clone()),
                        Value::Array(a) => a.first()?.get("body")?.as_str().map(str::to_owned),
                        _ => None,
                    })
            };
            let mut body = text(poll.get("question")?)?;
            for answer in poll.get("answers")?.as_array()? {
                if let Some(t) = text(answer) {
                    body.push('\n');
                    body.push_str(&t);
                }
            }
            ("poll", body, false, None)
        }
        _ => return None,
    };
    Some(Change::Add(Indexable {
        event_id,
        sender,
        ts,
        kind,
        body,
        link,
        media,
    }))
}

/// FTS5 query: every word must appear, each as a prefix (so results come while typing).
fn word_query(input: &str) -> Option<String> {
    let words: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

/// Trigram query (matches inside words): the words of 3+ characters, all required.
fn trigram_query(input: &str) -> Option<String> {
    let words: Vec<String> = input
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.chars().count() >= 3)
        .map(|w| format!("\"{}\"", w.replace('"', "")))
        .collect();
    (!words.is_empty()).then(|| words.join(" AND "))
}

/// Narrows a search or a shared media listing.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KindFilter {
    /// Photos and videos.
    Media,
    /// Files, audio and voice messages.
    Files,
    /// Messages with a link.
    Links,
}

impl KindFilter {
    fn sql(self) -> &'static str {
        match self {
            Self::Media => "m.kind IN ('image', 'video')",
            Self::Files => "m.kind IN ('file', 'audio', 'voice')",
            Self::Links => "m.link = 1",
        }
    }
}

/// The index file and its backfill state.
pub(crate) struct SearchIndex {
    db: Mutex<Connection>,
    /// The chat to backfill first (being searched or browsed).
    priority: Mutex<Option<OwnedRoomId>>,
    running: std::sync::atomic::AtomicBool,
}

struct Row {
    event_id: String,
    room_id: String,
    sender: String,
    ts: i64,
    kind: String,
    snippet: String,
    media: Option<String>,
}

impl SearchIndex {
    #[cfg_attr(target_family = "wasm", allow(unused_variables))]
    pub(crate) fn open(store_dir: &Path) -> Result<Self> {
        // In a browser there are no files: the index lives in memory for now (rebuilt by the
        // backfill on each visit).
        #[cfg(target_family = "wasm")]
        let db = Connection::open_in_memory().map_err(sql)?;
        #[cfg(not(target_family = "wasm"))]
        let db = {
            std::fs::create_dir_all(store_dir)?;
            Connection::open(store_dir.join(DB_FILE)).map_err(sql)?
        };
        db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
            .map_err(sql)?;
        // Indexes made before undecrypted messages were kept had skipped them for good: walk
        // the history once more (already indexed messages are only counted again).
        let had_utd = db
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'utd'",
                [],
                |_| Ok(()),
            )
            .optional()
            .map_err(sql)?
            .is_some();
        db.execute_batch(SCHEMA).map_err(sql)?;
        if !had_utd {
            db.execute("UPDATE backfill SET done = 0, token = NULL, events = 0", [])
                .map_err(sql)?;
        }
        Ok(Self {
            db: Mutex::new(db),
            priority: Mutex::default(),
            running: Default::default(),
        })
    }

    /// Messages still waiting for their key: all of them, or those of one key.
    fn undecrypted_rows(&self, key: Option<(&str, &str)>) -> Result<Vec<(String, String)>> {
        let db = self.db.lock().unwrap();
        let rows = match key {
            Some((room, session)) => db
                .prepare("SELECT room_id, raw FROM utd WHERE room_id = ?1 AND session_id = ?2")
                .and_then(|mut q| {
                    q.query_map(params![room, session], |r| Ok((r.get(0)?, r.get(1)?)))?
                        .collect::<rusqlite::Result<Vec<_>>>()
                }),
            None => db
                .prepare("SELECT room_id, raw FROM utd ORDER BY rowid DESC LIMIT 2000")
                .and_then(|mut q| {
                    q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                        .collect::<rusqlite::Result<Vec<_>>>()
                }),
        };
        rows.map_err(sql)
    }

    /// Apply events (one transaction). Returns how many were already indexed.
    fn apply(&self, room_id: &str, events: &[&Raw<AnySyncTimelineEvent>]) -> Result<usize> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction().map_err(sql)?;
        let mut known = 0;
        for raw in events {
            if let Some((event_id, session_id)) = undecrypted(raw) {
                tx.execute(
                    "INSERT OR IGNORE INTO utd (event_id, room_id, session_id, raw)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![event_id, room_id, session_id, raw.json().get()],
                )
                .map_err(sql)?;
                continue;
            }
            let Some(change) = change_of(raw) else {
                continue;
            };
            match change {
                Change::Add(m) => {
                    let inserted = tx
                        .execute(
                            "INSERT OR IGNORE INTO msg (event_id, room_id, sender, ts, kind, body, link, media)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                            params![
                                m.event_id,
                                room_id,
                                m.sender,
                                m.ts,
                                m.kind,
                                m.body,
                                m.link,
                                m.media.map(|v| v.to_string())
                            ],
                        )
                        .map_err(sql)?;
                    if inserted == 0 {
                        known += 1;
                    }
                    tx.execute("DELETE FROM utd WHERE event_id = ?1", params![m.event_id])
                        .map_err(sql)?;
                }
                Change::Replace {
                    target,
                    body,
                    kind,
                    link,
                    media,
                } => {
                    tx.execute(
                        "UPDATE msg SET body = ?2, kind = ?3, link = ?4, media = COALESCE(?5, media)
                         WHERE event_id = ?1",
                        params![target, body, kind, link, media.map(|v| v.to_string())],
                    )
                    .map_err(sql)?;
                }
                Change::Remove(target) => {
                    tx.execute("DELETE FROM msg WHERE event_id = ?1", params![target])
                        .map_err(sql)?;
                }
            }
        }
        tx.commit().map_err(sql)?;
        Ok(known)
    }

    fn state(&self, room_id: &str) -> Result<(Option<String>, bool, i64, bool)> {
        let db = self.db.lock().unwrap();
        let row = db
            .query_row(
                "SELECT token, done, events FROM backfill WHERE room_id = ?1",
                params![room_id],
                |r| Ok((r.get(0)?, r.get::<_, bool>(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(sql)?;
        Ok(match row {
            Some((token, done, events)) => (token, done, events, true),
            None => (None, false, 0, false),
        })
    }

    fn save_state(
        &self,
        room_id: &str,
        token: Option<&str>,
        done: bool,
        events: i64,
    ) -> Result<()> {
        self.db
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO backfill (room_id, token, done, events) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(room_id) DO UPDATE SET token = ?2, done = ?3, events = ?4",
                params![room_id, token, done, events],
            )
            .map_err(sql)?;
        Ok(())
    }

    fn count(&self) -> Result<u64> {
        self.db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM msg", [], |r| r.get::<_, i64>(0))
            .map(|n| n as u64)
            .map_err(sql)
    }

    fn rooms_done(&self) -> Result<u64> {
        self.db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM backfill WHERE done = 1", [], |r| {
                r.get::<_, i64>(0)
            })
            .map(|n| n as u64)
            .map_err(sql)
    }

    fn room_done(&self, room_id: &str) -> bool {
        self.state(room_id).map(|s| s.1).unwrap_or(false)
    }

    /// Matches, newest first: word matches, then (if there's room) matches inside words.
    fn search(
        &self,
        query: &str,
        room_id: Option<&str>,
        sender: Option<&str>,
        kind: Option<KindFilter>,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<Row>> {
        let mut rows = Vec::new();
        for (table, fts) in [
            ("msg_words", word_query(query)),
            ("msg_tri", trigram_query(query)),
        ] {
            let Some(fts) = fts else { continue };
            if rows.len() >= limit {
                break;
            }
            let mut filters = vec![format!("{table} MATCH ?1")];
            if room_id.is_some() {
                filters.push("m.room_id = ?2".into());
            }
            if sender.is_some() {
                filters.push("m.sender = ?3".into());
            }
            if before.is_some() {
                filters.push("m.ts < ?4".into());
            }
            if let Some(k) = kind {
                filters.push(k.sql().into());
            }
            let sql_text = format!(
                "SELECT m.event_id, m.room_id, m.sender, m.ts, m.kind,
                        snippet({table}, 0, char(2), char(3), '…', 16), m.media
                 FROM {table} JOIN msg m ON m.id = {table}.rowid
                 WHERE {} ORDER BY m.ts DESC LIMIT ?5",
                filters.join(" AND ")
            );
            let db = self.db.lock().unwrap();
            let mut stmt = db.prepare(&sql_text).map_err(sql)?;
            let found = stmt
                .query_map(params![fts, room_id, sender, before, limit as i64], |r| {
                    Ok(Row {
                        event_id: r.get(0)?,
                        room_id: r.get(1)?,
                        sender: r.get(2)?,
                        ts: r.get(3)?,
                        kind: r.get(4)?,
                        snippet: r.get(5)?,
                        media: r.get(6)?,
                    })
                })
                .map_err(sql)?;
            for row in found {
                let row = row.map_err(sql)?;
                if rows.len() < limit && !rows.iter().any(|r: &Row| r.event_id == row.event_id) {
                    rows.push(row);
                }
            }
        }
        Ok(rows)
    }

    /// A chat's photos/videos, files or links, newest first.
    fn listing(
        &self,
        room_id: &str,
        kind: KindFilter,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<Row>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db
            .prepare(&format!(
                "SELECT m.event_id, m.room_id, m.sender, m.ts, m.kind, m.body, m.media FROM msg m
                 WHERE m.room_id = ?1 AND {} AND (?2 IS NULL OR m.ts < ?2)
                 ORDER BY m.ts DESC LIMIT ?3",
                kind.sql()
            ))
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![room_id, before, limit as i64], |r| {
                Ok(Row {
                    event_id: r.get(0)?,
                    room_id: r.get(1)?,
                    sender: r.get(2)?,
                    ts: r.get(3)?,
                    kind: r.get(4)?,
                    snippet: r.get(5)?,
                    media: r.get(6)?,
                })
            })
            .map_err(sql)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql)?;
        Ok(rows)
    }
}

impl SearchIndex {
    /// Messages per UTC day in a chat: (day start in ms, ours, others').
    fn activity(&self, room_id: &str, me: &str) -> Result<(Vec<DayCount>, bool)> {
        let db = self.db.lock().unwrap();
        let mut stmt = db
            .prepare(
                "SELECT ts / 86400000, SUM(sender = ?2), SUM(sender <> ?2) FROM msg
                 WHERE room_id = ?1 GROUP BY ts / 86400000 ORDER BY 1",
            )
            .map_err(sql)?;
        let days = stmt
            .query_map(params![room_id, me], |r| {
                Ok(DayCount {
                    day: r.get::<_, i64>(0)? * 86_400_000,
                    mine: r.get(1)?,
                    others: r.get(2)?,
                })
            })
            .map_err(sql)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql)?;
        let complete = db
            .query_row(
                "SELECT done FROM backfill WHERE room_id = ?1",
                params![room_id],
                |r| r.get::<_, bool>(0),
            )
            .unwrap_or(false);
        Ok((days, complete))
    }

    /// The first indexed message at or after `ts`.
    fn first_at(&self, room_id: &str, ts: i64) -> Result<Option<String>> {
        let db = self.db.lock().unwrap();
        db.query_row(
            "SELECT event_id FROM msg WHERE room_id = ?1 AND ts >= ?2 ORDER BY ts LIMIT 1",
            params![room_id, ts],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(sql(e)),
        })
    }
}

/// Messages sent on one (UTC) day.
#[derive(Debug, Clone, Serialize)]
pub struct DayCount {
    /// Start of the day, ms since epoch.
    pub day: i64,
    pub mine: u32,
    pub others: u32,
}

/// A chat's message history over time, for its "History" graph.
#[derive(Debug, Clone, Serialize)]
pub struct Activity {
    pub days: Vec<DayCount>,
    /// The whole history is indexed (else older messages are still being fetched).
    pub complete: bool,
}

fn sql(e: rusqlite::Error) -> Error {
    Error::Search(e.to_string())
}

/// A message found by search.
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub room_id: String,
    pub room_name: String,
    pub room_avatar: Option<String>,
    pub event_id: String,
    pub sender: String,
    pub sender_name: String,
    pub is_own: bool,
    /// Milliseconds since the Unix epoch.
    pub timestamp: i64,
    /// text, image, video, file, audio, voice, location, sticker or poll.
    pub kind: String,
    /// The matching part, with matches between `\u{2}` and `\u{3}`.
    pub snippet: String,
    /// Key for `media://localhost/_event/<key>` (photos and videos).
    pub media: Option<String>,
}

/// One item of a chat's shared photos/videos, files or links.
#[derive(Debug, Clone, Serialize)]
pub struct SharedItem {
    pub event_id: String,
    pub sender_name: String,
    pub timestamp: i64,
    pub kind: String,
    /// Media key (photos, videos, files), see [`SearchHit::media`].
    pub media: Option<String>,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub duration_ms: Option<u64>,
    pub filename: Option<String>,
    pub size: Option<u64>,
    /// Links: the message text and its first URL.
    pub text: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchStatus {
    pub messages: u64,
    pub chats_done: u64,
    pub chats: u64,
    /// The backfill is still reading older history.
    pub indexing: bool,
    /// The chat asked about has all its history indexed.
    pub chat_done: Option<bool>,
}

/// Register the media of an indexed message so the UI can show it; returns its key.
fn register_media(registry: &MediaRegistry, event_id: &str, content: &Value) -> Option<String> {
    let key = format!("idx{event_id}");
    let media = match content.get("msgtype")?.as_str()? {
        "m.image" => {
            let c: ImageMessageEventContent = serde_json::from_value(content.clone()).ok()?;
            let info = c.info.as_deref();
            EventMedia {
                thumbnail: info.and_then(|i| i.thumbnail_source.clone()),
                mimetype: info.and_then(|i| i.mimetype.clone()),
                filename: c.filename().to_owned(),
                source: c.source,
            }
        }
        "m.video" => {
            let c: VideoMessageEventContent = serde_json::from_value(content.clone()).ok()?;
            let info = c.info.as_deref();
            EventMedia {
                thumbnail: info.and_then(|i| i.thumbnail_source.clone()),
                mimetype: info.and_then(|i| i.mimetype.clone()),
                filename: c.filename().to_owned(),
                source: c.source,
            }
        }
        "m.audio" => {
            let c: AudioMessageEventContent = serde_json::from_value(content.clone()).ok()?;
            EventMedia {
                thumbnail: None,
                mimetype: c.info.as_deref().and_then(|i| i.mimetype.clone()),
                filename: c.filename().to_owned(),
                source: c.source,
            }
        }
        "m.file" => {
            let c: FileMessageEventContent = serde_json::from_value(content.clone()).ok()?;
            EventMedia {
                thumbnail: None,
                mimetype: c.info.as_deref().and_then(|i| i.mimetype.clone()),
                filename: c.filename().to_owned(),
                source: c.source,
            }
        }
        _ => return None,
    };
    registry.insert(key.clone(), media);
    Some(key)
}

/// The chat's avatar, or the other person's in a 1:1 chat.
async fn room_avatar(room: &Room) -> Option<String> {
    let mut avatar = room.avatar_url();
    if avatar.is_none() && room.is_direct().await.unwrap_or(false) {
        avatar = room.heroes().await.into_iter().find_map(|h| h.avatar_url);
    }
    avatar.map(|u| u.to_string())
}

/// Display names, looked up once per sender and room.
struct Names(HashMap<(String, String), String>);

impl Names {
    async fn get(&mut self, room: Option<&Room>, user: &str) -> String {
        let key = (
            room.map(|r| r.room_id().to_string()).unwrap_or_default(),
            user.to_owned(),
        );
        if let Some(name) = self.0.get(&key) {
            return name.clone();
        }
        let fallback = user
            .trim_start_matches('@')
            .split(':')
            .next()
            .unwrap_or(user)
            .to_owned();
        let name = match (room, matrix_sdk::ruma::UserId::parse(user)) {
            (Some(room), Ok(id)) => room
                .get_member_no_sync(&id)
                .await
                .ok()
                .flatten()
                .and_then(|m| m.display_name().map(str::to_owned))
                .unwrap_or(fallback),
            _ => fallback,
        };
        self.0.insert(key, name.clone());
        name
    }
}

impl crate::Core {
    fn search_index(&self) -> Result<Arc<SearchIndex>> {
        self.with_active(|a| a.search.clone())?
            .ok_or_else(|| Error::Search("search isn't available".into()))
    }

    /// Search messages in all chats, or one. Newest first; `before` (a timestamp) pages on.
    pub async fn search_messages(
        &self,
        query: &str,
        room_id: Option<&str>,
        sender: Option<&str>,
        kind: Option<KindFilter>,
        before: Option<i64>,
    ) -> Result<Vec<SearchHit>> {
        let index = self.search_index()?;
        if let Some(room) = room_id {
            *index.priority.lock().unwrap() = OwnedRoomId::try_from(room).ok();
        }
        let client = self.client()?;
        let own = client.user_id().map(|u| u.to_string()).unwrap_or_default();
        let (query, room_id, sender) = (
            query.to_owned(),
            room_id.map(str::to_owned),
            sender.map(str::to_owned),
        );
        let search = index.clone();
        let rows = crate::rt::blocking(move || {
            search.search(
                &query,
                room_id.as_deref(),
                sender.as_deref(),
                kind,
                before,
                40,
            )
        })
        .await
        .map_err(|e| Error::Search(e.to_string()))??;

        let mut names = Names(HashMap::new());
        let mut hits = Vec::with_capacity(rows.len());
        for row in rows {
            let room = OwnedRoomId::try_from(row.room_id.as_str())
                .ok()
                .and_then(|id| client.get_room(&id));
            // Left chats stay in the index but aren't shown.
            let Some(room) = room else { continue };
            let media = row
                .media
                .as_deref()
                .filter(|_| matches!(row.kind.as_str(), "image" | "video"))
                .and_then(|m| serde_json::from_str::<Value>(m).ok())
                .and_then(|m| register_media(&self.media, &row.event_id, &m));
            hits.push(SearchHit {
                room_name: room
                    .cached_display_name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| row.room_id.clone()),
                room_avatar: room_avatar(&room).await,
                sender_name: names.get(Some(&room), &row.sender).await,
                is_own: row.sender == own,
                room_id: row.room_id,
                event_id: row.event_id,
                sender: row.sender,
                timestamp: row.ts,
                kind: row.kind,
                snippet: row.snippet,
                media,
            });
        }
        Ok(hits)
    }

    /// A chat's photos and videos, files or links, newest first (`before` pages on).
    /// How many messages per day in a chat (from the local index: whole history once the
    /// background backfill has reached it; it's prioritised while this is asked).
    pub async fn message_activity(&self, room_id: &str) -> Result<Activity> {
        let index = self.search_index()?;
        *index.priority.lock().unwrap() = OwnedRoomId::try_from(room_id).ok();
        let me = self
            .client()?
            .user_id()
            .map(ToString::to_string)
            .unwrap_or_default();
        let room_id = room_id.to_owned();
        let (days, complete) = crate::rt::blocking(move || index.activity(&room_id, &me))
            .await
            .map_err(|e| Error::Search(e.to_string()))??;
        Ok(Activity { days, complete })
    }

    /// The first message on or after `ts` (ms), to jump there.
    pub async fn message_at(&self, room_id: &str, ts: i64) -> Result<Option<String>> {
        let index = self.search_index()?;
        let room_id = room_id.to_owned();
        crate::rt::blocking(move || index.first_at(&room_id, ts))
            .await
            .map_err(|e| Error::Search(e.to_string()))?
    }

    pub async fn shared_media(
        &self,
        room_id: &str,
        kind: KindFilter,
        before: Option<i64>,
    ) -> Result<Vec<SharedItem>> {
        let index = self.search_index()?;
        *index.priority.lock().unwrap() = OwnedRoomId::try_from(room_id).ok();
        let room = self.room(room_id)?;
        let rows = {
            let (index, room_id) = (index.clone(), room_id.to_owned());
            crate::rt::blocking(move || index.listing(&room_id, kind, before, 60))
                .await
                .map_err(|e| Error::Search(e.to_string()))??
        };
        let mut names = Names(HashMap::new());
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let content = row
                .media
                .as_deref()
                .and_then(|m| serde_json::from_str::<Value>(m).ok());
            let info = content.as_ref().and_then(|c| c.get("info"));
            let num = |key: &str| info.and_then(|i| i.get(key)).and_then(Value::as_u64);
            let url = (kind == KindFilter::Links)
                .then(|| crate::html::first_link(&row.snippet))
                .flatten();
            items.push(SharedItem {
                media: content
                    .as_ref()
                    .and_then(|c| register_media(&self.media, &row.event_id, c)),
                width: num("w"),
                height: num("h"),
                duration_ms: num("duration"),
                size: num("size"),
                filename: content.as_ref().and_then(|c| {
                    c.get("filename")
                        .or_else(|| c.get("body"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                }),
                text: (kind == KindFilter::Links).then(|| row.snippet.clone()),
                url,
                sender_name: names.get(Some(&room), &row.sender).await,
                event_id: row.event_id,
                timestamp: row.ts,
                kind: row.kind,
            });
        }
        Ok(items)
    }

    /// How far the index got; `room_id` also says whether that chat is complete.
    pub fn search_status(&self, room_id: Option<&str>) -> Result<SearchStatus> {
        let index = self.search_index()?;
        let client = self.client()?;
        Ok(SearchStatus {
            messages: index.count()?,
            chats_done: index.rooms_done()?,
            chats: client
                .joined_rooms()
                .iter()
                .filter(|r| !r.is_space())
                .count() as u64,
            indexing: index.running.load(std::sync::atomic::Ordering::Relaxed),
            chat_done: room_id.map(|r| index.room_done(r)),
        })
    }

    /// Start reading older history into the index (once per session; the UI calls it).
    /// Needs a Tokio runtime (call it from async code).
    pub fn start_search_backfill(&self) -> Result<()> {
        let index = self.search_index()?;
        if index
            .running
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            return Ok(());
        }
        let client = self.client()?;
        let task = crate::rt::spawn(async move {
            if let Err(e) = backfill(&client, &index).await {
                warn!("search backfill stopped: {e}");
            }
            index
                .running
                .store(false, std::sync::atomic::Ordering::Relaxed);
        });
        self.with_active(|a| a.search_task = Some(task.abort_handle()))
    }
}

/// An encrypted event that wasn't decrypted: its event ID and Megolm session.
fn undecrypted(raw: &Raw<AnySyncTimelineEvent>) -> Option<(String, String)> {
    if raw.get_field::<String>("type").ok()??.as_str() != "m.room.encrypted" {
        return None;
    }
    let event_id = raw.get_field::<String>("event_id").ok()??;
    let content: Value = raw.get_field("content").ok()??;
    let session_id = content["session_id"].as_str()?.to_owned();
    Some((event_id, session_id))
}

/// Try again the messages that couldn't be read (all, or those of the keys just received),
/// and index the ones that open now. Asking also fetches missing keys from the backup.
async fn retry_undecrypted(
    client: &Client,
    index: &Arc<SearchIndex>,
    keys: Option<Vec<(String, String)>>,
) {
    let rows = match keys {
        None => index.undecrypted_rows(None),
        Some(keys) => {
            let mut rows = Vec::new();
            for (room, session) in &keys {
                match index.undecrypted_rows(Some((room, session))) {
                    Ok(found) => rows.extend(found),
                    Err(e) => return warn!("search: undecrypted messages: {e}"),
                }
            }
            Ok(rows)
        }
    };
    let rows = match rows {
        Ok(rows) => rows,
        Err(e) => return warn!("search: undecrypted messages: {e}"),
    };
    let mut opened = 0usize;
    for (room_id, json) in rows {
        let Some(room) = OwnedRoomId::try_from(room_id.as_str())
            .ok()
            .and_then(|id| client.get_room(&id))
        else {
            continue;
        };
        let Ok(raw) = Raw::<AnySyncTimelineEvent>::from_json_string(json) else {
            continue;
        };
        let Ok(event) = room.decrypt_event(&raw.cast_unchecked(), None).await else {
            continue;
        };
        if matches!(
            event.kind,
            matrix_sdk::deserialized_responses::TimelineEventKind::UnableToDecrypt { .. }
        ) {
            continue;
        }
        let decrypted = event.raw().clone();
        let index = index.clone();
        let applied = crate::rt::blocking(move || index.apply(&room_id, &[&decrypted])).await;
        if matches!(applied, Ok(Ok(_))) {
            opened += 1;
        }
    }
    if opened > 0 {
        info!(opened, "search: indexed messages whose key arrived");
    }
}

/// Open the index and keep it fed from sync (called once per signed-in client).
pub(crate) fn install(client: &Client, store_dir: &Path) -> Option<Arc<SearchIndex>> {
    let index = match SearchIndex::open(store_dir) {
        Ok(index) => Arc::new(index),
        Err(e) => {
            warn!("message search unavailable: {e}");
            return None;
        }
    };
    // Keys arriving later (backup download, forwarded keys): index what they open.
    let keyed = index.clone();
    let keys_client = client.clone();
    crate::rt::spawn(async move {
        use futures_util::StreamExt;
        let Some(mut stream) = keys_client.encryption().room_keys_received_stream().await else {
            return;
        };
        while let Some(batch) = stream.next().await {
            let Ok(batch) = batch else { continue };
            let keys = batch
                .into_iter()
                .map(|k| (k.room_id.to_string(), k.session_id))
                .collect();
            retry_undecrypted(&keys_client, &keyed, Some(keys)).await;
        }
    });
    let live = index.clone();
    client.add_event_handler(move |event: Raw<AnySyncTimelineEvent>, room: Room| {
        let index = live.clone();
        async move {
            let room_id = room.room_id().to_string();
            let _ = crate::rt::blocking(move || {
                if let Err(e) = index.apply(&room_id, &[&event]) {
                    warn!("couldn't index a message: {e}");
                }
            })
            .await;
        }
    });
    Some(index)
}

/// One `/messages` page backwards: the next token (None at the start of the room), how many
/// events it had and how many were already indexed. `Ok(None)`: we may not read this history.
async fn page(
    room: &Room,
    index: &Arc<SearchIndex>,
    from: Option<&str>,
) -> Result<Option<(Option<String>, usize, usize)>> {
    let mut options = MessagesOptions::backward().from(from);
    options.limit = UInt::from(PAGE);
    let messages = match room.messages(options).await {
        Ok(messages) => messages,
        Err(e) => {
            let status = e.as_client_api_error().map(|c| c.status_code.as_u16());
            if matches!(status, Some(403 | 404)) {
                return Ok(None);
            }
            return Err(e.into());
        }
    };
    let raws: Vec<Raw<AnySyncTimelineEvent>> =
        messages.chunk.iter().map(|e| e.raw().clone()).collect();
    let count = raws.len();
    let room_id = room.room_id().to_string();
    let index = index.clone();
    let known =
        crate::rt::blocking(move || index.apply(&room_id, &raws.iter().collect::<Vec<_>>()))
            .await
            .map_err(|e| Error::Search(e.to_string()))??;
    let next = messages.end.filter(|_| count > 0);
    Ok(Some((next, count, known)))
}

async fn backfill(client: &Client, index: &Arc<SearchIndex>) -> Result<()> {
    let mut rooms = client.joined_rooms();
    rooms.retain(|r| !r.is_space());
    // Most recently active chats first.
    rooms.sort_by_key(|r| std::cmp::Reverse(r.recency_stamp().map(u64::from)));
    info!(chats = rooms.len(), "search backfill starting");

    // Catch up on what arrived while we were away (sync only brings the last few events of
    // each chat): from the newest back to the first message already indexed.
    for room in &rooms {
        let (_, _, _, started) = index.state(room.room_id().as_str())?;
        if !started {
            continue;
        }
        let mut from: Option<String> = None;
        for _ in 0..20 {
            let Some((next, count, known)) = page(room, index, from.as_deref()).await? else {
                break;
            };
            crate::rt::sleep(PAUSE).await;
            if known > 0 || count == 0 || next.is_none() {
                break;
            }
            from = next;
        }
    }

    // Then older history, a few pages per chat in turn; a chat being searched goes first.
    loop {
        let mut pending: Vec<&Room> = Vec::new();
        for room in &rooms {
            if !index.state(room.room_id().as_str())?.1 {
                pending.push(room);
            }
        }
        if pending.is_empty() {
            break;
        }
        let priority = index.priority.lock().unwrap().take();
        if let Some(p) = priority
            && let Some(i) = pending.iter().position(|r| r.room_id() == p)
        {
            pending.swap(0, i);
        }
        for room in pending {
            let room_id = room.room_id().as_str();
            let max_events = if room.joined_members_count() > BIG_ROOM_MEMBERS {
                BIG_ROOM_MAX_EVENTS
            } else {
                i64::MAX
            };
            let (mut token, _, mut events, _) = index.state(room_id)?;
            for _ in 0..PAGES_PER_TURN {
                // Network errors end this run (the next one resumes from the saved token).
                let done = match page(room, index, token.as_deref()).await? {
                    Some((next, count, _)) => {
                        events += count as i64;
                        token = next;
                        token.is_none() || events >= max_events
                    }
                    // Not allowed to read this history: don't retry it forever.
                    None => {
                        debug!(room = room_id, "backfill: no access to history");
                        true
                    }
                };
                index.save_state(room_id, token.as_deref(), done, events)?;
                crate::rt::sleep(PAUSE).await;
                if done {
                    break;
                }
                // Someone is waiting on another chat: go there now.
                if index
                    .priority
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(|p| p != room.room_id())
                {
                    break;
                }
            }
            if index.priority.lock().unwrap().is_some() {
                break;
            }
        }
    }
    info!("search backfill complete");
    // Messages read before their key had arrived.
    retry_undecrypted(client, index, None).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(v: Value) -> Raw<AnySyncTimelineEvent> {
        Raw::new(&v).unwrap().cast_unchecked()
    }

    fn msg(id: &str, body: &str) -> Raw<AnySyncTimelineEvent> {
        raw(serde_json::json!({
            "type": "m.room.message", "event_id": id, "sender": "@a:x", "origin_server_ts": 1000,
            "content": { "msgtype": "m.text", "body": body }
        }))
    }

    fn index() -> SearchIndex {
        let dir = std::env::temp_dir().join(format!("alwy-search-{}", rand_suffix()));
        SearchIndex::open(&dir).unwrap()
    }

    fn rand_suffix() -> String {
        format!("{:?}", std::time::SystemTime::now()).replace(|c: char| !c.is_alphanumeric(), "")
    }

    fn found(index: &SearchIndex, q: &str) -> Vec<String> {
        index
            .search(q, None, None, None, None, 10)
            .unwrap()
            .into_iter()
            .map(|r| r.event_id)
            .collect()
    }

    #[test]
    fn queries() {
        assert_eq!(
            word_query("bonj  l'ami").as_deref(),
            Some("\"bonj\"* \"l\"* \"ami\"*")
        );
        assert_eq!(word_query(" ,; "), None);
        assert_eq!(
            trigram_query("ab partement").as_deref(),
            Some("\"partement\"")
        );
    }

    #[test]
    fn finds_by_prefix_accent_and_inside_words() {
        let index = index();
        index
            .apply(
                "!r:x",
                &[
                    &msg("$1", "On se voit au café demain ?"),
                    &msg("$2", "Le nouvel appartement est super"),
                ],
            )
            .unwrap();
        assert_eq!(found(&index, "cafe"), vec!["$1"]);
        assert_eq!(found(&index, "dem"), vec!["$1"]);
        assert_eq!(found(&index, "CAFÉ voit"), vec!["$1"]);
        assert_eq!(found(&index, "partement"), vec!["$2"]);
        assert!(found(&index, "pizza").is_empty());
        // Highlighted snippet.
        let hit = &index.search("cafe", None, None, None, None, 1).unwrap()[0];
        assert!(hit.snippet.contains("\u{2}café\u{3}"), "{}", hit.snippet);
    }

    #[test]
    fn edits_and_redactions() {
        let index = index();
        index.apply("!r:x", &[&msg("$1", "first text")]).unwrap();
        let edit = raw(serde_json::json!({
            "type": "m.room.message", "event_id": "$2", "sender": "@a:x", "origin_server_ts": 2000,
            "content": { "msgtype": "m.text", "body": "* second", "m.new_content": { "msgtype": "m.text", "body": "second text" },
                         "m.relates_to": { "rel_type": "m.replace", "event_id": "$1" } }
        }));
        index.apply("!r:x", &[&edit]).unwrap();
        assert!(found(&index, "first").is_empty());
        assert_eq!(found(&index, "second"), vec!["$1"]);
        let redaction = raw(serde_json::json!({
            "type": "m.room.redaction", "event_id": "$3", "sender": "@a:x", "origin_server_ts": 3000,
            "redacts": "$1", "content": { "redacts": "$1" }
        }));
        index.apply("!r:x", &[&redaction]).unwrap();
        assert!(found(&index, "second").is_empty());
        // Already indexed: counted as known (the catch-up stops there).
        index.apply("!r:x", &[&msg("$4", "x")]).unwrap();
        assert_eq!(index.apply("!r:x", &[&msg("$4", "x")]).unwrap(), 1);
    }

    #[test]
    fn undecrypted_waits_for_its_key() {
        let index = index();
        let encrypted = raw(serde_json::json!({
            "type": "m.room.encrypted", "event_id": "$e", "sender": "@a:x", "origin_server_ts": 1,
            "content": { "algorithm": "m.megolm.v1.aes-sha2", "ciphertext": "AAA", "session_id": "S1",
                         "sender_key": "K", "device_id": "D" }
        }));
        index.apply("!r:x", &[&encrypted]).unwrap();
        let waiting = index.undecrypted_rows(Some(("!r:x", "S1"))).unwrap();
        assert_eq!(waiting.len(), 1);
        assert!(
            index
                .undecrypted_rows(Some(("!r:x", "S2")))
                .unwrap()
                .is_empty()
        );
        // Decrypted later: indexed, and no longer waiting.
        index.apply("!r:x", &[&msg("$e", "photo du chat")]).unwrap();
        assert_eq!(found(&index, "chat"), vec!["$e"]);
        assert!(index.undecrypted_rows(None).unwrap().is_empty());
    }

    #[test]
    fn reply_fallback_is_not_indexed() {
        assert_eq!(
            strip_reply_fallback("> <@b:x> pizza ce soir ?\n\nOui !"),
            "Oui !"
        );
        assert_eq!(strip_reply_fallback("> citation"), "> citation");
    }

    #[test]
    fn media_kinds_and_links() {
        let index = index();
        let photo = raw(serde_json::json!({
            "type": "m.room.message", "event_id": "$p", "sender": "@a:x", "origin_server_ts": 1,
            "content": { "msgtype": "m.image", "body": "vacances.jpg", "url": "mxc://x/y" }
        }));
        index
            .apply(
                "!r:x",
                &[&photo, &msg("$l", "regarde https://example.org stp")],
            )
            .unwrap();
        let media = index.listing("!r:x", KindFilter::Media, None, 10).unwrap();
        assert_eq!(media.len(), 1);
        assert_eq!(media[0].event_id, "$p");
        let links = index.listing("!r:x", KindFilter::Links, None, 10).unwrap();
        assert_eq!(links[0].event_id, "$l");
        assert_eq!(found(&index, "vacances"), vec!["$p"]);
    }
}
