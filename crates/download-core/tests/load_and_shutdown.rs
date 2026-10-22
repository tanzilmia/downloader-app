use async_trait::async_trait;
use download_core::{
    engine::{TransferEngine, TransferProgress},
    extractor::MockExtractor,
    *,
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct WaitingEngine {
    active: AtomicUsize,
    maximum: AtomicUsize,
}

struct RecoveringEngine(AtomicUsize);
#[async_trait]
impl TransferEngine for RecoveringEngine {
    async fn run(
        &self,
        _: Job,
        progress: mpsc::Sender<TransferProgress>,
        cancel: CancellationToken,
    ) -> Result<(), AppError> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            progress
                .send(TransferProgress {
                    percent: 42.0,
                    speed_bytes_per_second: None,
                })
                .await
                .unwrap();
            cancel.cancelled().await;
            return Err(AppError {
                code: ErrorCode::Interrupted,
                message: "Write failed".into(),
                retryable: true,
                retry_after_seconds: None,
            });
        }
        Ok(())
    }
}

#[tokio::test]
async fn worker_write_failure_reports_health_and_recovers_after_storage_is_repaired() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fault.db");
    let repository = Repository::open(&path).await.unwrap();
    let manager = DownloadManager::new(
        repository.clone(),
        Arc::new(MockExtractor),
        Arc::new(RecoveringEngine(AtomicUsize::new(0))),
        Arc::new(|_| {}),
    )
    .await
    .unwrap();
    let job = manager
        .enqueue("https://youtu.be/sample", "mock-1080p")
        .await
        .unwrap();
    let fault = sqlx::SqlitePool::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    // Claim succeeds; progress and terminal/recovery writes fail until repaired.
    sqlx::query("CREATE TRIGGER simulated_storage_failure BEFORE UPDATE ON jobs WHEN NEW.status != 'downloading' OR json_extract(NEW.payload,'$.progress') > 0 BEGIN SELECT RAISE(FAIL,'simulated storage failure'); END").execute(&fault).await.unwrap();
    let scheduler = manager.start();
    tokio::time::timeout(Duration::from_secs(5), async {
        while manager.snapshot().await.unwrap().health.is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        repository.get(&job.id).await.unwrap().status,
        JobStatus::Downloading
    );
    sqlx::query("DROP TRIGGER simulated_storage_failure")
        .execute(&fault)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = manager.snapshot().await.unwrap();
            if snapshot.jobs[0].status == JobStatus::Completed && snapshot.health.is_none() {
                assert_eq!(snapshot.jobs[0].attempt, 2);
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    manager.stop().await;
    manager.wait_stopped().await;
    scheduler.await.unwrap();
    fault.close().await;
    repository.close().await;
}
#[async_trait]
impl TransferEngine for WaitingEngine {
    async fn run(
        &self,
        _: Job,
        _: mpsc::Sender<TransferProgress>,
        cancel: CancellationToken,
    ) -> Result<(), AppError> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        cancel.cancelled().await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        Err(AppError {
            code: ErrorCode::Interrupted,
            message: "Stopped".into(),
            retryable: true,
            retry_after_seconds: None,
        })
    }
}

#[tokio::test]
async fn two_hundred_queued_jobs_stay_bounded_and_shutdown_remains_resumable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("load.db");
    let repository = Repository::open(&path).await.unwrap();
    let engine = Arc::new(WaitingEngine {
        active: AtomicUsize::new(0),
        maximum: AtomicUsize::new(0),
    });
    let manager = DownloadManager::new(
        repository.clone(),
        Arc::new(MockExtractor),
        engine.clone(),
        Arc::new(|_| {}),
    )
    .await
    .unwrap();
    manager
        .update_settings(Settings {
            concurrency: 4,
            theme: Theme::Dark,
            ..Settings::default()
        })
        .await
        .unwrap();
    for _ in 0..200 {
        manager
            .enqueue("https://youtu.be/sample", "mock-1080p")
            .await
            .unwrap();
    }
    let scheduler = manager.start();
    tokio::time::timeout(Duration::from_secs(5), async {
        while engine.active.load(Ordering::SeqCst) != 4 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let snapshot = manager.snapshot().await.unwrap();
    assert_eq!(snapshot.jobs.len(), 200);
    assert_eq!(
        snapshot
            .jobs
            .iter()
            .filter(|j| j.status == JobStatus::Downloading)
            .count(),
        4
    );
    let queued = snapshot
        .jobs
        .iter()
        .find(|j| j.status == JobStatus::Queued)
        .unwrap();
    manager.cancel(&queued.id).await.unwrap();
    manager.stop().await;
    manager.wait_stopped().await;
    scheduler.await.unwrap();
    assert_eq!(engine.active.load(Ordering::SeqCst), 0);
    assert_eq!(engine.maximum.load(Ordering::SeqCst), 4);
    let final_state = manager.snapshot().await.unwrap();
    assert_eq!(
        final_state
            .jobs
            .iter()
            .filter(|j| j.status == JobStatus::Queued)
            .count(),
        199
    );
    assert!(!final_state
        .jobs
        .iter()
        .any(|j| j.status == JobStatus::Failed || j.status == JobStatus::Downloading));
    repository.close().await;
    let reopened = Repository::open(&path).await.unwrap();
    assert_eq!(reopened.list().await.unwrap(), final_state.jobs);
    reopened.close().await;
}
