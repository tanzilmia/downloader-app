use crate::process::failure;
use download_core::{detect_platform, validate_url, AppError, ErrorCode, Media, MediaFormat};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Metadata {
    pub title: Option<String>,
    pub uploader: Option<String>,
    pub channel: Option<String>,
    pub duration: Option<f64>,
    #[serde(default)]
    pub formats: Vec<Format>,
    pub format_id: Option<String>,
    pub ext: Option<String>,
    pub protocol: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "_type")]
    pub kind: Option<String>,
    pub is_live: Option<bool>,
    pub has_drm: Option<bool>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub abr: Option<f64>,
}
#[derive(Clone, Deserialize)]
pub struct Format {
    pub format_id: String,
    pub ext: Option<String>,
    pub protocol: Option<String>,
    pub url: Option<String>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub height: Option<f64>,
    pub abr: Option<f64>,
    pub filesize: Option<u64>,
    pub filesize_approx: Option<u64>,
    pub has_drm: Option<bool>,
}
fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 40
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        && !value.starts_with('-')
}
fn allowed(format: &Format) -> bool {
    safe_id(&format.format_id)
        && format.has_drm != Some(true)
        && matches!(format.ext.as_deref(), Some("mp4" | "webm" | "m4a"))
        && matches!(
            format.protocol.as_deref(),
            Some("http" | "https" | "http_dash_segments" | "m3u8_native" | "m3u8")
        )
        && format
            .url
            .as_ref()
            .is_some_and(|url| validate_url(url).is_ok())
}
fn size(format: &Format) -> u64 {
    format.filesize.or(format.filesize_approx).unwrap_or(0)
}
fn audio_stream(format: &Format) -> bool {
    (format.vcodec.as_deref() == Some("none")
        || (format.vcodec.is_none() && format.ext.as_deref() == Some("m4a")))
        && format.acodec.as_deref() != Some("none")
}
fn clean(value: Option<String>, fallback: &str) -> String {
    let value: String = value
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(300)
        .collect();
    if value.trim().is_empty() {
        fallback.into()
    } else {
        value
    }
}

/// Only downloader inputs survive the preview boundary. Never retain page
/// metadata, credentials, output paths, or a webpage fallback that re-extracts.
pub(crate) fn download_info(bytes: &[u8], media: &Media) -> Result<Vec<u8>, AppError> {
    use serde_json::{json, Map, Value};
    let invalid = || {
        failure(
            ErrorCode::Unsupported,
            "The source returned unusable stream details. Inspect the link again.",
            true,
        )
    };
    let root: Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let mut needed = std::collections::HashSet::new();
    for format in &media.formats {
        needed.extend(format_parts(&format.id)?);
    }
    let candidates = root
        .get("formats")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let candidates = if candidates.is_empty() {
        vec![root.clone()]
    } else {
        candidates
    };
    let mut formats = Vec::new();
    for value in candidates {
        let id = value
            .get("format_id")
            .and_then(Value::as_str)
            .unwrap_or("direct");
        if !needed.contains(id) {
            continue;
        }
        let mut candidate = value.clone();
        candidate["format_id"] = json!(id);
        let parsed: Format = serde_json::from_value(candidate).map_err(|_| invalid())?;
        if !allowed(&parsed) {
            return Err(invalid());
        }
        let mut input = Map::new();
        for key in [
            "format_id",
            "url",
            "ext",
            "protocol",
            "vcodec",
            "acodec",
            "width",
            "height",
            "fps",
            "abr",
            "asr",
            "audio_channels",
            "filesize",
            "filesize_approx",
            "container",
        ] {
            if let Some(value) = value.get(key) {
                input.insert(key.into(), value.clone());
            }
        }
        input.insert("format_id".into(), json!(id));
        let url = validate_url(
            input
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?,
        )?;
        for key in ["manifest_url", "fragment_base_url"] {
            if let Some(address) = value.get(key).and_then(Value::as_str) {
                input.insert(key.into(), json!(validate_url(address)?.as_str()));
            }
        }
        if let Some(fragments) = value.get("fragments").and_then(Value::as_array) {
            let base = value
                .get("fragment_base_url")
                .and_then(Value::as_str)
                .map(validate_url)
                .transpose()?
                .unwrap_or(url);
            let fragments = fragments
                .iter()
                .map(|fragment| {
                    let address = fragment
                        .get("url")
                        .or_else(|| fragment.get("path"))
                        .and_then(Value::as_str)
                        .ok_or_else(invalid)?;
                    let address = base.join(address).map_err(|_| invalid())?;
                    let address = validate_url(address.as_str())?;
                    let mut fields = Map::new();
                    fields.insert("url".into(), json!(address.as_str()));
                    for key in ["duration", "range", "index"] {
                        if let Some(value) = fragment.get(key) {
                            fields.insert(key.into(), value.clone());
                        }
                    }
                    Ok(Value::Object(fields))
                })
                .collect::<Result<Vec<_>, AppError>>()?;
            input.insert("fragments".into(), json!(fragments));
        }
        let mut headers = Map::new();
        for source in [&root, &value] {
            if let Some(values) = source.get("http_headers").and_then(Value::as_object) {
                for (key, value) in values {
                    if [
                        "user-agent",
                        "referer",
                        "origin",
                        "accept",
                        "accept-language",
                    ]
                    .contains(&key.to_ascii_lowercase().as_str())
                    {
                        if let Some(text) = value
                            .as_str()
                            .filter(|text| text.len() <= 4096 && !text.contains(['\r', '\n']))
                        {
                            headers.insert(key.clone(), json!(text));
                        }
                    }
                }
            }
        }
        input.insert("http_headers".into(), Value::Object(headers));
        formats.push(Value::Object(input));
    }
    if formats.len() != needed.len() {
        return Err(invalid());
    }
    let info = serde_json::to_vec(
        &json!({"id":"downpour", "title":"Public media", "_type":"video", "formats":formats}),
    )
    .map_err(|_| invalid())?;
    if info.len() > 8 * 1024 * 1024 {
        return Err(invalid());
    }
    Ok(info)
}

