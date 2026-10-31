//! Optional app lock: a PIN asked when the app opens or comes back after a while (biometrics
//! are offered by the shell on top of it). It only hides the UI: the local store isn't
//! encrypted with the PIN. `app_lock.json` holds a salted PBKDF2 hash, never the PIN; it's
//! removed on sign-out.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

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
}

/// Wrong attempts since the last success (process-wide; the core is a singleton).
static ATTEMPTS: Mutex<(u32, Option<Instant>)> = Mutex::new((0, None));

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
        self.data_dir.join(LOCK_FILE).exists()
    }

    /// Turn the lock on with `pin` (4–12 digits), change it, or turn it off with `None`.
    pub async fn set_app_lock(&self, pin: Option<&str>) -> Result<()> {
        let path = self.data_dir.join(LOCK_FILE);
        let Some(pin) = pin else {
            return match tokio::fs::remove_file(&path).await {
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
        let stored = tokio::task::spawn_blocking(move || Stored {
            hash: hash(&pin, &salt, ROUNDS),
            salt,
            rounds: ROUNDS,
        })
        .await
        .map_err(|e| Error::AppLock(e.to_string()))?;
        tokio::fs::create_dir_all(&self.data_dir).await?;
        let tmp = path.with_extension("tmp");
        tokio::fs::write(&tmp, serde_json::to_vec(&stored)?).await?;
        tokio::fs::rename(&tmp, &path).await?;
        Ok(())
    }

    /// Check a PIN. Unlocked when there's no lock.
    pub async fn unlock_app(&self, pin: &str) -> Result<Unlock> {
        {
            let attempts = ATTEMPTS.lock().unwrap();
            if let (_, Some(until)) = *attempts
                && let Some(left) = until.checked_duration_since(Instant::now())
            {
                return Ok(Unlock::Wait {
                    seconds: left.as_secs() + 1,
                });
            }
        }
        let bytes = match tokio::fs::read(self.data_dir.join(LOCK_FILE)).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Unlock::Ok),
            Err(e) => return Err(e.into()),
        };
        let stored: Stored = serde_json::from_slice(&bytes)?;
        let pin = pin.to_owned();
        let ok = tokio::task::spawn_blocking(move || {
            same(&hash(&pin, &stored.salt, stored.rounds), &stored.hash)
        })
        .await
        .map_err(|e| Error::AppLock(e.to_string()))?;
        let mut attempts = ATTEMPTS.lock().unwrap();
        if ok {
            *attempts = (0, None);
            return Ok(Unlock::Ok);
        }
        attempts.0 += 1;
        if attempts.0 >= FREE_ATTEMPTS {
            let pause = PAUSE * 2u32.pow((attempts.0 - FREE_ATTEMPTS).min(6));
            attempts.1 = Some(Instant::now() + pause);
            return Ok(Unlock::Wait {
                seconds: pause.as_secs(),
            });
        }
        Ok(Unlock::Wrong)
    }

    /// Biometrics (checked by the shell) unlock without the PIN.
    pub fn biometric_unlocked(&self) {
        *ATTEMPTS.lock().unwrap() = (0, None);
    }
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
    fn pin_rules() {
        assert!(valid_pin("1234"));
        assert!(!valid_pin("123"));
        assert!(!valid_pin("12a4"));
        assert!(!valid_pin("1234567890123"));
    }
}
