use async_trait::async_trait;
use download_core::{
    engine::{MockEngine, TransferEngine, TransferProgress},
    extractor::{Extractor, MockExtractor},
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

const URL: &str = "https://www.youtube.com/watch?v=BaW_jenozKc";
fn fixture() -> Job {
    serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/job.json"
    ))
    .unwrap()
}
async fn manager(repository: Repository, engine: Arc<dyn TransferEngine>) -> Arc<DownloadManager> {
    DownloadManager::new(
        repository,
        Arc::new(MockExtractor),
        engine,
        Arc::new(|_| {}),
    )
    .await
    .unwrap()
}
async fn wait_for(manager: &DownloadManager, id: &str, status: JobStatus) -> Job {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let job = manager
                .snapshot()
                .await
                .unwrap()
                .jobs
                .into_iter()
                .find(|job| job.id == id)
                .unwrap();
            if job.status == status {
                return job;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("job did not reach expected state")
}

#[tokio::test]
async fn shared_contract_fixture_roundtrips_and_matches_mock() {
    let job = fixture();
    let mut expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/job.json"
    ))
    .unwrap();
    // JavaScript has a single JSON number type; Rust distinguishes integer/float.
    let serialized = serde_json::to_value(&job).unwrap();
    assert_eq!(
        serialized["progress"].as_f64(),
        expected["progress"].as_f64()
    );
    expected["progress"] = serialized["progress"].clone();
    assert_eq!(serialized, expected);
    assert_eq!(MockExtractor.inspect(URL).await.unwrap(), job.media);
    let urls: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/urls.json"
    ))
    .unwrap();
    for input in urls["valid"].as_array().unwrap() {
        assert!(validate_url(input.as_str().unwrap()).is_ok(), "{input}");
    }
    for input in urls["invalid"].as_array().unwrap() {
        assert!(validate_url(input.as_str().unwrap()).is_err(), "{input}");
    }
}

#[tokio::test]
async fn fresh_database_migrates_reopens_and_recovers_interrupted_jobs() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("test.db");
    let repository = Repository::open(&path).await.unwrap();
    let first = manager(repository.clone(), Arc::new(MockEngine::default())).await;
    first
        .update_settings(Settings {
            concurrency: 1,
            theme: Theme::Light,
        })
        .await
        .unwrap();
    let mut job = fixture();
    job.status = JobStatus::Downloading;
    job.progress = 45.0;
    repository.insert(&job).await.unwrap();
    repository.close().await;
    drop(first);
    let reopened = Repository::open(&path).await.unwrap();
    let recovered = manager(reopened.clone(), Arc::new(MockEngine::default()))
        .await
        .snapshot()
        .await
        .unwrap();
    assert_eq!(
        recovered.settings,
        Settings {
            concurrency: 1,
            theme: Theme::Light
        }
    );
    assert_eq!(recovered.jobs[0].status, JobStatus::Queued);
    assert_eq!(recovered.jobs[0].progress, 0.0);
    assert_eq!(recovered.jobs[0].attempt, 1);
    assert_eq!(recovered.jobs[0].revision, job.revision + 1);
    assert_eq!(
        recovered.jobs[0].error.as_ref().unwrap().code,
        ErrorCode::Interrupted
    );
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!(sqlx::query("UPDATE jobs SET status = 'bogus'")
        .execute(&pool)
        .await
        .is_err());
    pool.close().await;
    reopened.close().await;
}

#[tokio::test]
async fn success_events_are_durable_and_monotonic() {
    let directory = tempfile::tempdir().unwrap();
    let repository = Repository::open(&directory.path().join("test.db"))
        .await
        .unwrap();
    let (sender, mut events) = mpsc::unbounded_channel();
    let manager = DownloadManager::new(
        repository.clone(),
        Arc::new(MockExtractor),
        Arc::new(MockEngine {
            step_duration: Duration::from_millis(4),
        }),
        Arc::new(move |job| {
            sender.send(job).unwrap();
        }),
    )
    .await
    .unwrap();
    let job = manager.enqueue(URL, "mock-720p").await.unwrap();
    let scheduler = manager.start();
    let complete = wait_for(&manager, &job.id, JobStatus::Completed).await;
    assert_eq!(complete.progress, 100.0);
    assert_eq!(complete.attempt, 1);
    assert!(complete.is_mock);
    let mut previous: Option<Job> = None;
    while let Ok(event) = events.try_recv() {
        if let Some(previous) = previous {
            assert!(event.revision > previous.revision);
            assert!(event.progress >= previous.progress);
        }
        assert!(repository.get(&event.id).await.unwrap().revision >= event.revision);
        previous = Some(event);
    }
    assert_eq!(previous.unwrap().status, JobStatus::Completed);
    assert!(manager.retry(&job.id).await.is_err());
    manager.stop().await;
    scheduler.await.unwrap();
    repository.close().await;
}

