use command_group::AsyncCommandGroup;
use download_core::{AppError, ErrorCode};
use std::{ffi::OsString, path::Path, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use tokio_util::sync::CancellationToken;

pub fn failure(code: ErrorCode, message: &str, retryable: bool) -> AppError {
    AppError {
        code,
        message: message.into(),
        retryable,
        retry_after_seconds: None,
    }
}
pub type LineSink = Arc<dyn Fn(&str) + Send + Sync>;
pub struct ProcessOutput {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub success: bool,
}
struct GroupGuard {
    child: command_group::AsyncGroupChild,
    running: bool,
}
impl Drop for GroupGuard {
    fn drop(&mut self) {
        if self.running {
            let _ = self.child.start_kill();
        }
    }
}

pub async fn run(
    executable: &Path,
    args: &[OsString],
    cwd: &Path,
    timeout: Duration,
    cancel: CancellationToken,
    sink: Option<LineSink>,
) -> Result<ProcessOutput, AppError> {
    run_with_input(executable, args, cwd, timeout, cancel, sink, None).await
}

/// Metadata is passed in memory, never through an info file or process arguments.
pub async fn run_with_input(
    executable: &Path,
    args: &[OsString],
    cwd: &Path,
    timeout: Duration,
    cancel: CancellationToken,
    sink: Option<LineSink>,
    input: Option<Arc<[u8]>>,
) -> Result<ProcessOutput, AppError> {
    if input
        .as_ref()
        .is_some_and(|input| input.len() > 8 * 1024 * 1024)
    {
        return Err(failure(
            ErrorCode::Validation,
            "Media input exceeded its size limit.",
            false,
        ));
    }
    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Do not inherit proxy, Python/Node injection, configuration or credential env.
    let preserved: Vec<_> = [
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "TMP",
        "TEMP",
        "TMPDIR",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
    ]
    .iter()
    .filter_map(|key| std::env::var_os(key).map(|value| (*key, value)))
    .collect();
    command
        .env_clear()
        .envs(preserved)
        .env(
            "PATH",
            executable.parent().unwrap_or_else(|| Path::new(".")),
        )
        .env("HOME", cwd)
        .env("USERPROFILE", cwd)
        .env("XDG_CONFIG_HOME", cwd)
        .env("YTDLP_NO_PLUGINS", "1")
        .env("DENO_NO_UPDATE_CHECK", "1")
        .env("NO_COLOR", "1");
    let child = command.group().kill_on_drop(true).spawn().map_err(|_| {
        failure(
            ErrorCode::Dependency,
            "A verified media tool could not start. Repair the local engine installation.",
            false,
        )
    })?;
    let mut group = GroupGuard {
        child,
        running: true,
    };
    let input_cancel = CancellationToken::new();
    let _input_guard = input_cancel.clone().drop_guard();
    if let Some(input) = input {
        let mut stdin = group.child.inner().stdin.take().expect("piped stdin");
        let stopped = input_cancel.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = stopped.cancelled() => {},
                _ = stdin.write_all(&input) => {},
            }
            // Closing the pipe signals EOF, including on cancellation/failure.
        });
    }
    let stdout = group.child.inner().stdout.take().expect("piped stdout");
    let stderr = group.child.inner().stderr.take().expect("piped stderr");
    let pipe_failed = CancellationToken::new();
    let failed = pipe_failed.clone();
    let mut out_task = tokio::spawn(async move {
        let result = read_output(stdout, 8 * 1024 * 1024, sink, false).await;
        if result.is_err() {
            failed.cancel();
        }
        result
    });
    let mut err_task = tokio::spawn(read_output(stderr, 64 * 1024, None, true));
    let mut check = tokio::time::interval(Duration::from_secs(1));
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    let status = loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break Err(failure(ErrorCode::Interrupted, "Transfer cancelled.", true)),
            _ = pipe_failed.cancelled() => break Err(failure(ErrorCode::Internal, "Media-tool output exceeded its limit or could not be read.", false)),
            _ = &mut deadline => break Err(failure(ErrorCode::Timeout, "The media tool exceeded its time limit. Retry when the source is available.", true)),
            status = group.child.wait() => break status.map_err(|_| failure(ErrorCode::Internal, "Could not observe the media tool's exit status.", true)),
            _ = check.tick() => {
                if let Ok(free) = fs2::available_space(cwd) {
                    if free < 64 * 1024 * 1024 { break Err(failure(ErrorCode::DiskFull, "The download disk is almost full. Free space, then retry.", true)); }
                }
                if let Ok(entries) = std::fs::read_dir(cwd) {
                    let total: u64 = entries.filter_map(Result::ok).filter_map(|entry| entry.metadata().ok()).map(|metadata| metadata.len()).sum();
                    if total > 12 * 1024 * 1024 * 1024 { break Err(failure(ErrorCode::DiskFull, "The transfer exceeded its working-file limit. Choose a smaller format.", false)); }
                }
            }
        }
    };
    if status.is_err() {
        let _ = group.child.kill().await;
    }
    input_cancel.cancel();
    group.running = false;
    let stdout = tokio::time::timeout(Duration::from_secs(3), &mut out_task).await;
    let stderr = tokio::time::timeout(Duration::from_secs(3), &mut err_task).await;
    if stdout.is_err() {
        out_task.abort();
    }
    if stderr.is_err() {
        err_task.abort();
    }
    let status = status?;
    let stdout = stdout
        .ok()
        .and_then(Result::ok)
        .and_then(Result::ok)
        .ok_or_else(|| {
            failure(
                ErrorCode::Internal,
                "Media-tool output was too large or could not be read.",
                false,
            )
        })?;
    let stderr = stderr
        .ok()
        .and_then(Result::ok)
        .and_then(Result::ok)
        .unwrap_or_default();
    Ok(ProcessOutput {
        stdout,
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        success: status.success(),
    })
}

