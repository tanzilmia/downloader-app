use async_trait::async_trait;
use download_core::{
    engine::{TransferEngine, TransferProgress},
    extractor::MockExtractor,
    *,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Models a child process that completes during cancellation acknowledgement.
struct LateSuccess;
#[async_trait]
impl TransferEngine for LateSuccess {
    async fn run(
        &self,
        _: Job,
        _: mpsc::Sender<TransferProgress>,
        _: CancellationToken,
    ) -> Result<(), AppError> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }
}

#[tokio::test]
async fn immediate_retry_never_accepts_the_cancelled_attempts_late_success() {
    let directory = tempfile::tempdir().unwrap();
    let repository = Repository::open(&directory.path().join("race.db"))
        .await
        .unwrap();
    let (sender, mut events) = mpsc::unbounded_channel();
    let manager = DownloadManager::new(
        repository.clone(),
        Arc::new(MockExtractor),
        Arc::new(LateSuccess),
        Arc::new(move |job| {
            sender.send(job).unwrap();
        }),
    )
    .await
    .unwrap();
    let job = manager
        .enqueue("https://youtu.be/sample", "mock-1080p")
        .await
        .unwrap();
    let scheduler = manager.start();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if repository.get(&job.id).await.unwrap().status == JobStatus::Downloading {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    manager.cancel(&job.id).await.unwrap();
    manager.retry(&job.id).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let event = events.recv().await.unwrap();
            if event.status == JobStatus::Completed {
                assert_eq!(
                    event.attempt, 2,
                    "old worker completed a newer queued attempt"
                );
                break;
            }
        }
    })
    .await
    .unwrap();
    manager.stop().await;
    scheduler.await.unwrap();
    repository.close().await;
}
