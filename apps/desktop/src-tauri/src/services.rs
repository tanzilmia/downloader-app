use download_core::{
    manager::{EventSink, HealthSink},
    AppError, DownloadManager, ErrorCode, Repository,
};
use media_engine::ytdlp::YtdlpBackend;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, OnceCell};
use tokio_util::sync::CancellationToken;

pub struct Runtime {
    pub manager: Arc<DownloadManager>,
    pub engine: Option<Arc<YtdlpBackend>>,
}

/// Initialization runs on Tokio, keeping the macOS event loop free for webview
/// creation and OS permission prompts. Commands await the same initialization.
pub struct Services {
    pub destination_change: Mutex<()>,
    manifest: PathBuf,
    output: PathBuf,
    database: PathBuf,
    events: EventSink,
    health: HealthSink,
    inner: OnceCell<Runtime>,
    shutdown: CancellationToken,
}

impl Services {
    pub fn new(
        manifest: PathBuf,
        output: PathBuf,
        database: PathBuf,
        events: EventSink,
        health: HealthSink,
    ) -> Arc<Self> {
        Arc::new(Self {
            destination_change: Mutex::new(()),
            manifest,
            output,
            database,
            events,
            health,
            inner: OnceCell::new(),
            shutdown: CancellationToken::new(),
        })
    }
    pub async fn get(&self) -> Result<&Runtime, AppError> {
        self.inner.get_or_try_init(|| async {
            tokio::select! {
                _ = self.shutdown.cancelled() => Err(AppError { code: ErrorCode::Interrupted, message: "Application is closing.".into(), retryable: true, retry_after_seconds: None, }),
                result = self.initialize() => result,
            }
        }).await
    }
    async fn initialize(&self) -> Result<Runtime, AppError> {
        let engine = Arc::new(YtdlpBackend::new(&self.manifest, &self.output).await?);
        let repository = Repository::open(&self.database).await?;
        let manager = DownloadManager::new(
            repository,
            engine.clone(),
            engine.clone(),
            self.events.clone(),
        )
        .await?;
        manager.set_health_sink(self.health.clone());
        let _scheduler = manager.start();
        Ok(Runtime {
            manager,
            engine: Some(engine),
        })
    }
    pub async fn stop(&self) {
        self.shutdown.cancel();
        if let Some(runtime) = self.inner.get() {
            runtime.manager.stop().await;
            runtime.manager.wait_stopped().await;
        }
    }
    #[cfg(test)]
    pub fn from_manager(manager: Arc<DownloadManager>) -> Arc<Self> {
        let services = Self::new(
            PathBuf::new(),
            PathBuf::new(),
            PathBuf::new(),
            Arc::new(|_| {}),
            Arc::new(|_| {}),
        );
        services
            .inner
            .set(Runtime {
                manager,
                engine: None,
            })
            .ok()
            .expect("empty test services");
        services
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn startup_storage_failure_is_retryable_and_initialization_is_shared() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("repairable/history.db");
        let services = Services::new(
            directory.path().join("missing-engine.json"),
            directory.path().join("downloads"),
            database.clone(),
            Arc::new(|_| {}),
            Arc::new(|_| {}),
        );
        assert!(services.get().await.is_err());
        tokio::fs::create_dir(database.parent().unwrap())
            .await
            .unwrap();
        let (first, second) = tokio::join!(services.get(), services.get());
        assert!(Arc::ptr_eq(
            &first.unwrap().manager,
            &second.unwrap().manager
        ));
        assert!(
            !services
                .get()
                .await
                .unwrap()
                .manager
                .snapshot()
                .await
                .unwrap()
                .engine_info
                .available
        );
        services.stop().await;
    }
}
