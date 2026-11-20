//! Signs an update file exactly like `tauri build` does with TAURI_SIGNING_PRIVATE_KEY: a
//! minisign signature, base64-encoded, trusted comment "timestamp:<secs>\tfile:<name>".
//!
//!   tauri-update-sign sign KEY_FILE PASSWORD_FILE NAME < file > file.sig
//!   tauri-update-sign verify PUBKEY (tauri.conf.json's base64 value) SIG_FILE FILE
//!
//! `sign` runs as the buildsign account (see ../build-sign), which alone can read the key
//! file (Tauri's format: base64 of a minisign secret key) and its password. `verify` needs
//! only the public key: build scripts check every signature before publishing.

use base64::{Engine, engine::general_purpose::STANDARD};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() {
    if let Err(e) = run() {
        eprintln!("tauri-update-sign: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["sign", key, password, name] => sign(key, password, name),
        ["verify", pubkey, sig, file] => verify(pubkey, sig, file),
        _ => Err("usage: sign KEY_FILE PASSWORD_FILE NAME < file > file.sig | verify PUBKEY SIG_FILE FILE".into()),
    }
}

/// Base64 text (a file's content or tauri.conf.json's value) → the minisign text inside.
fn unbase64(text: &str) -> Result<String> {
    Ok(String::from_utf8(STANDARD.decode(text.trim())?)?)
}

fn sign(key: &str, password: &str, name: &str) -> Result<()> {
    let key = unbase64(&std::fs::read_to_string(key)?)?;
    let password = std::fs::read_to_string(password)?;
    let password = password.strip_suffix('\n').unwrap_or(&password).to_owned();
    let sk = minisign::SecretKeyBox::from_string(&key)?.into_secret_key(Some(password))?;
    let secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let trusted = format!("timestamp:{secs}\tfile:{name}");
    let signature = minisign::sign(
        None,
        &sk,
        std::io::stdin().lock(),
        Some(&trusted),
        Some("signature from tauri secret key"),
    )?;
    let mut out = std::io::stdout().lock();
    out.write_all(STANDARD.encode(signature.to_string()).as_bytes())?;
    out.flush()?;
    Ok(())
}

fn verify(pubkey: &str, sig: &str, file: &str) -> Result<()> {
    let pk = minisign::PublicKeyBox::from_string(&unbase64(pubkey)?)?.into_public_key()?;
    let signature = minisign::SignatureBox::from_string(&unbase64(&std::fs::read_to_string(sig)?)?)?;
    minisign::verify(&pk, &signature, std::fs::File::open(file)?, true, false, false)?;
    Ok(())
}
