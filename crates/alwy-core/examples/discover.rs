//! `cargo run -p alwy-core --example discover -- example.org`

#[tokio::main]
async fn main() {
    let server = std::env::args().nth(1).unwrap_or_else(|| "example.org".into());
    match alwy_core::discovery::discover(&server).await {
        Ok(info) => println!("{info:#?}"),
        Err(e) => eprintln!("error: {e}"),
    }
}
