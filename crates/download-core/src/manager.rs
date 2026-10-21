use crate::{
    domain::*,
    engine::{TransferEngine, TransferProgress},
    extractor::Extractor,
    Repository,
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

pub type EventSink = Arc<dyn Fn(Job) + Send + Sync>;
struct Runtime {
    active: HashMap<String, CancellationToken>,
}

pub struct DownloadManager {
    repository: Repository,
    extractor: Arc<dyn Extractor>,
    engine: Arc<dyn TransferEngine>,
    runtime: Mutex<Runtime>,
    emit: EventSink,
    shutdown: CancellationToken,
}

impl DownloadManager {
    pub async fn new(
        repository: Repository,
        extractor: Arc<dyn Extractor>,
        engine: Arc<dyn TransferEngine>,
        emit: EventSink,
    ) -> Result<Arc<Self>, AppError> {
        let manager = Arc::new(Self {
            repository,
            extractor,
            engine,
            runtime: Mutex::new(Runtime {
                active: HashMap::new(),
            }),
            emit,
            shutdown: CancellationToken::new(),
        });
        // No workers exist until recovery has committed all interrupted snapshots.
        for mut job in manager.repository.list().await? {
            if job.status == JobStatus::Downloading {
                job.transition(JobStatus::Queued)?;
                job.progress = 0.0;
                job.error = Some(AppError {
                    code: ErrorCode::Interrupted,
                    message:
                        "Previous session ended during this transfer. Queued for a fresh attempt."
                            .into(),
                    retryable: true,
                });
                manager.repository.save(&job).await?;
            }
        }
        Ok(manager)
    }
    pub fn start(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(200));
            loop {
                tokio::select! {
                    _ = manager.shutdown.cancelled() => break,
                    _ = interval.tick() => { if let Err(error) = manager.schedule().await { eprintln!("Downpour scheduler: {error}"); } }
                }
            }
        })
    }
    pub async fn stop(&self) {
        self.shutdown.cancel();
        let runtime = self.runtime.lock().await;
        for token in runtime.active.values() {
            token.cancel();
        }
    }
    pub async fn snapshot(&self) -> Result<Snapshot, AppError> {
        let _guard = self.runtime.lock().await;
        Ok(Snapshot {
            jobs: self.repository.list().await?,
            settings: self.repository.settings().await?,
            engine: "mock".into(),
            persistence: "sqlite".into(),
        })
    }
    pub async fn inspect(&self, url: &str) -> Result<Media, AppError> {
        self.extractor.inspect(url).await
    }
    pub async fn enqueue(&self, url: &str, format_id: &str) -> Result<Job, AppError> {
        let media = self.extractor.inspect(url).await?;
        let format = media
            .formats
            .iter()
            .find(|f| f.id == format_id)
            .cloned()
            .ok_or_else(|| AppError::validation("Choose an available media format."))?;
        let timestamp = now();
        let job = Job {
            id: uuid::Uuid::new_v4().to_string(),
            is_mock: media.is_mock,
            media,
            format,
            status: JobStatus::Queued,
            progress: 0.0,
            attempt: 0,
            revision: 0,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            error: None,
        };
        let _guard = self.runtime.lock().await;
        self.repository.insert(&job).await?;
        (self.emit)(job.clone());
        Ok(job)
    }
    pub async fn cancel(&self, id: &str) -> Result<Job, AppError> {
        let runtime = self.runtime.lock().await;
        let mut job = self.repository.get(id).await?;
        job.transition(JobStatus::Cancelled)?;
        self.repository.save(&job).await?;
        if let Some(token) = runtime.active.get(id) {
            token.cancel();
        }
        (self.emit)(job.clone());
        Ok(job)
    }
    pub async fn retry(&self, id: &str) -> Result<Job, AppError> {
        let _guard = self.runtime.lock().await;
        let mut job = self.repository.get(id).await?;
        if !matches!(job.status, JobStatus::Failed | JobStatus::Cancelled) {
            return Err(AppError::validation(
                "Only failed or cancelled downloads can be retried.",
            ));
        }
        job.transition(JobStatus::Queued)?;
        job.progress = 0.0;
        job.error = None;
        self.repository.save(&job).await?;
        (self.emit)(job.clone());
        Ok(job)
    }
    pub async fn update_settings(&self, settings: Settings) -> Result<Settings, AppError> {
        let _guard = self.runtime.lock().await;
        self.repository.save_settings(&settings).await?;
        Ok(settings)
    }
    async fn schedule(self: &Arc<Self>) -> Result<(), AppError> {
        let mut runtime = self.runtime.lock().await;
        let concurrency = usize::from(self.repository.settings().await?.concurrency);
        if runtime.active.len() >= concurrency {
            return Ok(());
        }
        // Only scan indexed queued rows, never the entire completed history.
        // Include occupied IDs because an immediately retried job may still be
        // waiting for its cancelled worker to exit.
        for mut job in self.repository.queued(concurrency).await? {
            if runtime.active.len() >= concurrency {
                break;
            }
            if job.status != JobStatus::Queued || runtime.active.contains_key(&job.id) {
                continue;
            }
            job.transition(JobStatus::Downloading)?;
            job.attempt += 1;
            job.error = None;
            self.repository.save(&job).await?;
            let cancel = self.shutdown.child_token();
            runtime.active.insert(job.id.clone(), cancel.clone());
            (self.emit)(job.clone());
            let manager = Arc::clone(self);
            tokio::spawn(async move {
                manager.work(job, cancel).await;
            });
        }
        Ok(())
    }
    async fn work(self: Arc<Self>, job: Job, cancel: CancellationToken) {
        let (sender, mut receiver) = mpsc::channel::<TransferProgress>(8);
        let engine = Arc::clone(&self.engine);
        let request = job.clone();
        let token = cancel.clone();
        let mut worker = tokio::spawn(async move { engine.run(request, sender, token).await });
        let result = loop {
            tokio::select! {
                result = &mut worker => break result.unwrap_or_else(|_| Err(AppError { code: ErrorCode::Internal, message: "Transfer worker stopped unexpectedly.".into(), retryable: true })),
                Some(progress) = receiver.recv() => {
                    if let Err(error) = self.progress(&job.id, progress.percent).await {
                        cancel.cancel();
                        // Await child termination; releasing its slot early would exceed the bound.
                        let _ = worker.await;
                        break Err(error);
                    }
                }
            }
        };
        if let Err(error) = self.finish(&job.id, result).await {
            eprintln!("Downpour worker persistence: {error}");
        }
        self.runtime.lock().await.active.remove(&job.id);
    }
    async fn progress(&self, id: &str, percent: f64) -> Result<(), AppError> {
        let _guard = self.runtime.lock().await;
        let mut job = self.repository.get(id).await?;
        if job.status != JobStatus::Downloading {
            return Ok(());
        }
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err(AppError::validation("Engine reported invalid progress."));
        }
        job.progress = job.progress.max(percent).min(99.0);
        job.touch();
        self.repository.save(&job).await?;
        (self.emit)(job);
        Ok(())
    }
    async fn finish(&self, id: &str, result: Result<(), AppError>) -> Result<(), AppError> {
        let _guard = self.runtime.lock().await;
        let mut job = self.repository.get(id).await?;
        if job.status != JobStatus::Downloading {
            return Ok(());
        }
        match result {
            Ok(()) => {
                job.transition(JobStatus::Completed)?;
                job.progress = 100.0;
            }
            Err(error) => {
                job.transition(JobStatus::Failed)?;
                job.error = Some(error);
            }
        }
        self.repository.save(&job).await?;
        (self.emit)(job);
        Ok(())
    }
}
