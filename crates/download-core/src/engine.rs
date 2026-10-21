use crate::domain::*;
use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct TransferProgress {
    pub percent: f64,
    pub speed_bytes_per_second: Option<f64>,
}

/// Real implementations must return only after output validation/finalization.
/// The mock has no artifact: it deliberately never creates a media file.
#[async_trait]
pub trait TransferEngine: Send + Sync {
    fn name(&self) -> &'static str {
        "mock"
    }
    fn info(&self) -> EngineInfo {
        EngineInfo {
            available: true,
            message: "Simulated transfers. No media files are created.".into(),
            output_directory: None,
            tools: vec![],
        }
    }
    async fn transfer(
        &self,
        job: Job,
        progress: mpsc::Sender<TransferProgress>,
        cancel: CancellationToken,
    ) -> Result<Option<OutputFile>, AppError> {
        self.run(job, progress, cancel).await?;
        Ok(None)
    }
    async fn discard(&self, _job: &Job, _output: &OutputFile) -> Result<(), AppError> {
        Ok(())
    }
    async fn committed(&self, _job: &Job) -> Result<(), AppError> {
        Ok(())
    }
    async fn run(
        &self,
        job: Job,
        progress: mpsc::Sender<TransferProgress>,
        cancel: CancellationToken,
    ) -> Result<(), AppError>;
}

pub struct MockEngine {
    pub step_duration: Duration,
}
impl Default for MockEngine {
    fn default() -> Self {
        Self {
            step_duration: Duration::from_millis(350),
        }
    }
}

#[async_trait]
impl TransferEngine for MockEngine {
    async fn run(
        &self,
        _job: Job,
        progress: mpsc::Sender<TransferProgress>,
        cancel: CancellationToken,
    ) -> Result<(), AppError> {
        for step in 1..=20 {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(AppError { code: ErrorCode::Interrupted, message: "Transfer cancelled.".into(), retryable: true, retry_after_seconds: None, }),
                _ = tokio::time::sleep(self.step_duration) => {}
            }
            if progress
                .send(TransferProgress {
                    percent: f64::from(step * 5),
                    speed_bytes_per_second: None,
                })
                .await
                .is_err()
            {
                return Err(AppError {
                    code: ErrorCode::Internal,
                    message: "Progress receiver closed.".into(),
                    retryable: true,
                    retry_after_seconds: None,
                });
            }
        }
        Ok(())
    }
}
