//! The web server inside AlwyChat's web Docker image (`docker/web/Dockerfile.release`): the
//! image has nothing else, no system, no shell. It serves the files `pnpm build:web` made, all
//! kept in memory, the way the Caddyfile next to it does for people who use Caddy:
//!
//! - Brotli copies (`file.br`) are sent as they are to browsers that take Brotli (all current
//!   ones over https); the rare one that doesn't gets them decompressed once and kept. The image
//!   ships only the `.br` copy of each such file.
//! - Any address without a file extension is the app (`index.html`): the app does its own pages.
//! - `/config.json` = this site's settings from the environment (`DEFAULT_MATRIX_SERVER`).
//! - ETags (304 when the browser already has it), the same security and cache headers.
//!
//! Settings: `DEFAULT_MATRIX_SERVER` (the server the sign-in screen suggests), `PORT` (default 80),
//! `ALWY_ROOT` (default /site). `alwy-serve check` asks the running server for its config: the
//! image's health check (there is no shell or curl in it).
use std::{
    collections::HashMap,
    convert::Infallible,
    hash::{DefaultHasher, Hash, Hasher},
    io::Read,
    net::SocketAddr,
    path::Path,
    sync::{Arc, OnceLock},
    time::Duration,
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode,
    header::{self, HeaderValue},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

/// One file of the site.
struct File {
    body: Bytes,
    /// `body` is Brotli-compressed.
    brotli: bool,
    /// Decompressed on first need (a browser without Brotli).
    plain: OnceLock<Bytes>,
    mime: &'static str,
    etag: HeaderValue,
}

struct Site {
    files: HashMap<String, File>,
    config: File,
}

fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(80);
    if std::env::args().nth(1).as_deref() == Some("check") {
        std::process::exit(if check(port) { 0 } else { 1 });
    }
    let root = std::env::var("ALWY_ROOT").unwrap_or_else(|_| "/site".into());
    let site = match load(Path::new(&root)) {
        Ok(site) => Arc::new(site),
        Err(e) => {
            eprintln!("alwy-serve: can't read {root}: {e}");
            std::process::exit(1);
        }
    };
    let bytes: usize = site.files.values().map(|f| f.body.len()).sum();
    eprintln!(
        "alwy-serve: {} files ({:.1} MB) on port {port}",
        site.files.len(),
        bytes as f64 / 1e6
    );
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(serve(site, port));
}

async fn serve(site: Arc<Site>, port: u16) {
    let listener = match TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("alwy-serve: port {port}: {e}");
            std::process::exit(1);
        }
    };
    let accept = async {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let site = site.clone();
            tokio::spawn(async move {
                let service = service_fn(move |req| {
                    let site = site.clone();
                    async move { Ok::<_, Infallible>(answer(&site, &req)) }
                });
                let _ = http1::Builder::new()
                    .timer(hyper_util::rt::TokioTimer::new())
                    .header_read_timeout(Duration::from_secs(20))
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    };
    // `docker stop` sends SIGTERM to process 1, which has no default handler: stop on it.
    let mut term =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("signal");
    tokio::select! {
        () = accept => {}
        _ = term.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
}

fn answer<B>(site: &Site, req: &Request<B>) -> Response<Full<Bytes>> {
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return plain(StatusCode::METHOD_NOT_ALLOWED, "method not allowed");
    }
    let path = req.uri().path();
    if path.split('/').any(|part| part == "..") {
        return plain(StatusCode::BAD_REQUEST, "bad path");
    }
    let key = if path.ends_with('/') {
        format!("{path}index.html")
    } else {
        path.to_owned()
    };
    let (file, key) = if key == "/config.json" {
        (&site.config, key.as_str())
    } else if let Some(file) = site.files.get(&key) {
        (file, key.as_str())
    } else if !key.rsplit('/').next().unwrap_or("").contains('.') {
        // A page of the app (/register, /room/…): the app itself.
        match site.files.get("/index.html") {
            Some(file) => (file, "/index.html"),
            None => return plain(StatusCode::NOT_FOUND, "not found"),
        }
    } else {
        return plain(StatusCode::NOT_FOUND, "not found");
    };

    let mut response = Response::builder();
    let headers = response.headers_mut().expect("builder");
    security_headers(headers);
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(file.mime));
    headers.insert(header::ETAG, file.etag.clone());
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if key.starts_with("/_app/immutable/") {
            // File names with a content hash never change.
            "public, max-age=31536000, immutable"
        } else {
            // Checked each time (an ETag makes that a tiny 304), so updates reach everyone.
            "no-cache"
        }),
    );
    if file.brotli {
        headers.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    }
    let fresh = req
        .headers()
        .get(header::IF_NONE_MATCH)
        .is_some_and(|tags| {
            tags.as_bytes()
                .split(|&b| b == b',')
                .any(|t| t.trim_ascii() == file.etag.as_bytes())
        });
    if fresh {
        return response
            .status(StatusCode::NOT_MODIFIED)
            .body(Full::default())
            .expect("response");
    }
    let body = if file.brotli && takes_brotli(req) {
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("br"));
        file.body.clone()
    } else if file.brotli {
        file.plain.get_or_init(|| decompress(&file.body)).clone()
    } else {
        file.body.clone()
    };
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(body.len()));
    let body = if req.method() == Method::HEAD {
        Bytes::new()
    } else {
        body
    };
    response
        .status(StatusCode::OK)
        .body(Full::new(body))
        .expect("response")
}