async fn read_output<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    limit: usize,
    sink: Option<LineSink>,
    tail: bool,
) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut pending = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count = reader.read(&mut chunk).await?;
        if count == 0 {
            break;
        }
        if sink.is_none() && output.len() + count > limit {
            if tail {
                output.drain(..(output.len() + count - limit));
            } else {
                return Err(std::io::Error::other("media output limit"));
            }
        }
        if let Some(sink) = &sink {
            for byte in &chunk[..count] {
                if *byte == b'\n' || *byte == b'\r' {
                    sink(&String::from_utf8_lossy(&pending));
                    pending.clear();
                } else if pending.len() < 32768 {
                    pending.push(*byte);
                }
            }
        } else {
            output.extend_from_slice(&chunk[..count]);
        }
    }
    if let Some(sink) = &sink {
        if !pending.is_empty() {
            sink(&String::from_utf8_lossy(&pending));
        }
    }
    Ok(output)
}

fn has_http_status(text: &str, code: &str) -> bool {
    [
        "http error ",
        "http ",
        "httperror ",
        "status code ",
        "status ",
        "http/1.1 ",
        "http/2 ",
    ]
    .iter()
    .any(|prefix| text.contains(&format!("{prefix}{code}")))
}

pub fn classify(stderr: &str) -> AppError {
    let text = stderr.to_ascii_lowercase();
    let source = if text.contains("[youtube]") {
        "YouTube"
    } else {
        "The source"
    };
    // Warnings can contain an HTTP status missing from yt-dlp's final error.
    // Keep that evidence, but only classify content restrictions from the final
    // error so an unrelated DRM/login warning cannot mislabel a public video.
    let terminal = text
        .lines()
        .rfind(|line| line.trim_start().starts_with("error:"))
        .unwrap_or(&text);
    if text.contains("no space left") || text.contains("disk full") {
        failure(
            ErrorCode::DiskFull,
            "The download disk is full. Free space, then retry.",
            true,
        )
    } else if has_http_status(&text, "429") || text.contains("too many requests") {
        failure(
            ErrorCode::RateLimited,
            &format!("{source} is rate-limiting this connection (HTTP 429). Wait a while before trying again."),
            true,
        )
    } else if [
        "not a bot",
        "not a robot",
        "bot check",
        "captcha",
        "automated requests",
    ]
    .iter()
    .any(|phrase| terminal.contains(phrase))
    {
        failure(ErrorCode::Network, &format!("{source} is asking for a bot check and is currently blocking automated access from this connection. Try again later."), true)
    } else if terminal.contains("private video")
        || terminal.contains("members-only")
        || terminal.contains("members only")
    {
        failure(
            ErrorCode::Unsupported,
            "This video is private or members-only. Choose a publicly accessible video.",
            false,
        )
    } else if terminal.contains("drm") {
        failure(
            ErrorCode::Unsupported,
            "This media is DRM-protected and cannot be downloaded by Downpour.",
            false,
        )
    } else if terminal.contains("sign in")
        || terminal.contains("login")
        || terminal.contains("log in")
        || terminal.contains("age-restricted")
    {
        failure(ErrorCode::Unsupported, "This video requires sign-in or age verification. Choose a publicly accessible video that does not require an account.", false)
    } else if has_http_status(terminal, "403") || terminal.contains("forbidden") {
        failure(ErrorCode::Network, &format!("{source} rejected the request (HTTP 403). Check that the link opens in your browser, then try again later."), true)
    } else if terminal.contains("unsupported url")
        || terminal.contains("requested format")
        || terminal.contains("not a valid url")
    {
        failure(
            ErrorCode::Unsupported,
            "This link or format is not supported. Choose another public source or format.",
            false,
        )
    } else if text.contains("permission denied") {
        failure(
            ErrorCode::Storage,
            "The download folder is not writable. Check its permissions.",
            true,
        )
    } else {
        failure(
            ErrorCode::Network,
            "The source could not be downloaded. Check the link and connection, then retry.",
            true,
        )
    }
}

