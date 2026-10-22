use crate::{process::failure, tools::sha256};
use download_core::{files::safe_filename_stem, AppError, ErrorCode, Job, OutputFile};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct FileStore {
    root: PathBuf,
}

impl FileStore {
    pub async fn new(root: &Path) -> Result<Self, AppError> {
        tokio::fs::create_dir_all(root)
            .await
            .map_err(AppError::storage)?;
        let root = tokio::fs::canonicalize(root)
            .await
            .map_err(AppError::storage)?;
        let store = Self { root };
        private_dir(&store.root.join(".partial")).await?;
        Ok(store)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// A queued job owns its destination, independently of later preferences.
    /// Never recreate a missing selected folder (for example an unplugged drive)
    /// or follow a replacement symlink to a different destination.
    pub async fn for_job(&self, job: &Job) -> Result<Self, AppError> {
        let Some(directory) = &job.download_directory else {
            return Ok(self.clone());
        };
        let path = Path::new(directory);
        let canonical = tokio::fs::canonicalize(path).await.map_err(|_| {
            failure(ErrorCode::Storage, "The download folder is unavailable. Reconnect the drive or restore the folder, then retry.", true)
        })?;
        if !path.is_absolute() || canonical != path || !canonical.is_dir() {
            return Err(failure(ErrorCode::Storage, "The download folder has moved or changed. Restore its original location before retrying.", false));
        }
        Ok(Self { root: canonical })
    }
    /// Check a picker-selected existing folder without changing its permissions.
    /// Publication requires write access and same-volume hard links.
    pub async fn check_destination(path: &Path) -> Result<PathBuf, AppError> {
        let invalid = || {
            failure(ErrorCode::Storage, "This folder cannot safely save downloads. Choose a different writable folder or drive.", true)
        };
        if !path.is_absolute() {
            return Err(invalid());
        }
        let root = tokio::fs::canonicalize(path).await.map_err(|_| invalid())?;
        if !root.is_dir() || root.to_str().is_none() {
            return Err(invalid());
        }
        let probe = root.join(format!(".downpour-check-{}", uuid::Uuid::new_v4()));
        let link = probe.with_extension("link");
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&probe)
            .await
            .map_err(|_| invalid())?;
        let result = async {
            file.write_all(b"Downpour folder check").await?;
            file.sync_all().await?;
            tokio::fs::hard_link(&probe, &link).await
        }
        .await;
        drop(file);
        let removed = tokio::fs::remove_file(&probe).await;
        let unlinked = if result.is_ok() {
            tokio::fs::remove_file(&link).await
        } else {
            Ok(())
        };
        result.map_err(|_| invalid())?;
        removed.map_err(|_| invalid())?;
        unlinked.map_err(|_| invalid())?;
        Ok(root)
    }
    fn job_dir(&self, job: &Job) -> Result<PathBuf, AppError> {
        let id = uuid::Uuid::parse_str(&job.id)
            .map_err(|_| failure(ErrorCode::Validation, "Invalid download identity.", false))?;
        if id.to_string() != job.id {
            return Err(failure(
                ErrorCode::Validation,
                "Invalid download identity.",
                false,
            ));
        }
        Ok(self.root.join(".partial").join(&job.id))
    }
    pub fn final_path(&self, job: &Job) -> Result<PathBuf, AppError> {
        self.job_dir(job)?;
        if !["mp4", "webm", "m4a"].contains(&job.format.extension.as_str()) {
            return Err(failure(
                ErrorCode::Validation,
                "Unsupported output extension.",
                false,
            ));
        }
        let mut stem = safe_filename_stem(&job.media.title);
        while stem.len() > 120 {
            stem.pop();
        }
        Ok(self
            .root
            .join(format!("{}-{}.{}", stem, job.id, job.format.extension)))
    }
    pub async fn prepare(&self, job: &Job) -> Result<PathBuf, AppError> {
        self.prepare_directory(job, true).await
    }
    async fn prepare_directory(&self, job: &Job, check_space: bool) -> Result<PathBuf, AppError> {
        private_dir(&self.root.join(".partial")).await?;
        let directory = self.job_dir(job)?;
        private_dir(&directory).await?;
        let free = fs2::available_space(&directory).map_err(AppError::storage)?;
        let required = job
            .format
            .estimated_bytes
            .saturating_mul(2)
            .saturating_add(128 * 1024 * 1024);
        if check_space && free < required {
            return Err(failure(ErrorCode::DiskFull, "There is not enough free space for this download and processing. Free space, then retry.", true));
        }
        for entry in std::fs::read_dir(&directory).map_err(AppError::storage)? {
            let entry = entry.map_err(AppError::storage)?;
            if entry.file_type().map_err(AppError::storage)?.is_symlink() {
                return Err(failure(
                    ErrorCode::Storage,
                    "A partial-download path is a symbolic link. Remove it before retrying.",
                    false,
                ));
            }
        }
        Ok(directory)
    }
    pub async fn recover(
        &self,
        job: &Job,
        cancel: &CancellationToken,
    ) -> Result<Option<OutputFile>, AppError> {
        let directory = self.job_dir(job)?;
        if !tokio::fs::try_exists(directory.join("receipt.json"))
            .await
            .map_err(AppError::storage)?
        {
            return Ok(None);
        }
        let _ = self.prepare_directory(job, false).await?;
        let receipt_path = directory.join("receipt.json");
        if regular_file(&receipt_path).await? > 16384 {
            return Err(failure(
                ErrorCode::Integrity,
                "The download recovery receipt is invalid.",
                false,
            ));
        }
        let receipt = tokio::fs::read(receipt_path)
            .await
            .map_err(AppError::storage)?;
        if receipt.len() > 16384 {
            return Err(failure(
                ErrorCode::Integrity,
                "The download recovery receipt is invalid.",
                false,
            ));
        }
        let output: OutputFile = serde_json::from_slice(&receipt).map_err(|_| {
            failure(
                ErrorCode::Integrity,
                "The download recovery receipt is invalid.",
                false,
            )
        })?;
        let final_path = self.final_path(job)?;
        if Path::new(&output.path) != final_path {
            return Err(failure(
                ErrorCode::Integrity,
                "The output receipt points outside its download.",
                false,
            ));
        }
        let staged = directory.join(format!("verified.{}", job.format.extension));
        let candidate = if tokio::fs::try_exists(&final_path)
            .await
            .map_err(AppError::storage)?
        {
            &final_path
        } else {
            &staged
        };
        regular_file(candidate).await?;
        let (bytes, hash) = sha256(candidate, cancel).await?;
        if bytes != output.bytes || hash != output.sha256 || bytes == 0 {
            return Err(failure(ErrorCode::Integrity, "A recovered file failed its integrity check. It has been preserved for inspection.", false));
        }
        if candidate == &staged {
            tokio::fs::hard_link(&staged, &final_path)
                .await
                .map_err(AppError::storage)?;
        }
        Ok(Some(output))
    }
    pub async fn publish(
        &self,
        job: &Job,
        staged: &Path,
        duration: f64,
        cancel: &CancellationToken,
    ) -> Result<OutputFile, AppError> {
        let directory = self.job_dir(job)?;
        if staged != directory.join(format!("verified.{}", job.format.extension)) {
            return Err(failure(
                ErrorCode::Validation,
                "Unexpected staging file.",
                false,
            ));
        }
        regular_file(staged).await?;
        let (bytes, hash) = sha256(staged, cancel).await?;
        if bytes == 0 {
            return Err(failure(
                ErrorCode::Integrity,
                "The downloaded file is empty.",
                true,
            ));
        }
        let destination = self.final_path(job)?;
        if tokio::fs::try_exists(&destination)
            .await
            .map_err(AppError::storage)?
        {
            return Err(failure(
                ErrorCode::Storage,
                "The output filename already exists. It will not be overwritten.",
                false,
            ));
        }
        let output = OutputFile {
            path: destination.to_string_lossy().into_owned(),
            bytes,
            sha256: hash,
            duration_seconds: duration,
        };
        // Receipt precedes publication so a crash between link and DB commit is recoverable.
        atomic_write(
            &directory.join("receipt.json"),
            &serde_json::to_vec(&output).map_err(AppError::storage)?,
        )
        .await?;
        if cancel.is_cancelled() {
            return Err(failure(ErrorCode::Interrupted, "Transfer cancelled.", true));
        }
        // Same-volume hard_link atomically creates the destination without overwriting.
        tokio::fs::hard_link(staged, &destination)
            .await
            .map_err(AppError::storage)?;
        tokio::fs::File::open(&destination)
            .await
            .map_err(AppError::storage)?
            .sync_all()
            .await
            .map_err(AppError::storage)?;
        Ok(output)
    }
    pub async fn validate_output(&self, job: &Job, full_hash: bool) -> Result<PathBuf, AppError> {
        let output = job.output.as_ref().ok_or_else(|| {
            failure(
                ErrorCode::Validation,
                "This download has no saved media file.",
                false,
            )
        })?;
        let path = self.final_path(job)?;
        if Path::new(&output.path) != path {
            return Err(failure(
                ErrorCode::Integrity,
                "The saved file location does not match this download.",
                false,
            ));
        }
        let size = regular_file(&path).await?;
        if size != output.bytes || size == 0 {
            return Err(failure(
                ErrorCode::Integrity,
                "The saved media file is missing or has changed.",
                false,
            ));
        }
        if full_hash && sha256(&path, &CancellationToken::new()).await?.1 != output.sha256 {
            return Err(failure(
                ErrorCode::Integrity,
                "The saved media file failed its checksum check.",
                false,
            ));
        }
        Ok(path)
    }
    pub async fn committed(&self, job: &Job) -> Result<(), AppError> {
        self.validate_output(job, false).await?;
        let directory = self.job_dir(job)?;
        if tokio::fs::try_exists(&directory)
            .await
            .map_err(AppError::storage)?
        {
            private_dir(&directory).await?;
            tokio::fs::remove_dir_all(directory)
                .await
                .map_err(AppError::storage)?;
        }
        Ok(())
    }
    pub async fn discard(&self, job: &Job, output: &OutputFile) -> Result<(), AppError> {
        let mut owned = job.clone();
        owned.output = Some(output.clone());
        let path = self.validate_output(&owned, true).await?;
        tokio::fs::remove_file(path)
            .await
            .map_err(AppError::storage)?;
        let directory = self.job_dir(job)?;
        for name in [
            "receipt.json".to_string(),
            format!("verified.{}", job.format.extension),
        ] {
            let path = directory.join(name);
            if tokio::fs::try_exists(&path)
                .await
                .map_err(AppError::storage)?
            {
                regular_file(&path).await?;
                tokio::fs::remove_file(path)
                    .await
                    .map_err(AppError::storage)?;
            }
        }
        Ok(())
    }
}

pub async fn regular_file(path: &Path) -> Result<u64, AppError> {
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|_| {
        failure(
            ErrorCode::Storage,
            "The media file could not be found or read.",
            true,
        )
    })?;
    if !metadata.is_file() || metadata.is_symlink() {
        return Err(failure(
            ErrorCode::Integrity,
            "Media files must be regular files, not symbolic links.",
            false,
        ));
    }
    Ok(metadata.len())
}
async fn private_dir(path: &Path) -> Result<(), AppError> {
    let builder = tokio::fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    match builder.create(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(AppError::storage(error)),
    }
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(AppError::storage)?;
    if !metadata.is_dir() || metadata.is_symlink() {
        return Err(failure(
            ErrorCode::Storage,
            "The download working directory is unsafe.",
            false,
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o777 != 0o700 {
            tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
                .await
                .map_err(AppError::storage)?;
        }
    }
    Ok(())
}
async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .await
        .map_err(AppError::storage)?;
    file.write_all(bytes).await.map_err(AppError::storage)?;
    file.sync_all().await.map_err(AppError::storage)?;
    tokio::fs::rename(temporary, path)
        .await
        .map_err(AppError::storage)
}
