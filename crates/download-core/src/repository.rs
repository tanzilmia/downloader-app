use crate::domain::*;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::{path::Path, time::Duration};

#[derive(Clone)]
pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub async fn open(path: &Path) -> Result<Self, AppError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(AppError::storage)?;
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(AppError::storage)?;
        Ok(Self { pool })
    }
    pub async fn list(&self) -> Result<Vec<Job>, AppError> {
        let rows = sqlx::query("SELECT payload FROM jobs ORDER BY created_at ASC, rowid ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::storage)?;
        rows.into_iter()
            .map(|r| serde_json::from_str(r.get::<&str, _>("payload")).map_err(AppError::storage))
            .collect()
    }
    pub async fn get(&self, id: &str) -> Result<Job, AppError> {
        let row = sqlx::query("SELECT payload FROM jobs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::storage)?
            .ok_or_else(|| AppError::validation("This download could not be found."))?;
        serde_json::from_str(row.get::<&str, _>("payload")).map_err(AppError::storage)
    }
    pub(crate) async fn queued(&self, limit: usize) -> Result<Vec<Job>, AppError> {
        let rows = sqlx::query("SELECT payload FROM jobs WHERE status = 'queued' ORDER BY created_at ASC, rowid ASC LIMIT ?")
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::storage)?;
        rows.into_iter()
            .map(|r| serde_json::from_str(r.get::<&str, _>("payload")).map_err(AppError::storage))
            .collect()
    }
    pub async fn insert(&self, job: &Job) -> Result<(), AppError> {
        sqlx::query("INSERT INTO jobs(id,status,created_at,payload) VALUES(?,?,?,?)")
            .bind(&job.id)
            .bind(job.status.as_str())
            .bind(&job.created_at)
            .bind(serde_json::to_string(job).map_err(AppError::storage)?)
            .execute(&self.pool)
            .await
            .map_err(AppError::storage)?;
        Ok(())
    }
    // Only the manager owns mutation ordering. Revisions are persisted with snapshots.
    pub(crate) async fn save(&self, job: &Job) -> Result<(), AppError> {
        let result = sqlx::query("UPDATE jobs SET status = ?, payload = ? WHERE id = ?")
            .bind(job.status.as_str())
            .bind(serde_json::to_string(job).map_err(AppError::storage)?)
            .bind(&job.id)
            .execute(&self.pool)
            .await
            .map_err(AppError::storage)?;
        if result.rows_affected() != 1 {
            return Err(AppError::validation("This download no longer exists."));
        }
        Ok(())
    }
    pub async fn settings(&self) -> Result<Settings, AppError> {
        let payload: String = sqlx::query_scalar("SELECT payload FROM settings WHERE id = 1")
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::storage)?;
        let settings: Settings = serde_json::from_str(&payload).map_err(AppError::storage)?;
        settings.validate()?;
        Ok(settings)
    }
    pub(crate) async fn save_settings(&self, settings: &Settings) -> Result<(), AppError> {
        settings.validate()?;
        sqlx::query("UPDATE settings SET payload = ? WHERE id = 1")
            .bind(serde_json::to_string(settings).map_err(AppError::storage)?)
            .execute(&self.pool)
            .await
            .map_err(AppError::storage)?;
        Ok(())
    }
    pub async fn close(&self) {
        self.pool.close().await;
    }
}
