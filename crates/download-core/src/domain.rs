use chrono::Utc;
use serde::{Deserialize, Serialize};
use url::{Host, Url};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u32>,
}

impl AppError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Validation,
            message: message.into(),
            retryable: false,
            retry_after_seconds: None,
        }
    }
    pub fn storage(error: impl std::fmt::Display) -> Self {
        eprintln!("Downpour storage error: {error}");
        Self { code: ErrorCode::Storage, message: "Local storage is unavailable. Check disk space and app-data permissions, then restart.".into(), retryable: true, retry_after_seconds: None }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for AppError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Validation,
    Storage,
    Network,
    RateLimited,
    Unsupported,
    Interrupted,
    Internal,
    Dependency,
    DiskFull,
    Integrity,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Youtube,
    Instagram,
    Facebook,
    Tiktok,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaFormat {
    pub id: String,
    pub label: String,
    pub extension: String,
    pub quality: String,
    pub estimated_bytes: u64,
    pub audio_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub url: String,
    pub title: String,
    pub creator: String,
    pub platform: Platform,
    pub duration_seconds: u64,
    pub formats: Vec<MediaFormat>,
    pub is_mock: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn permits(self, next: Self) -> bool {
        use JobStatus::*;
        matches!(
            (self, next),
            (Queued, Downloading | Cancelled)
                | (Downloading, Completed | Failed | Cancelled | Queued)
                | (Failed | Cancelled, Queued)
        )
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub media: Media,
    pub format: MediaFormat,
    pub status: JobStatus,
    pub progress: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_bytes_per_second: Option<f64>,
    pub attempt: u32,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub error: Option<AppError>,
    pub is_mock: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutputFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub available: bool,
    pub message: String,
    pub output_directory: Option<String>,
    pub tools: Vec<ToolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
}

impl Job {
    pub fn transition(&mut self, status: JobStatus) -> Result<(), AppError> {
        if !self.status.permits(status) {
            return Err(AppError::validation(format!(
                "Cannot change {} to {}.",
                self.status.as_str(),
                status.as_str()
            )));
        }
        self.status = status;
        if status != JobStatus::Downloading {
            self.speed_bytes_per_second = None;
        }
        self.touch();
        Ok(())
    }
    pub fn touch(&mut self) {
        self.revision += 1;
        self.updated_at = now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub concurrency: u8,
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            concurrency: 2,
            theme: Theme::Dark,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), AppError> {
        if !(1..=4).contains(&self.concurrency) {
            return Err(AppError::validation(
                "Choose between 1 and 4 simultaneous downloads.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub jobs: Vec<Job>,
    pub settings: Settings,
    pub engine: String,
    pub persistence: String,
    pub engine_info: EngineInfo,
    pub health: Option<AppError>,
}

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn validate_url(input: &str) -> Result<Url, AppError> {
    let invalid = || {
        AppError::validation(
            "Use a public http or https video link, without sign-in details or a custom port.",
        )
    };
    let trimmed = input.trim();
    if trimmed.len() > 2048 {
        return Err(invalid());
    }
    let mut url = Url::parse(trimmed).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err(invalid());
    }
    let host = match url.host() {
        Some(Host::Domain(domain)) => domain,
        _ => return Err(invalid()),
    };
    let reserved = [
        "localhost",
        "local",
        "internal",
        "test",
        "invalid",
        "example",
        "onion",
        "home",
        "lan",
    ];
    if !host.contains('.')
        || !host.ends_with(|c: char| c.is_ascii_alphabetic())
        || !host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
        || host
            .split('.')
            .any(|s| s.is_empty() || s.starts_with('-') || s.ends_with('-'))
        || reserved
            .iter()
            .any(|s| host == *s || host.ends_with(&format!(".{s}")))
    {
        return Err(invalid());
    }
    url.set_fragment(None);
    Ok(url)
}

pub fn detect_platform(url: &Url) -> Platform {
    let host = url.host_str().unwrap_or_default();
    let matches = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    if matches("youtube.com") || matches("youtu.be") {
        Platform::Youtube
    } else if matches("instagram.com") {
        Platform::Instagram
    } else if matches("facebook.com") || matches("fb.watch") {
        Platform::Facebook
    } else if matches("tiktok.com") {
        Platform::Tiktok
    } else {
        Platform::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_urls_and_detects_platform_by_domain_boundary() {
        for url in [
            "",
            "file:///etc/passwd",
            "https://localhost/a",
            "http://127.1/a",
            "https://[::1]/",
            "https://user:pass@youtube.com/",
            "https://youtube.com:123/a",
            "https://a.local/a",
            "https://a.internal/a",
            "https://a.test/a",
            "https://-bad.com/a",
            "https://foo.com./a",
            "https://2130706433/a",
        ] {
            assert!(validate_url(url).is_err(), "{url}");
        }
        assert_eq!(
            detect_platform(&validate_url("https://www.youtube.com/watch?v=1").unwrap()),
            Platform::Youtube
        );
        assert_eq!(
            detect_platform(&validate_url("https://youtube.com.evil.org/a").unwrap()),
            Platform::Other
        );
        assert_eq!(
            validate_url(" https://youtu.be/one#fragment ")
                .unwrap()
                .as_str(),
            "https://youtu.be/one"
        );
    }
    #[test]
    fn transition_table_is_exhaustive() {
        use JobStatus::*;
        let states = [Queued, Downloading, Completed, Failed, Cancelled];
        let expected = [
            (Queued, Downloading),
            (Queued, Cancelled),
            (Downloading, Completed),
            (Downloading, Failed),
            (Downloading, Cancelled),
            (Downloading, Queued),
            (Failed, Queued),
            (Cancelled, Queued),
        ];
        for from in states {
            for to in states {
                assert_eq!(from.permits(to), expected.contains(&(from, to)));
            }
        }
    }
}