fn takes_brotli<B>(req: &Request<B>) -> bool {
    req.headers()
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .any(|v| {
            v.to_str().unwrap_or("").split(',').any(|e| {
                let mut parts = e.split(';');
                parts.next().is_some_and(|name| name.trim() == "br")
                    && !parts.any(|p| p.trim().replace(' ', "") == "q=0")
            })
        })
}

/// The Caddyfile's headers: the page's own Content-Security-Policy is in index.html (it needs a
/// hash of SvelteKit's start script); these add what a <meta> tag can't do.
fn security_headers(headers: &mut hyper::HeaderMap) {
    for (name, value) in [
        ("content-security-policy", "frame-ancestors 'none'"),
        ("x-frame-options", "DENY"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        (
            "permissions-policy",
            "camera=(self), microphone=(self), geolocation=(self), display-capture=(self)",
        ),
        ("cross-origin-opener-policy", "same-origin-allow-popups"),
    ] {
        headers.insert(name, HeaderValue::from_static(value));
    }
}

fn plain(status: StatusCode, text: &'static str) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from_static(text.as_bytes())));
    *response.status_mut() = status;
    security_headers(response.headers_mut());
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn decompress(data: &[u8]) -> Bytes {
    let mut out = Vec::with_capacity(data.len() * 4);
    let mut reader = brotli_decompressor::Decompressor::new(data, 64 * 1024);
    match reader.read_to_end(&mut out) {
        Ok(_) => out.into(),
        Err(_) => Bytes::new(),
    }
}

/// Every file under `root`, keyed by its address ("/index.html"). `x.br` stands for `x`, and is
/// preferred to `x` itself when both are there.
fn load(root: &Path) -> std::io::Result<Site> {
    let mut found = Vec::new();
    walk(root, root, &mut found)?;
    let compressed: std::collections::HashSet<&str> = found
        .iter()
        .filter_map(|(a, _)| a.strip_suffix(".br"))
        .collect();
    let mut files = HashMap::new();
    for (address, path) in &found {
        let (address, brotli) = match address.strip_suffix(".br") {
            Some(original) => (original.to_owned(), true),
            None => (address.clone(), false),
        };
        if !brotli && compressed.contains(address.as_str()) {
            continue;
        }
        let body = Bytes::from(std::fs::read(path)?);
        files.insert(address.clone(), file(body, brotli, mime(&address)));
    }
    let server = std::env::var("DEFAULT_MATRIX_SERVER").unwrap_or_default();
    let config = format!("{{\"default_server\":{}}}", json_string(server.trim()));
    Ok(Site {
        files,
        config: file(config.into(), false, "application/json"),
    })
}

fn walk(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, std::path::PathBuf)>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else if let Ok(relative) = path.strip_prefix(root) {
            let address = format!("/{}", relative.to_string_lossy().replace('\\', "/"));
            out.push((address, path));
        }
    }
    Ok(())
}

fn file(body: Bytes, brotli: bool, mime: &'static str) -> File {
    let mut hasher = DefaultHasher::new();
    body.hash(&mut hasher);
    let etag = HeaderValue::from_str(&format!("\"{:016x}\"", hasher.finish())).expect("etag");
    File {
        body,
        brotli,
        plain: OnceLock::new(),
        mime,
        etag,
    }
}

fn mime(address: &str) -> &'static str {
    match address.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `alwy-serve check`: is the server answering? (Docker's HEALTHCHECK, no curl in the image.)
fn check(port: u16) -> bool {
    use std::io::Write;
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(3),
    ) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    if stream
        .write_all(b"GET /config.json HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut answer = [0u8; 12];
    stream.read_exact(&mut answer).is_ok()
        && (answer.starts_with(b"HTTP/1.1 200") || answer.starts_with(b"HTTP/1.0 200"))
}
