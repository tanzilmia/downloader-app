use crate::{
    domain::*,
    engine::{TransferEngine, TransferProgress},
    extractor::Extractor,
    Repository,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

pub type EventSink = Arc<dyn Fn(Job) + Send + Sync>;
pub type HealthSink = Arc<dyn Fn(Option<AppError>) + Send + Sync>;
struct Runtime {
    active: HashMap<String, CancellationToken>,
    reconcile: HashSet<String>,
}

pub struct DownloadManager {
    repository: Repository,
    extractor: Arc<dyn Extractor>,
    engine: Arc<dyn TransferEngine>,
    runtime: Mutex<Runtime>,
    emit: EventSink,
    shutdown: CancellationToken,
    health: std::sync::Mutex<Option<AppError>>,
    health_sink: std::sync::Mutex<Option<HealthSink>>,
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
                reconcile: HashSet::new(),
            }),
            emit,
            shutdown: CancellationToken::new(),
            health: std::sync::Mutex::new(None),
            health_sink: std::sync::Mutex::new(None),
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
                    retry_after_seconds: None,
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
                    _ = interval.tick() => {
                        match manager.schedule().await {
                            Err(error) => { manager.report_health(Some(error)); tokio::time::sleep(Duration::from_secs(1)).await; },
                            Ok(()) => manager.report_health(None),
                        }
                    }
                }
            }
        })
    }
    pub fn set_health_sink(&self, sink: HealthSink) {
        *self.health_sink.lock().expect("health sink lock") = Some(sink);
    }
    fn report_health(&self, error: Option<AppError>) {
        let mut current = self.health.lock().expect("health lock");
        if *current == error {
            return;
        }
        *current = error.clone();
        if let Some(sink) = self.health_sink.lock().expect("health sink lock").as_ref() {
            sink(error);
        }
    }
    pub async fn stop(&self) {
        self.shutdown.cancel();
        let runtime = self.runtime.lock().await;
        for token in runtime.active.values() {
            token.cancel();
        }
    }
    pub async fn wait_stopped(&self) {
        let _ = tokio::time::timeout(Duration::from_secs(10), async {
            while !self.runtime.lock().await.active.is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
    }
    pub async fn snapshot(&self) -> Result<Snapshot, AppError> {
        let _guard = self.runtime.lock().await;
        Ok(Snapshot {
            jobs: self.repository.list().await?,
            settings: self.repository.settings().await?,
            engine: self.engine.name().into(),
            persistence: "sqlite".into(),
            engine_info: self.engine.info(),
            health: self.health.lock().expect("health lock").clone(),
        })
    }
    pub async fn inspect(&self, url: &str) -> Result<Media, AppError> {
        self.extractor.inspect(url).await
    }
    pub async fn job(&self, id: &str) -> Result<Job, AppError> {
        let _guard = self.runtime.lock().await;
        self.repository.get(id).await
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
            speed_bytes_per_second: None,
            attempt: 0,
            revision: 0,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            error: None,
            output: None,
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
        // A transient write failure must not strand a downloading row after its
        // worker exits. Receipts let the next attempt recover published media.
        for id in runtime.reconcile.clone() {
            if runtime.active.contains_key(&id) {
                continue;
            }
            let mut job = self.repository.get(&id).await?;
            if job.status == JobStatus::Downloading {
                job.transition(JobStatus::Queued)?;
                job.progress = 0.0;
                job.error = Some(AppError {
                    code: ErrorCode::Interrupted,
                    message: "Storage recovered. Resuming the interrupted transfer.".into(),
                    retryable: true,
                    retry_after_seconds: None,
                });
                self.repository.save(&job).await?;
                (self.emit)(job);
            } else if job.status == JobStatus::Completed {
                self.engine.committed(&job).await?;
            }
            runtime.reconcile.remove(&id);
        }
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
            job.speed_bytes_per_second = None;
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
        let mut worker = tokio::spawn(async move { engine.transfer(request, sender, token).await });
        let mut speed_expiry = tokio::time::interval(Duration::from_secs(1));
        let mut last_sample = tokio::time::Instant::now();
        let mut last_percent = job.progress;
        let mut has_speed = false;
        let result = loop {
            tokio::select! {
                result = &mut worker => break result.unwrap_or_else(|_| Err(AppError { code: ErrorCode::Internal, message: "Transfer worker stopped unexpectedly.".into(), retryable: true, retry_after_seconds: None, })),
                Some(progress) = receiver.recv() => {
                    last_sample = tokio::time::Instant::now();
                    last_percent = progress.percent;
                    has_speed = progress.speed_bytes_per_second.is_some();
                    if let Err(error) = self.progress(&job.id, progress).await {
                        cancel.cancel();
                        // Await child termination; releasing its slot early would exceed the bound.
                        let _ = worker.await;
                        break Err(error);
                    }
                }
                _ = speed_expiry.tick(), if has_speed => {
                    if last_sample.elapsed() >= Duration::from_secs(3) {
                        has_speed = false;
                        if let Err(error) = self.progress(&job.id, TransferProgress {
                            percent: last_percent,
                            speed_bytes_per_second: None,
                        }).await {
                            cancel.cancel();
                            let _ = worker.await;
                            break Err(error);
                        }
                    }
                }
            }
        };
        if let Err(error) = self.finish(&job.id, result).await {
            self.report_health(Some(error));
            self.runtime.lock().await.reconcile.insert(job.id.clone());
        }
        self.runtime.lock().await.active.remove(&job.id);
    }
    async fn progress(&self, id: &str, progress: TransferProgress) -> Result<(), AppError> {
        let _guard = self.runtime.lock().await;
        let mut job = self.repository.get(id).await?;
        if job.status != JobStatus::Downloading {
            return Ok(());
        }
        if !progress.percent.is_finite()
            || !(0.0..=100.0).contains(&progress.percent)
            || progress
                .speed_bytes_per_second
                .is_some_and(|speed| !speed.is_finite() || speed < 0.0)
        {
            return Err(AppError::validation("Engine reported invalid progress."));
        }
        job.progress = job.progress.max(progress.percent).min(99.0);
        job.speed_bytes_per_second = progress.speed_bytes_per_second;
        job.touch();
        self.repository.save(&job).await?;
        (self.emit)(job);
        Ok(())
    }
    async fn finish(
        &self,
        id: &str,
        result: Result<Option<OutputFile>, AppError>,
    ) -> Result<(), AppError> {
        let _guard = self.runtime.lock().await;
        let mut job = self.repository.get(id).await?;
        if job.status != JobStatus::Downloading {
            if let Ok(Some(output)) = result {
                self.engine.discard(&job, &output).await?;
            }
            return Ok(());
        }
        match result {
            Ok(output) => {
                job.transition(JobStatus::Completed)?;
                job.progress = 100.0;
                job.output = output;
            }
            Err(error) => {
                if self.shutdown.is_cancelled() && error.code == ErrorCode::Interrupted {
                    job.transition(JobStatus::Queued)?;
                    job.progress = 0.0;
                } else {
                    job.transition(JobStatus::Failed)?;
                }
                job.error = Some(error);
            }
        }
        self.repository.save(&job).await?;
        (self.emit)(job.clone());
        if job.status == JobStatus::Completed {
            self.engine.committed(&job).await?;
        }
        Ok(())
    }
}
