//! Optional app lock: a PIN asked when the app opens or comes back after a while (biometrics
//! are offered by the shell on top of it). It only hides the UI: the local store isn't
//! encrypted with the PIN. `app_lock.json` holds a salted PBKDF2 hash, never the PIN; it's
//! removed on sign-out.

use std::time::Duration;

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{Error, Result};

const LOCK_FILE: &str = "app_lock.json";
const ROUNDS: u32 = 200_000;
/// Wrong PINs allowed before a pause, and how long it lasts (doubling each time).
const FREE_ATTEMPTS: u32 = 5;
const PAUSE: Duration = Duration::from_secs(30);

#[derive(Serialize, Deserialize)]
struct Stored {
    salt: Vec<u8>,
    hash: Vec<u8>,
    rounds: u32,
    /// Wrong PINs since the last success, and until when (ms since the epoch) no PIN is
    /// taken: kept here so that restarting the app doesn't hand out new tries.
    #[serde(default)]
    tries: u32,
    #[serde(default)]
    wait_until: u64,
}

fn now_ms() -> u64 {
    matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now()
        .get()
        .into()
}

/// One PIN check at a time, so parallel guesses can't share a try.
static CHECKING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Unlock {
    Ok,
    Wrong,
    /// Too many wrong PINs: try again after this many seconds.
    Wait {
        seconds: u64,
    },
}

fn hash(pin: &str, salt: &[u8], rounds: u32) -> Vec<u8> {
    pbkdf2::pbkdf2_hmac_array::<Sha256, 32>(pin.as_bytes(), salt, rounds).to_vec()
}

/// Constant-time comparison.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn valid_pin(pin: &str) -> bool {
    (4..=12).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}

impl crate::Core {
    pub fn app_lock_enabled(&self) -> bool {
        crate::files::read_now(&self.data_dir.join(LOCK_FILE)).is_ok()
    }

    /// Turn the lock on with `pin` (4–12 digits), change it, or turn it off with `None`.
    pub async fn set_app_lock(&self, pin: Option<&str>) -> Result<()> {
        let path = self.data_dir.join(LOCK_FILE);
        let Some(pin) = pin else {
            return match crate::files::remove_file(&path).await {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
                _ => Ok(()),
            };
        };
        if !valid_pin(pin) {
            return Err(Error::AppLock("the PIN must be 4 to 12 digits".into()));
        }
        let mut salt = vec![0u8; 16];
        rand::rng().fill_bytes(&mut salt);
        let pin = pin.to_owned();
        let stored = crate::rt::blocking(move || Stored {
            hash: hash(&pin, &salt, ROUNDS),
            salt,
            rounds: ROUNDS,
            tries: 0,
            wait_until: 0,
        })
        .await
        .map_err(|e| Error::AppLock(e.to_string()))?;
        crate::files::create_dir_all(&self.data_dir).await?;
        self.save_app_lock(&stored).await
    }

    async fn save_app_lock(&self, stored: &Stored) -> Result<()> {
        let path = self.data_dir.join(LOCK_FILE);
        let tmp = path.with_extension("tmp");
        crate::files::write(&tmp, serde_json::to_vec(stored)?).await?;
        crate::files::rename(&tmp, &path).await?;
        Ok(())
    }

    /// Check a PIN. Unlocked when there's no lock.
    pub async fn unlock_app(&self, pin: &str) -> Result<Unlock> {
        let _one = CHECKING.lock().await;
        let bytes = match crate::files::read(self.data_dir.join(LOCK_FILE)).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Unlock::Ok),
            Err(e) => return Err(e.into()),
        };
        let mut stored: Stored = serde_json::from_slice(&bytes)?;
        let left = stored.wait_until.saturating_sub(now_ms());
        if left > 0 {
            return Ok(Unlock::Wait {
                // A clock set back must not lock the app for longer than the longest pause.
                seconds: (left / 1000 + 1).min(pause_after(u32::MAX).as_secs()),
            });
        }
        let (pin, salt, expected, rounds) = (
            pin.to_owned(),
            stored.salt.clone(),
            stored.hash.clone(),
            stored.rounds,
        );
        let ok = crate::rt::blocking(move || same(&hash(&pin, &salt, rounds), &expected))
            .await
            .map_err(|e| Error::AppLock(e.to_string()))?;
        if ok {
            if stored.tries > 0 || stored.wait_until > 0 {
                (stored.tries, stored.wait_until) = (0, 0);
                self.save_app_lock(&stored).await?;
            }
            return Ok(Unlock::Ok);
        }
        stored.tries = stored.tries.saturating_add(1);
        let pause = (stored.tries >= FREE_ATTEMPTS).then(|| pause_after(stored.tries));
        stored.wait_until = pause.map_or(0, |p| now_ms() + p.as_millis() as u64);
        self.save_app_lock(&stored).await?;
        Ok(match pause {
            Some(pause) => Unlock::Wait {
                seconds: pause.as_secs(),
            },
            None => Unlock::Wrong,
        })
    }

    /// Biometrics (checked by the shell) unlock without the PIN.
    pub async fn biometric_unlocked(&self) {
        let _one = CHECKING.lock().await;
        let Ok(bytes) = crate::files::read(self.data_dir.join(LOCK_FILE)).await else {
            return;
        };
        if let Ok(mut stored) = serde_json::from_slice::<Stored>(&bytes)
            && (stored.tries > 0 || stored.wait_until > 0)
        {
            (stored.tries, stored.wait_until) = (0, 0);
            let _ = self.save_app_lock(&stored).await;
        }
    }
}

/// The pause after `tries` wrong PINs in a row: 30 s, doubling up to 32 min.
fn pause_after(tries: u32) -> Duration {
    PAUSE * 2u32.pow(tries.saturating_sub(FREE_ATTEMPTS).min(6))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_compares() {
        let h = hash("1234", b"salt", 1000);
        assert!(same(&h, &hash("1234", b"salt", 1000)));
        assert!(!same(&h, &hash("1235", b"salt", 1000)));
        assert!(!same(&h, &hash("1234", b"pepper", 1000)));
    }

    #[test]
    fn pauses_double_and_stop_growing() {
        assert_eq!(pause_after(5), Duration::from_secs(30));
        assert_eq!(pause_after(6), Duration::from_secs(60));
        assert_eq!(pause_after(u32::MAX), Duration::from_secs(30 * 64));
    }

    #[test]
    fn pin_rules() {
        assert!(valid_pin("1234"));
        assert!(!valid_pin("123"));
        assert!(!valid_pin("12a4"));
        assert!(!valid_pin("1234567890123"));
    }
}
