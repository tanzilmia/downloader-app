use crate::domain::*;
use async_trait::async_trait;

#[async_trait]
pub trait Extractor: Send + Sync {
    async fn inspect(&self, url: &str) -> Result<Media, AppError>;
}

pub struct MockExtractor;

#[async_trait]
impl Extractor for MockExtractor {
    async fn inspect(&self, input: &str) -> Result<Media, AppError> {
        let url = validate_url(input)?;
        let platform = detect_platform(&url);
        let (title, creator, duration) = match platform {
            Platform::Youtube => ("A little further from the everyday", "The Open Road", 247),
            Platform::Instagram => ("Small moments, worth keeping", "Everyday stories", 42),
            Platform::Tiktok => ("A moment of inspiration", "The creative corner", 31),
            Platform::Facebook => ("Stories from a slower life", "Field Notes", 128),
            Platform::Other => (
                "Something worth coming back to",
                "An open corner of the web",
                180,
            ),
        };
        Ok(Media {
            url: url.to_string(),
            title: title.into(),
            creator: creator.into(),
            platform,
            duration_seconds: duration,
            formats: vec![
                MediaFormat {
                    id: "mock-1080p".into(),
                    label: "Full HD".into(),
                    extension: "mp4".into(),
                    quality: "1080p".into(),
                    estimated_bytes: 86_000_000,
                    audio_only: false,
                },
                MediaFormat {
                    id: "mock-720p".into(),
                    label: "HD".into(),
                    extension: "mp4".into(),
                    quality: "720p".into(),
                    estimated_bytes: 42_000_000,
                    audio_only: false,
                },
                MediaFormat {
                    id: "mock-audio".into(),
                    label: "Audio only".into(),
                    extension: "m4a".into(),
                    quality: "128 kbps".into(),
                    estimated_bytes: 4_000_000,
                    audio_only: true,
                },
            ],
            is_mock: true,
        })
    }
}
