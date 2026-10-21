use crate::{
    files::{regular_file, FileStore},
    inspection::{Inspection, InspectionCache},
    metadata,
    network::PublicProxy,
    process::{args, classify, failure, run, run_with_input, LineSink},
    tools::{verify, Toolchain},
    youtube_requests::{canonical_url, YoutubeRequests},
};
use async_trait::async_trait;
use download_core::{
    engine::{MockEngine, TransferEngine, TransferProgress},
    extractor::Extractor,
    *,
};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{mpsc, Mutex, OnceCell, Semaphore};
use tokio_util::sync::CancellationToken;

pub struct YtdlpBackend {
    tools: OnceCell<Toolchain>,
    manifest: PathBuf,
    tool_error: std::sync::Mutex<Option<AppError>>,
    files: FileStore,
    inspections: Semaphore,
    youtube: YoutubeRequests,
    cache: Mutex<InspectionCache>,
}
impl YtdlpBackend {
    pub async fn new(manifest: &Path, output: &Path) -> Result<Self, AppError> {
        let files = FileStore::new(output).await?;
        #[cfg(debug_assertions)]
        eprintln!("Downpour: checking media tools");
        let backend = Self {
            tools: OnceCell::new(),
            manifest: manifest.to_owned(),
            tool_error: std::sync::Mutex::new(None),
            files,
            inspections: Semaphore::new(2),
            youtube: YoutubeRequests::default(),
            cache: Mutex::new(InspectionCache::default()),
        };
        // Hash verification establishes availability. Launching four processes
        // here made a transient startup self-test disable search for the session.
        // Actual operations still verify executables immediately before use.
        let _ = backend.check_tools().await;
        Ok(backend)
    }
    pub fn files(&self) -> &FileStore {
        &self.files
    }
    async fn toolchain(&self) -> Result<&Toolchain, AppError> {
        self.tools
            .get_or_try_init(|| async {
                let result = Toolchain::load(&self.manifest).await;
                *self.tool_error.lock().expect("tool error lock") = result.as_ref().err().cloned();
                result
            })
            .await
    }
    pub async fn check_tools(&self) -> Result<(), AppError> {
        self.toolchain().await.map(|_| ())
    }
    fn base(&self, tools: &Toolchain, proxy: &PublicProxy, youtube: bool) -> Vec<OsString> {
        let mut values = args(&[
            "--ignore-config",
            "--no-plugin-dirs",
            "--no-cookies",
            "--no-cookies-from-browser",
            "--no-cache-dir",
            "--no-remote-components",
            "--no-js-runtimes",
            "--no-playlist",
            "--socket-timeout",
            "20",
            "--retries",
            "1",
            "--fragment-retries",
            "1",
            "--extractor-retries",
            "1",
            "--use-extractors",
            "youtube.*,facebook.*,instagram.*,tiktok.*,generic",
            "--js-runtimes",
        ]);
        values.push(OsString::from(format!(
            "deno:{}",
            tools.tool("deno").path.display()
        )));
        values.extend(args(&["--proxy", &proxy.url]));
        if youtube {
            values.extend(args(&["--sleep-requests", "1"]));
        }
        values
    }
    async fn inspection(
        &self,
        source: &str,
        cancel: CancellationToken,
    ) -> Result<Inspection, AppError> {
        let url = canonical_url(validate_url(source)?);
        if cancel.is_cancelled() {
            return Err(failure(ErrorCode::Interrupted, "Transfer cancelled.", true));
        }
        if let Some(inspection) = self.cache.lock().await.get(url.as_str()) {
            return Ok(inspection);
        }
        let permit = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(failure(ErrorCode::Interrupted, "Transfer cancelled.", true)),
            permit = tokio::time::timeout(Duration::from_secs(10), self.inspections.acquire()) => permit,
        };
        let _permit = permit
            .map_err(|_| {
                failure(
                    ErrorCode::Network,
                    "The media inspector is busy. Try again in a moment.",
                    true,
                )
            })?
            .map_err(|_| failure(ErrorCode::Internal, "The media inspector stopped.", true))?;
        let tools = self.toolchain().await?;
        verify(tools.tool("yt-dlp")).await?;
        verify(tools.tool("deno")).await?;
        let youtube = detect_platform(&url) == Platform::Youtube;
        let mut request = tokio::time::timeout(
            Duration::from_secs(10),
            self.youtube.acquire(youtube, &cancel),
        )
        .await
        .map_err(|_| {
            failure(
                ErrorCode::Network,
                "Another YouTube request is in progress. Try again after it finishes.",
                true,
            )
        })??;
        // A concurrent lookup may have populated the cache while this request
        // waited for the serialized YouTube operation.
        if let Some(inspection) = self.cache.lock().await.get(url.as_str()) {
            return Ok(inspection);
        }
        let proxy = PublicProxy::start().await?;
        let mut values = self.base(tools, &proxy, youtube);
        values.extend(args(&[
            "--dump-single-json",
            "--skip-download",
            "--",
            url.as_str(),
        ]));
        let output = run(
            &tools.tool("yt-dlp").path,
            &values,
            self.files.root(),
            Duration::from_secs(60),
            cancel,
            None,
        )
        .await?;
        if !output.success {
            let error = proxy.error().unwrap_or_else(|| classify(&output.stderr));
            return Err(request
                .as_mut()
                .map_or_else(|| error.clone(), |request| request.failed(error.clone())));
        }
        let media = metadata::parse(&output.stdout, url.as_str())?;
        let transfer = metadata::download_info(&output.stdout, &media)?;
        if let Some(request) = &mut request {
            request.succeeded();
        }
        let inspection = Inspection {
            media,
            transfer: transfer.into(),
            created: std::time::Instant::now(),
        };
        self.cache
            .lock()
            .await
            .insert(url.to_string(), inspection.clone());
        Ok(inspection)
    }
    async fn download(
        &self,
        job: Job,
        progress: mpsc::Sender<TransferProgress>,
        cancel: CancellationToken,
    ) -> Result<Option<OutputFile>, AppError> {
        if job.is_mock {
            MockEngine::default().run(job, progress, cancel).await?;
            return Ok(None);
        }
        let source = validate_url(&job.media.url)?;
        let youtube = detect_platform(&source) == Platform::Youtube;
        let tools = self.toolchain().await?;
        for name in ["yt-dlp", "deno", "ffmpeg", "ffprobe"] {
            verify(tools.tool(name)).await?;
        }
        if let Some(output) = self.files.recover(&job, &cancel).await? {
            return Ok(Some(output));
        }
        let inspection = self.inspection(&job.media.url, cancel.clone()).await?;
        if !inspection.media.formats.iter().any(|format| {
            format.id == job.format.id
                && format.extension == job.format.extension
                && format.audio_only == job.format.audio_only
        }) {
            return Err(failure(ErrorCode::Unsupported, "The selected format is no longer available. Inspect the link and choose a format again.", false));
        }
        let directory = self.files.prepare(&job).await?;
        let parts = metadata::format_parts(&job.format.id)?;
        let proxy = PublicProxy::start().await?;
        let mut sources: Vec<PathBuf> = vec![];
        for (index, part) in parts.iter().enumerate() {
            let mut request = self.youtube.acquire(youtube, &cancel).await?;
            let source = directory.join(format!("stream-{index}.download"));
            let sender = progress.clone();
            let fraction = 88.0 / parts.len() as f64;
            let offset = index as f64 * fraction;
            let sink: LineSink = Arc::new(move |line| {
                if let Some(sample) = parse_progress(line, offset, fraction) {
                    let _ = sender.try_send(sample);
                }
            });
            let mut values = self.base(tools, &proxy, youtube);
            let selector = format!("{part}[protocol^=http]/{part}[protocol^=m3u8]");
            values.extend(args(&[
                "--downloader",
                "native",
                "--fixup",
                "never",
                "--abort-on-unavailable-fragments",
                "--max-filesize",
                "4G",
                "--no-mtime",
                "--no-simulate",
                "--newline",
                "--progress",
                "--progress-delta",
                "0.25",
                "--progress-template",
                "download:DOWNPOUR:{\"percent\":%(progress._percent_str)j,\"speed\":%(progress.speed)j}",
                "--format",
                &selector,
                "--output",
            ]));
            values.push(source.clone().into_os_string());
            values.extend(args(&["--load-info-json", "-"]));
            #[cfg(debug_assertions)]
            if let Ok(rate) = std::env::var("DOWNPOUR_TEST_RATE") {
                if rate == "512K" {
                    values.splice(0..0, args(&["--limit-rate", "512K"]));
                }
            }
            let output = run_with_input(
                &tools.tool("yt-dlp").path,
                &values,
                &directory,
                Duration::from_secs(1800),
                cancel.clone(),
                Some(sink),
                Some(Arc::clone(&inspection.transfer)),
            )
            .await?;
            if !output.success {
                let error = proxy.error().unwrap_or_else(|| classify(&output.stderr));
                return Err(request
                    .as_mut()
                    .map_or_else(|| error.clone(), |request| request.failed(error.clone())));
            }
            if let Some(request) = &mut request {
                request.succeeded();
            }
            if regular_file(&source).await? == 0 {
                return Err(failure(
                    ErrorCode::Integrity,
                    "The source returned an empty media file.",
                    true,
                ));
            }
            sources.push(source);
        }
        drop(proxy); // FFmpeg/FFprobe only process local files.
        let _ = progress
            .send(TransferProgress {
                percent: 92.0,
                speed_bytes_per_second: None,
            })
            .await;
        let staged = directory.join(format!("verified.{}", job.format.extension));
        if tokio::fs::try_exists(&staged)
            .await
            .map_err(AppError::storage)?
        {
            regular_file(&staged).await?;
            tokio::fs::remove_file(&staged)
                .await
                .map_err(AppError::storage)?;
        }
        let mut values = args(&["-nostdin", "-hide_banner", "-loglevel", "error", "-n"]);
        for source in &sources {
            values.extend(args(&[
                "-protocol_whitelist",
                "file",
                "-format_whitelist",
                "mov,matroska,webm,mpegts,aac,mp3,ogg",
                "-i",
            ]));
            values.push(source.clone().into_os_string());
        }
        if sources.len() == 2 {
            values.extend(args(&["-map", "0:v:0", "-map", "1:a:0"]));
        } else if job.format.audio_only {
            values.extend(args(&["-map", "0:a:0", "-vn"]));
        } else {
            values.extend(args(&["-map", "0:v:0", "-map", "0:a:0?"]));
        }
        values.extend(args(&[
            "-map_metadata",
            "-1",
            "-map_chapters",
            "-1",
            "-c",
            "copy",
        ]));
        if job.format.extension == "mp4" || job.format.extension == "m4a" {
            values.extend(args(&["-movflags", "+faststart"]));
        }
        values.push(staged.clone().into_os_string());
        let output = run(
            &tools.tool("ffmpeg").path,
            &values,
            &directory,
            Duration::from_secs(300),
            cancel.clone(),
            None,
        )
        .await?;
        if !output.success {
            return Err(failure(
                ErrorCode::Integrity,
                "Media processing failed. Choose another format or retry the source.",
                true,
            ));
        }
        let _ = progress
            .send(TransferProgress {
                percent: 97.0,
                speed_bytes_per_second: None,
            })
            .await;
        let mut values = args(&[
            "-v",
            "error",
            "-protocol_whitelist",
            "file",
            "-format_whitelist",
            "mov,matroska,webm,mpegts,aac,mp3,ogg",
            "-show_entries",
            "format=duration:stream=codec_type,codec_name",
            "-of",
            "json",
        ]);
        values.push(staged.clone().into_os_string());
        let probe = run(
            &tools.tool("ffprobe").path,
            &values,
            &directory,
            Duration::from_secs(30),
            cancel.clone(),
            None,
        )
        .await?;
        let info: serde_json::Value = serde_json::from_slice(&probe.stdout).map_err(|_| {
            failure(
                ErrorCode::Integrity,
                "Media verification returned invalid output.",
                true,
            )
        })?;
        let streams = info["streams"].as_array().ok_or_else(|| {
            failure(
                ErrorCode::Integrity,
                "No playable media streams were found.",
                true,
            )
        })?;
        let expected = if job.format.audio_only {
            "audio"
        } else {
            "video"
        };
        if !probe.success
            || !streams
                .iter()
                .any(|stream| stream["codec_type"] == expected)
        {
            return Err(failure(
                ErrorCode::Integrity,
                "The downloaded file does not contain the requested media.",
                true,
            ));
        }
        let duration = info["format"]["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|d| d.is_finite() && *d > 0.0)
            .ok_or_else(|| {
                failure(
                    ErrorCode::Integrity,
                    "The media file has no valid duration.",
                    true,
                )
            })?;
        self.files
            .publish(&job, &staged, duration, &cancel)
            .await
            .map(Some)
    }
}

