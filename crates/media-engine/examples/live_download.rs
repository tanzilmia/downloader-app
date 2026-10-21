//! Authorized live integration probe. No default download runs during tests.
use download_core::{DownloadManager, JobStatus, Repository};
use media_engine::ytdlp::YtdlpBackend;
use std::{path::PathBuf, sync::Arc, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(3..=4).contains(&args.len()) {
        return Err(
            "usage: live_download <engine.json> <output-directory> <authorized-public-url> [format-id]".into(),
        );
    }
    let output = PathBuf::from(&args[1]);
    let backend = Arc::new(YtdlpBackend::new(&PathBuf::from(&args[0]), &output).await?);
    let media = download_core::extractor::Extractor::inspect(backend.as_ref(), &args[2]).await?;
    println!(
        "Metadata verified: {} ({} formats)",
        media.title,
        media.formats.len()
    );
    let repository = Repository::open(&output.join("probe.db")).await?;
    let manager = DownloadManager::new(
        repository.clone(),
        backend.clone(),
        backend.clone(),
        Arc::new(|job| {
            println!(
                "{} {}% attempt {}",
                job.status.as_str(),
                job.progress.round(),
                job.attempt
            );
        }),
    )
    .await?;
    let format = args.get(3).unwrap_or(&media.formats[0].id);
    let job = manager.enqueue(&media.url, format).await?;
    let scheduler = manager.start();
    let result = tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let current = manager.job(&job.id).await?;
            match current.status {
                JobStatus::Completed => break Ok(current),
                JobStatus::Failed | JobStatus::Cancelled => {
                    break Err(current
                        .error
                        .unwrap_or_else(|| download_core::AppError::validation("Probe failed.")))
                }
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    })
    .await?;
    manager.stop().await;
    scheduler.await?;
    let completed = result?;
    let path = backend.files().validate_output(&completed, true).await?;
    println!("Verified saved output: {}", path.display());
    println!("{}", serde_json::to_string_pretty(&completed.output)?);
    repository.close().await;
    Ok(())
}
