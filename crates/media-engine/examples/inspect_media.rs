//! Opt-in metadata-only diagnosis through the actual public-destination proxy.
//! Never downloads media or collects cookies. Output excludes raw extractor logs.
use download_core::extractor::Extractor;
use media_engine::ytdlp::YtdlpBackend;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 2 {
        return Err("usage: inspect_media <engine.json> <authorized-public-url>".into());
    }
    let directory = tempfile::tempdir()?;
    let backend = YtdlpBackend::new(Path::new(&arguments[0]), directory.path()).await?;
    let report = match backend.inspect(&arguments[1]).await {
        Ok(media) => {
            serde_json::json!({"success": true, "title": media.title, "formatCount": media.formats.len()})
        }
        Err(error) => serde_json::json!({"success": false, "error": error}),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