/// Parse only the selected progress fields; unavailable values arrive as "NA".
pub(crate) fn parse_progress(line: &str, offset: f64, fraction: f64) -> Option<TransferProgress> {
    let payload = line.trim().strip_prefix("DOWNPOUR:")?;
    let sample: serde_json::Value = serde_json::from_str(payload).ok()?;
    let percent = sample
        .get("percent")?
        .as_str()
        .and_then(|value| {
            value
                .trim()
                .trim_end_matches('%')
                .trim()
                .parse::<f64>()
                .ok()
        })
        .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
        .unwrap_or(0.0);
    let speed = sample
        .get("speed")?
        .as_f64()
        .filter(|speed| speed.is_finite() && *speed >= 0.0);
    Some(TransferProgress {
        percent: offset + fraction * percent / 100.0,
        speed_bytes_per_second: speed,
    })
}

#[async_trait]
impl Extractor for YtdlpBackend {
    async fn inspect(&self, source: &str) -> Result<Media, AppError> {
        self.inspection(source, CancellationToken::new())
            .await
            .map(|inspection| inspection.media)
    }
}
#[async_trait]
impl TransferEngine for YtdlpBackend {
    fn name(&self) -> &'static str {
        "yt-dlp"
    }
    fn info(&self) -> EngineInfo {
        EngineInfo {
            available: self.tools.get().is_some(),
            message: self
                .tool_error
                .lock()
                .expect("tool error lock")
                .as_ref()
                .map(|error| error.message.clone())
                .unwrap_or_else(|| "Public media engine ready.".to_string()),
            output_directory: Some(self.files.root().to_string_lossy().into_owned()),
            tools: self.tools.get().map(Toolchain::info).unwrap_or_default(),
        }
    }
    async fn run(
        &self,
        job: Job,
        progress: mpsc::Sender<TransferProgress>,
        cancel: CancellationToken,
    ) -> Result<(), AppError> {
        self.download(job, progress, cancel).await.map(|_| ())
    }
    async fn transfer(
        &self,
        job: Job,
        progress: mpsc::Sender<TransferProgress>,
        cancel: CancellationToken,
    ) -> Result<Option<OutputFile>, AppError> {
        self.download(job, progress, cancel).await
    }
    async fn committed(&self, job: &Job) -> Result<(), AppError> {
        if job.is_mock {
            Ok(())
        } else {
            self.files.committed(job).await
        }
    }
    async fn discard(&self, job: &Job, output: &OutputFile) -> Result<(), AppError> {
        self.files.discard(job, output).await
    }
}
