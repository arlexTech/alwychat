//! Grab the first camera through the native capture for 3 s and save the last preview frame:
//! `cargo run -p alwy-call --example camera -- /tmp/frame.jpg`.
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "frame.jpg".into());
    let last: Arc<Mutex<(usize, Vec<u8>)>> = Arc::default();
    let sink = {
        let last = last.clone();
        Arc::new(move |_key: &str, jpeg: Vec<u8>| {
            let mut l = last.lock().unwrap();
            l.0 += 1;
            l.1 = jpeg;
        })
    };
    let preview = alwy_call::CameraPreview::start(0, sink)
        .await
        .expect("camera");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    drop(preview);
    let (frames, jpeg) = last.lock().unwrap().clone();
    std::fs::write(&out, &jpeg).unwrap();
    println!("{frames} preview frames, last {} bytes → {out}", jpeg.len());
}
