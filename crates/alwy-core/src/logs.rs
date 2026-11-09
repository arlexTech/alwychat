//! "Report a problem": the app's recent log, kept in memory (a tracing layer the shells add to
//! their subscriber), and the last crash, kept in `last_crash.txt` in the data folder until
//! it's sent or dismissed. A report is sent to a chat the user picks (usually a DM with whoever
//! helps them): nothing goes anywhere else.

use std::{
    collections::VecDeque,
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{Layer, layer::Context};

use crate::Result;

/// How much recent log is kept (oldest lines go first).
const MAX_BYTES: usize = 400_000;
/// Written by the panic hook (and by the Android side for Java crashes).
const CRASH_FILE: &str = "last_crash.txt";

static LINES: Mutex<(VecDeque<String>, usize)> = Mutex::new((VecDeque::new(), 0));
static CRASH_PATH: OnceLock<PathBuf> = OnceLock::new();

/// The tracing layer that keeps the recent log.
pub fn layer() -> Recorder {
    Recorder
}

pub struct Recorder;

impl<S: Subscriber> Layer<S> for Recorder {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let meta = event.metadata();
        let mut line = format!("{} {:5} {}: ", timestamp(), meta.level(), meta.target());
        event.record(&mut Fields(&mut line));
        push(line);
    }
}

struct Fields<'a>(&'a mut String);

impl Visit for Fields<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.0.push_str(value);
        } else {
            let _ = write!(self.0, " {}={value}", field.name());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.0, "{value:?}");
        } else {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }
}

fn push(line: String) {
    let mut kept = LINES.lock().unwrap_or_else(|e| e.into_inner());
    kept.1 += line.len() + 1;
    kept.0.push_back(line);
    while kept.1 > MAX_BYTES {
        match kept.0.pop_front() {
            Some(old) => kept.1 -= old.len() + 1,
            None => break,
        }
    }
}

/// The recent log, oldest line first.
pub fn recent() -> String {
    let kept = LINES.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = String::with_capacity(kept.1);
    for line in &kept.0 {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// "2026-11-06 10:22:01.123" (UTC).
fn timestamp() -> String {
    let ms: u64 = matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now()
        .get()
        .into();
    let (secs, ms) = (ms / 1000, ms % 1000);
    let (y, m, d) = civil_date(secs / 86_400);
    let s = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.{ms:03}",
        s / 3600,
        s / 60 % 60,
        s % 60
    )
}

/// Days since 1970-01-01 → (year, month, day) (Howard Hinnant's algorithm).
fn civil_date(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + u64::from(m <= 2), m, d)
}

/// Keep panics: in the log, and in `last_crash.txt` with the log before it (offered at the next
/// start). Once per process; the previous hook still runs.
pub fn watch_crashes(data_dir: &Path) {
    if CRASH_PATH.set(data_dir.join(CRASH_FILE)).is_err() {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let text = format!(
            "{} panic: {info}\n{}",
            timestamp(),
            std::backtrace::Backtrace::force_capture()
        );
        push(text.clone());
        #[cfg(not(target_family = "wasm"))]
        if let Some(path) = CRASH_PATH.get() {
            let _ = std::fs::write(path, format!("{text}\n--- log before it ---\n{}", recent()));
        }
        previous(info);
    }));
}

impl crate::Core {
    /// What the last crash left (since then not sent or dismissed), if any.
    pub async fn last_crash(&self) -> Option<String> {
        let bytes = crate::files::read(self.data_dir.join(CRASH_FILE))
            .await
            .ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned()).filter(|t| !t.trim().is_empty())
    }

    pub async fn dismiss_crash(&self) {
        let _ = crate::files::remove_file(self.data_dir.join(CRASH_FILE)).await;
    }

    /// Send a problem report to a chat: `message` (what happened, the app version…) as a
    /// message and, with `include_log`, the recent log (and the last crash) as a text file.
    pub async fn send_report(&self, room_id: &str, message: &str, include_log: bool) -> Result<()> {
        let room = self.room(room_id)?;
        room.send(
            matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain(message),
        )
        .await?;
        if include_log {
            let mut text = String::new();
            if let Some(crash) = self.last_crash().await {
                text.push_str("=== Last crash ===\n");
                text.push_str(&crash);
                text.push_str("\n\n");
            }
            text.push_str("=== Recent log ===\n");
            text.push_str(&recent());
            let name = format!("alwychat-log-{}.txt", timestamp().replace([' ', ':'], "-"));
            room.send_attachment(
                name,
                &mime::TEXT_PLAIN_UTF_8,
                text.into_bytes(),
                matrix_sdk::attachment::AttachmentConfig::new(),
            )
            .await?;
            self.dismiss_crash().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(19_723), (2024, 1, 1));
        assert_eq!(civil_date(20_398), (2025, 11, 6));
        assert_eq!(civil_date(11_016), (2000, 2, 29));
    }

    #[test]
    fn keeps_the_latest() {
        for i in 0..20_000 {
            push(format!("line {i:05} {}", "x".repeat(40)));
        }
        let log = recent();
        assert!(log.len() <= MAX_BYTES);
        assert!(log.ends_with("line 19999 xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n"));
        assert!(!log.contains("line 00000"));
    }
}