#[tokio::test]
async fn queue_bounds_concurrency_and_cancel_does_not_complete() {
    let directory = tempfile::tempdir().unwrap();
    let repository = Repository::open(&directory.path().join("test.db"))
        .await
        .unwrap();
    let manager = manager(
        repository.clone(),
        Arc::new(MockEngine {
            step_duration: Duration::from_millis(10),
        }),
    )
    .await;
    manager
        .update_settings(Settings {
            concurrency: 1,
            theme: Theme::Dark,
        })
        .await
        .unwrap();
    let first = manager.enqueue(URL, "mock-1080p").await.unwrap();
    let second = manager.enqueue(URL, "mock-1080p").await.unwrap();
    let scheduler = manager.start();
    wait_for(&manager, &first.id, JobStatus::Downloading).await;
    assert_eq!(
        repository.get(&second.id).await.unwrap().status,
        JobStatus::Queued
    );
    let cancelled = manager.cancel(&first.id).await.unwrap();
    assert_eq!(cancelled.status, JobStatus::Cancelled);
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(
        repository.get(&first.id).await.unwrap().status,
        JobStatus::Cancelled
    );
    manager.retry(&first.id).await.unwrap();
    for _ in 0..100 {
        let jobs = manager.snapshot().await.unwrap().jobs;
        assert!(
            jobs.iter()
                .filter(|j| j.status == JobStatus::Downloading)
                .count()
                <= 1
        );
        if jobs.iter().all(|job| job.status == JobStatus::Completed) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        wait_for(&manager, &first.id, JobStatus::Completed)
            .await
            .attempt,
        2
    );
    wait_for(&manager, &second.id, JobStatus::Completed).await;
    assert!(manager.cancel(&first.id).await.is_err());
    manager.stop().await;
    scheduler.await.unwrap();
    repository.close().await;
}

struct FailOnceEngine(AtomicUsize);
#[async_trait]
impl TransferEngine for FailOnceEngine {
    async fn run(
        &self,
        _: Job,
        _: mpsc::Sender<TransferProgress>,
        _: CancellationToken,
    ) -> Result<(), AppError> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(AppError {
                code: ErrorCode::Network,
                message: "A simulated network interruption.".into(),
                retryable: true,
            })
        } else {
            Ok(())
        }
    }
}
#[tokio::test]
async fn classified_failure_can_be_retried_and_inputs_are_revalidated() {
    let directory = tempfile::tempdir().unwrap();
    let repository = Repository::open(&directory.path().join("test.db"))
        .await
        .unwrap();
    let manager = manager(
        repository.clone(),
        Arc::new(FailOnceEngine(AtomicUsize::new(0))),
    )
    .await;
    assert!(manager
        .enqueue("http://127.0.0.1/a", "mock-1080p")
        .await
        .is_err());
    assert!(manager.enqueue(URL, "--exec").await.is_err());
    assert!(manager
        .update_settings(Settings {
            concurrency: 5,
            theme: Theme::Dark
        })
        .await
        .is_err());
    let job = manager.enqueue(URL, "mock-audio").await.unwrap();
    let scheduler = manager.start();
    let failed = wait_for(&manager, &job.id, JobStatus::Failed).await;
    assert_eq!(failed.error.unwrap().code, ErrorCode::Network);
    manager.retry(&job.id).await.unwrap();
    let complete = wait_for(&manager, &job.id, JobStatus::Completed).await;
    assert_eq!(complete.attempt, 2);
    assert!(complete.error.is_none());
    manager.stop().await;
    scheduler.await.unwrap();
    repository.close().await;
}
