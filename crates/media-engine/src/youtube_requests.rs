//! A local request pause respects upstream limits; it does not remove a block.
use crate::process::failure;
use download_core::{detect_platform, AppError, ErrorCode, Platform};
use std::time::Duration;
use tokio::{
    sync::{Mutex, MutexGuard},
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use url::Url;

const REQUEST_GAP: Duration = Duration::from_secs(5);
const FIRST_PAUSE: u32 = 60;
const MAX_PAUSE: u32 = 900;

#[derive(Default)]
struct State {
    finished: Option<Instant>,
    blocked_until: Option<Instant>,
    failures: u32,
}
impl State {
    fn paused(&self, now: Instant) -> Option<AppError> {
        let remaining = self.blocked_until?.checked_duration_since(now)?;
        if remaining.is_zero() {
            return None;
        }
        let seconds = remaining.as_secs_f64().ceil().min(f64::from(MAX_PAUSE)) as u32;
        let mut error = failure(ErrorCode::RateLimited, "YouTube is rate-limiting this connection (HTTP 429). Downpour has paused YouTube requests to avoid repeated retries. YouTube may still block access after this pause.", true);
        error.retry_after_seconds = Some(seconds);
        Some(error)
    }
    fn failed(&mut self, error: AppError, now: Instant) -> AppError {
        if error.code != ErrorCode::RateLimited {
            return error;
        }
        let seconds = FIRST_PAUSE
            .saturating_mul(2_u32.pow(self.failures.min(4)))
            .min(MAX_PAUSE);
        self.failures = self.failures.saturating_add(1);
        self.blocked_until = Some(now + Duration::from_secs(u64::from(seconds)));
        self.paused(now).expect("newly paused")
    }
    fn succeeded(&mut self) {
        self.failures = 0;
        self.blocked_until = None;
    }
}

#[derive(Default)]
pub(crate) struct YoutubeRequests(Mutex<State>);
pub(crate) struct Request<'a>(MutexGuard<'a, State>);
impl YoutubeRequests {
    pub(crate) async fn acquire(
        &self,
        youtube: bool,
        cancel: &CancellationToken,
    ) -> Result<Option<Request<'_>>, AppError> {
        if !youtube {
            return Ok(None);
        }
        let mut state = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(failure(ErrorCode::Interrupted, "Request cancelled.", true)),
            state = self.0.lock() => state,
        };
        if let Some(error) = state.paused(Instant::now()) {
            return Err(error);
        }
        if let Some(finished) = state.finished {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(failure(ErrorCode::Interrupted, "Request cancelled.", true)),
                _ = tokio::time::sleep_until(finished + REQUEST_GAP) => {},
            }
        }
        // Keep expired deadlines out of subsequent successful requests.
        state.blocked_until = None;
        Ok(Some(Request(state)))
    }
}
impl Request<'_> {
    pub(crate) fn failed(&mut self, error: AppError) -> AppError {
        self.0.failed(error, Instant::now())
    }
    pub(crate) fn succeeded(&mut self) {
        self.0.succeeded();
    }
}
impl Drop for Request<'_> {
    fn drop(&mut self) {
        self.0.finished = Some(Instant::now());
    }
}

/// The app downloads entire videos, so tracking/timestamp query variants share
/// one verified metadata cache entry. Unrecognized paths remain untouched.
pub(crate) fn canonical_url(url: Url) -> Url {
    if detect_platform(&url) != Platform::Youtube {
        return url;
    }
    let segments: Vec<_> = url.path_segments().into_iter().flatten().collect();
    let id = if url
        .host_str()
        .is_some_and(|host| host == "youtu.be" || host.ends_with(".youtu.be"))
    {
        segments.first().map(|id| id.to_string())
    } else if segments.first() == Some(&"watch") {
        url.query_pairs()
            .find(|(key, _)| key == "v")
            .map(|(_, id)| id.into_owned())
    } else if matches!(segments.first(), Some(&"shorts" | &"embed" | &"live")) {
        segments.get(1).map(|id| id.to_string())
    } else {
        None
    };
    match id {
        Some(id)
            if id.len() == 11
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') =>
        {
            Url::parse(&format!("https://www.youtube.com/watch?v={id}"))
                .expect("validated video ID")
        }
        _ => url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_share_a_key_and_unrelated_urls_are_unchanged() {
        let expected = "https://www.youtube.com/watch?v=D_8JUUup5js";
        for source in [
            "https://youtu.be/D_8JUUup5js?si=one",
            "https://youtube.com/shorts/D_8JUUup5js?si=two",
            "https://m.youtube.com/watch?v=D_8JUUup5js&t=5",
            "https://youtube.com/embed/D_8JUUup5js",
        ] {
            assert_eq!(
                canonical_url(Url::parse(source).unwrap()).as_str(),
                expected
            );
        }
        for source in [
            "https://youtube.com/@creator",
            "https://youtube.com/watch?v=invalid",
            "https://youtube.com.evil.org/shorts/D_8JUUup5js",
        ] {
            assert_eq!(canonical_url(Url::parse(source).unwrap()).as_str(), source);
        }
    }
    #[test]
    fn local_pause_expires_without_extending_on_repeated_checks_and_grows_on_rejection() {
        let mut state = State::default();
        let now = Instant::now();
        let error = || failure(ErrorCode::RateLimited, "HTTP 429", true);
        assert_eq!(state.failed(error(), now).retry_after_seconds, Some(60));
        assert_eq!(
            state
                .paused(now + Duration::from_millis(500))
                .unwrap()
                .retry_after_seconds,
            Some(60)
        );
        assert_eq!(
            state
                .paused(now + Duration::from_secs(30))
                .unwrap()
                .retry_after_seconds,
            Some(30)
        );
        assert!(state.paused(now + Duration::from_secs(60)).is_none());
        assert_eq!(
            state
                .failed(error(), now + Duration::from_secs(61))
                .retry_after_seconds,
            Some(120)
        );
        for _ in 0..10 {
            state.failed(error(), now);
        }
        assert_eq!(state.paused(now).unwrap().retry_after_seconds, Some(900));
        state.succeeded();
        assert!(state.paused(now).is_none());
        assert_eq!(state.failed(error(), now).retry_after_seconds, Some(60));
    }
    #[tokio::test]
    async fn cooldown_blocks_only_youtube_and_cancellation_interrupts_waiters() {
        let gate = YoutubeRequests::default();
        let cancel = CancellationToken::new();
        let mut request = gate.acquire(true, &cancel).await.unwrap().unwrap();
        request.failed(failure(ErrorCode::RateLimited, "HTTP 429", true));
        drop(request);
        assert_eq!(
            gate.acquire(true, &cancel)
                .await
                .err()
                .unwrap()
                .retry_after_seconds,
            Some(60)
        );
        assert!(gate.acquire(false, &cancel).await.unwrap().is_none());
        gate.0.lock().await.blocked_until = None;
        let waiting = gate.acquire(true, &cancel);
        tokio::pin!(waiting);
        tokio::select! {
            _ = &mut waiting => panic!("request gap must defer the operation"),
            _ = tokio::time::sleep(Duration::from_millis(10)) => {},
        }
        cancel.cancel();
        assert_eq!(waiting.await.err().unwrap().code, ErrorCode::Interrupted);
    }
}