pub fn args(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

#[cfg(test)]
mod classification_tests {
    use super::*;

    #[test]
    fn rate_limit_warning_takes_precedence_over_final_bot_or_sign_in_error() {
        let error = classify("WARNING: [youtube] video: Unable to download webpage: HTTP Error 429: Too Many Requests\nERROR: [youtube] video: Sign in to confirm you’re not a bot. Use --cookies-from-browser or --cookies for authentication.");
        assert_eq!(error.code, ErrorCode::RateLimited);
        assert!(error.retryable);
        assert!(error.message.contains("YouTube"));
        assert!(error.message.contains("HTTP 429"));
        assert!(!error.message.contains("private"));
        assert!(!error.message.contains("cookies"));
    }

    #[test]
    fn bot_challenges_and_generic_forbidden_responses_are_not_private_video_evidence() {
        for stderr in [
            "ERROR: [youtube] video: Sign in to confirm you're not a bot.",
            "ERROR: [youtube] video: Sign in to confirm you’re not a bot.",
            "ERROR: [youtube] video: Unable to download video data: HTTP Error 403: Forbidden",
        ] {
            let error = classify(stderr);
            assert_eq!(error.code, ErrorCode::Network);
            assert!(error.retryable);
            assert!(!error.message.contains("private"));
        }
    }

    #[test]
    fn confirmed_content_restrictions_remain_non_retryable() {
        for stderr in [
            "ERROR: [youtube] video: Private video. Sign in if you've been granted access.",
            "ERROR: [youtube] video: This video is members-only.",
            "ERROR: [generic] video: This video is DRM protected",
            "ERROR: [youtube] video: Sign in to confirm your age. This video may be inappropriate for some users.",
        ] {
            let error = classify(stderr);
            assert_eq!(error.code, ErrorCode::Unsupported);
            assert!(!error.retryable);
        }
    }

    #[test]
    fn unrelated_warning_does_not_mask_the_terminal_failure() {
        let error = classify("WARNING: [youtube] Some DRM formats require login\nERROR: [youtube] video: Unable to download video data: HTTP Error 403: Forbidden");
        assert_eq!(error.code, ErrorCode::Network);
        assert!(error.message.contains("HTTP 403"));
        let error = classify("ERROR: [generic] HTTP Error 429: Too Many Requests");
        assert!(error.message.starts_with("The source"));
    }

    #[test]
    fn numbers_in_a_video_identifier_are_not_http_statuses() {
        let error = classify("ERROR: [youtube] abc429403de: The read operation timed out");
        assert_eq!(error.code, ErrorCode::Network);
        assert!(!error.message.contains("HTTP"));
    }
}
