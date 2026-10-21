use async_trait::async_trait;
use download_core::{
    engine::{TransferEngine, TransferProgress},
    extractor::MockExtractor,
    *,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

struct ReportingEngine(Mutex<mpsc::Receiver<TransferProgress>>);
#[async_trait]
impl TransferEngine for ReportingEngine {
    async fn run(
        &self,
        _: Job,
        progress: mpsc::Sender<TransferProgress>,
        cancel: CancellationToken,
    ) -> Result<(), AppError> {
        let mut updates = self.0.lock().await;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return Err(AppError { code: ErrorCode::Interrupted, message: "Cancelled".into(), retryable: true, retry_after_seconds: None, }),
                update = updates.recv() => match update {
                    Some(update) => { let _ = progress.send(update).await; },
                    None => return Ok(()),
                }
            }
        }
    }
}
async fn wait_for(manager: &DownloadManager, id: &str, predicate: impl Fn(&Job) -> bool) -> Job {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let job = manager.job(id).await.unwrap();
            if predicate(&job) {
                return job;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("expected transfer update")
}
#[tokio::test]
async fn speed_is_durable_expires_and_clears_on_cancel_retry_and_completion() {
    let directory = tempfile::tempdir().unwrap();
    let repository = Repository::open(&directory.path().join("speed.db"))
        .await
        .unwrap();
    let (updates, receiver) = mpsc::channel(8);
    let (events, mut received) = mpsc::unbounded_channel();
    let manager = DownloadManager::new(
        repository.clone(),
        Arc::new(MockExtractor),
        Arc::new(ReportingEngine(Mutex::new(receiver))),
        Arc::new(move |job| {
            let _ = events.send(job);
        }),
    )
    .await
    .unwrap();
    let job = manager
        .enqueue("https://youtu.be/sample", "mock-1080p")
        .await
        .unwrap();
    let scheduler = manager.start();
    updates
        .send(TransferProgress {
            percent: 25.0,
            speed_bytes_per_second: Some(1_250_000.0),
        })
        .await
        .unwrap();
    wait_for(&manager, &job.id, |job| {
        job.speed_bytes_per_second == Some(1_250_000.0)
    })
    .await;
    assert!(repository
        .get(&job.id)
        .await
        .unwrap()
        .speed_bytes_per_second
        .is_some());
    assert!(std::iter::from_fn(|| received.try_recv().ok())
        .any(|job| job.speed_bytes_per_second == Some(1_250_000.0)));
    let stalled = wait_for(&manager, &job.id, |job| {
        job.progress == 25.0 && job.speed_bytes_per_second.is_none()
    })
    .await;
    assert_eq!(stalled.status, JobStatus::Downloading);
    updates
        .send(TransferProgress {
            percent: 30.0,
            speed_bytes_per_second: Some(512_000.0),
        })
        .await
        .unwrap();
    wait_for(&manager, &job.id, |job| {
        job.speed_bytes_per_second.is_some()
    })
    .await;
    assert!(manager
        .cancel(&job.id)
        .await
        .unwrap()
        .speed_bytes_per_second
        .is_none());
    assert!(manager
        .retry(&job.id)
        .await
        .unwrap()
        .speed_bytes_per_second
        .is_none());
    wait_for(&manager, &job.id, |job| {
        job.status == JobStatus::Downloading
    })
    .await;
    updates
        .send(TransferProgress {
            percent: 40.0,
            speed_bytes_per_second: Some(256_000.0),
        })
        .await
        .unwrap();
    wait_for(&manager, &job.id, |job| {
        job.speed_bytes_per_second == Some(256_000.0)
    })
    .await;
    drop(updates);
    let complete = wait_for(&manager, &job.id, |job| job.status == JobStatus::Completed).await;
    assert!(complete.speed_bytes_per_second.is_none());
    manager.stop().await;
    manager.wait_stopped().await;
    scheduler.await.unwrap();
    repository.close().await;
}

#[tokio::test]
async fn nonfinite_engine_speed_fails_without_persisting_an_invalid_sample() {
    let directory = tempfile::tempdir().unwrap();
    let repository = Repository::open(&directory.path().join("invalid-speed.db"))
        .await
        .unwrap();
    let (updates, receiver) = mpsc::channel(1);
    let manager = DownloadManager::new(
        repository.clone(),
        Arc::new(MockExtractor),
        Arc::new(ReportingEngine(Mutex::new(receiver))),
        Arc::new(|_| {}),
    )
    .await
    .unwrap();
    let job = manager
        .enqueue("https://youtu.be/sample", "mock-1080p")
        .await
        .unwrap();
    let scheduler = manager.start();
    updates
        .send(TransferProgress {
            percent: 25.0,
            speed_bytes_per_second: Some(f64::INFINITY),
        })
        .await
        .unwrap();
    let failed = wait_for(&manager, &job.id, |job| job.status == JobStatus::Failed).await;
    assert_eq!(failed.error.unwrap().code, ErrorCode::Validation);
    assert_eq!(failed.progress, 0.0);
    assert!(failed.speed_bytes_per_second.is_none());
    manager.stop().await;
    manager.wait_stopped().await;
    scheduler.await.unwrap();
    repository.close().await;
}