pub fn parse(bytes: &[u8], source: &str) -> Result<Media, AppError> {
    let url = validate_url(source)?;
    let metadata: Metadata = serde_json::from_slice(bytes).map_err(|_| {
        failure(
            ErrorCode::Unsupported,
            "The source did not return usable media metadata.",
            false,
        )
    })?;
    if metadata.is_live == Some(true)
        || metadata.has_drm == Some(true)
        || metadata
            .kind
            .as_ref()
            .is_some_and(|kind| !["video"].contains(&kind.as_str()))
    {
        return Err(failure(
            ErrorCode::Unsupported,
            "Live streams, playlists and DRM-protected media are not supported.",
            false,
        ));
    }
    let mut available: Vec<_> = metadata.formats.into_iter().filter(allowed).collect();
    if available.is_empty() {
        let direct = Format {
            format_id: metadata.format_id.unwrap_or_else(|| "direct".into()),
            ext: metadata.ext,
            protocol: metadata.protocol,
            url: metadata.url,
            vcodec: metadata.vcodec,
            acodec: metadata.acodec,
            height: None,
            abr: metadata.abr,
            filesize: None,
            filesize_approx: None,
            has_drm: metadata.has_drm,
        };
        if allowed(&direct) {
            available.push(direct);
        }
    }
    available.sort_by(|a, b| b.height.unwrap_or(0.0).total_cmp(&a.height.unwrap_or(0.0)));
    let mut formats = Vec::new();
    let mut qualities = std::collections::HashSet::new();
    for video in &available {
        if video.vcodec.as_deref() == Some("none") {
            continue;
        }
        let extension = video.ext.as_deref().unwrap_or("mp4");
        if !matches!(extension, "mp4" | "webm") {
            continue;
        }
        let height = video
            .height
            .filter(|h| h.is_finite() && *h > 0.0)
            .unwrap_or(0.0) as u64;
        let quality = if height == 0 {
            "Original".to_string()
        } else {
            format!("{height}p")
        };
        if qualities.contains(&(quality.clone(), extension.to_string())) {
            continue;
        }
        let mut id = video.format_id.clone();
        let mut estimated = size(video);
        if video.acodec.as_deref() == Some("none") {
            let audio_ext = if extension == "mp4" { "m4a" } else { "webm" };
            let audio = available
                .iter()
                .filter(|a| audio_stream(a) && a.ext.as_deref() == Some(audio_ext))
                .max_by(|a, b| a.abr.unwrap_or(0.0).total_cmp(&b.abr.unwrap_or(0.0)));
            let Some(audio) = audio else {
                continue;
            };
            id.push('+');
            id.push_str(&audio.format_id);
            estimated = if estimated == 0 || size(audio) == 0 {
                0
            } else {
                estimated.saturating_add(size(audio))
            };
        }
        formats.push(MediaFormat {
            id,
            label: if height >= 1080 {
                "High definition".into()
            } else {
                "Video".into()
            },
            extension: extension.into(),
            quality: quality.clone(),
            estimated_bytes: estimated,
            audio_only: false,
        });
        qualities.insert((quality, extension.to_string()));
        if formats.len() >= 8 {
            break;
        }
    }
    // Keep the best audio for each container. A higher-bitrate WebM stream must
    // not displace M4A when the desktop picker hides WebM.
    for extension in ["m4a", "webm"] {
        if let Some(audio) = available
            .iter()
            .filter(|a| audio_stream(a) && a.ext.as_deref() == Some(extension))
            .max_by(|a, b| a.abr.unwrap_or(0.0).total_cmp(&b.abr.unwrap_or(0.0)))
        {
            formats.push(MediaFormat {
                id: audio.format_id.clone(),
                label: "Audio only".into(),
                extension: extension.into(),
                quality: audio
                    .abr
                    .filter(|a| a.is_finite() && *a > 0.0)
                    .map(|a| format!("{} kbps", a as u64))
                    .unwrap_or_else(|| "Original audio".into()),
                estimated_bytes: size(audio),
                audio_only: true,
            });
        }
    }
    if !formats.iter().any(|f| f.audio_only && f.extension == "m4a") {
        // Some sources expose only a combined MP4. Copy its audio track into
        // M4A using the existing local-only processing path. Missing codec
        // metadata is common for direct links; FFmpeg/FFprobe still verify the
        // actual track before publication. Known incompatible codecs are omitted.
        if let Some(source) = available
            .iter()
            .filter(|f| {
                f.ext.as_deref() == Some("mp4")
                    && f.vcodec.as_deref() != Some("none")
                    && f.acodec.as_deref().is_none_or(|codec| {
                        codec == "aac" || codec == "alac" || codec.starts_with("mp4a.")
                    })
            })
            .min_by_key(|f| {
                let bytes = size(f);
                (
                    if bytes == 0 { u64::MAX } else { bytes },
                    f.height
                        .filter(|h| h.is_finite() && *h > 0.0)
                        .unwrap_or(f64::MAX) as u64,
                )
            })
        {
            formats.push(MediaFormat {
                id: format!("audio:{}", source.format_id),
                label: "Audio only".into(),
                extension: "m4a".into(),
                quality: "Original audio".into(),
                estimated_bytes: size(source),
                audio_only: true,
            });
        }
    }
    if formats.is_empty() {
        return Err(failure(
            ErrorCode::Unsupported,
            "No supported public HTTP media formats were found.",
            false,
        ));
    }
    Ok(Media {
        url: url.to_string(),
        title: clean(metadata.title, "Untitled video"),
        creator: clean(metadata.uploader.or(metadata.channel), "Public media"),
        platform: detect_platform(&url),
        duration_seconds: metadata
            .duration
            .filter(|d| d.is_finite() && *d >= 0.0)
            .unwrap_or(0.0)
            .round()
            .min(31_536_000.0) as u64,
        formats,
        is_mock: false,
    })
}
pub fn format_parts(value: &str) -> Result<Vec<&str>, AppError> {
    // The reserved prefix identifies audio extraction from one inspected MP4;
    // only the validated source selector reaches yt-dlp or cached stream data.
    let audio = value.strip_prefix("audio:");
    let parts: Vec<_> = audio.unwrap_or(value).split('+').collect();
    if parts.len() > 2 || (audio.is_some() && parts.len() != 1) || parts.iter().any(|p| !safe_id(p))
    {
        return Err(failure(
            ErrorCode::Validation,
            "The saved format selection is invalid. Inspect the source again.",
            false,
        ));
    }
    Ok(parts)
}
