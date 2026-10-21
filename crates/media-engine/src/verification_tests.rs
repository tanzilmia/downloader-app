//! Deterministic security/reliability tests; no public-network access.
use crate::{
    files::FileStore,
    metadata,
    process::{args, run},
    tools::{sha256, verify, ToolSpec},
};
use download_core::{AppError, ErrorCode, Job};
use std::{path::Path, time::Duration};
use tokio_util::sync::CancellationToken;

#[test]
fn progress_parses_real_speed_without_scaling_it_across_streams() {
    let sample = crate::ytdlp::parse_progress(
        r#"DOWNPOUR:{"percent":" 50.0%","speed":1250000.25}"#,
        44.0,
        44.0,
    )
    .unwrap();
    assert_eq!(sample.percent, 66.0);
    assert_eq!(sample.speed_bytes_per_second, Some(1250000.25));
    let unknown =
        crate::ytdlp::parse_progress(r#"DOWNPOUR:{"percent":"NA","speed":512000}"#, 0.0, 88.0)
            .unwrap();
    assert_eq!(unknown.percent, 0.0);
    assert_eq!(unknown.speed_bytes_per_second, Some(512000.0));
    for speed in ["null", "\"NA\"", "-1"] {
        let line = format!(r#"DOWNPOUR:{{"percent":"5%","speed":{speed}}}"#);
        assert!(crate::ytdlp::parse_progress(&line, 0.0, 88.0)
            .unwrap()
            .speed_bytes_per_second
            .is_none());
    }
    assert!(crate::ytdlp::parse_progress("[download] something else", 0.0, 88.0).is_none());
    assert!(crate::ytdlp::parse_progress("DOWNPOUR:{broken", 0.0, 88.0).is_none());
}

async fn fixture_manifest(root: &Path) -> std::path::PathBuf {
    use crate::tools::{host_target, ToolManifest};
    let mut tools = std::collections::BTreeMap::new();
    for name in ["yt-dlp", "ffmpeg", "ffprobe", "deno"] {
        let path = root.join(name);
        tokio::fs::write(&path, b"fixture bytes; never execute during startup")
            .await
            .unwrap();
        let (_, checksum) = sha256(&path, &CancellationToken::new()).await.unwrap();
        tools.insert(
            name.to_string(),
            ToolSpec {
                path,
                sha256: checksum,
                version: "fixture".into(),
            },
        );
    }
    let manifest = root.join("engine.json");
    tokio::fs::write(
        &manifest,
        serde_json::to_vec(&ToolManifest {
            schema_version: 1,
            target: host_target().into(),
            distribution: "development".into(),
            tools,
        })
        .unwrap(),
    )
    .await
    .unwrap();
    manifest
}

#[cfg(unix)]
async fn scripted_backend(root: &Path, response: &str, exit: u8) -> crate::ytdlp::YtdlpBackend {
    use std::os::unix::fs::PermissionsExt;
    let manifest = fixture_manifest(root).await;
    let executable = root.join("yt-dlp");
    let body = format!(
        "#!/bin/sh\nprintf x >> calls.txt\nprintf '%s\\n' '{}' {}\nexit {exit}\n",
        response.replace('\'', "'\\''"),
        if exit == 0 { "" } else { ">&2" }
    );
    tokio::fs::write(&executable, body).await.unwrap();
    tokio::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
        .await
        .unwrap();
    let (_, checksum) = sha256(&executable, &CancellationToken::new())
        .await
        .unwrap();
    let mut specification: crate::tools::ToolManifest =
        serde_json::from_slice(&tokio::fs::read(&manifest).await.unwrap()).unwrap();
    specification.tools.get_mut("yt-dlp").unwrap().sha256 = checksum;
    tokio::fs::write(&manifest, serde_json::to_vec(&specification).unwrap())
        .await
        .unwrap();
    crate::ytdlp::YtdlpBackend::new(&manifest, &root.join("downloads"))
        .await
        .unwrap()
}

#[test]
fn transfer_info_contains_only_public_downloader_fields_and_resolved_fragments() {
    let source = "https://www.youtube.com/watch?v=W9-MV7HWb3g";
    let raw = serde_json::json!({
        "id":"source-id", "title":"Public fixture", "webpage_url":source, "_filename":"unsafe/path", "cookies":"private", "requested_downloads":[{"url":"file:///private"}],
        "http_headers":{"User-Agent":"Fixture agent", "Cookie":"private", "Authorization":"private"},
        "formats":[{"format_id":"mp4", "url":"https://media.blender.org/video.mp4", "protocol":"http_dash_segments", "ext":"mp4", "vcodec":"h264", "acodec":"aac", "height":480,
            "fragment_base_url":"https://media.blender.org/parts/", "fragments":[{"path":"one.m4s", "duration":1.5, "filename":"unsafe/path"}],
            "http_headers":{"Accept":"video/*", "Proxy-Authorization":"private", "Origin":"https://www.youtube.com", "Referer":"bad\r\nHeader:value"}, "downloader_options":{"external_downloader":"bad"}
        }]
    });
    let bytes = serde_json::to_vec(&raw).unwrap();
    let media = metadata::parse(&bytes, source).unwrap();
    let info = metadata::download_info(&bytes, &media).unwrap();
    let text = String::from_utf8(info.clone()).unwrap();
    assert!(!text.contains("private"));
    assert!(!text.contains("unsafe"));
    assert!(!text.contains("webpage_url"));
    assert!(!text.contains("downloader_options"));
    let info: serde_json::Value = serde_json::from_slice(&info).unwrap();
    assert_eq!(
        info["formats"][0]["fragments"][0]["url"],
        "https://media.blender.org/parts/one.m4s"
    );
    assert_eq!(
        info["formats"][0]["http_headers"]["User-Agent"],
        "Fixture agent"
    );
    let mut invalid = raw;
    invalid["formats"][0]["fragments"][0]["url"] = serde_json::json!("file:///private");
    assert!(metadata::download_info(&serde_json::to_vec(&invalid).unwrap(), &media).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn preview_metadata_is_piped_to_both_transfers_without_another_page_lookup() {
    use download_core::{engine::TransferEngine, extractor::Extractor};
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let manifest = fixture_manifest(root.path()).await;
    let script = r#"#!/usr/bin/python3
import json,pathlib,sys
args=sys.argv[1:]
calls=pathlib.Path(__file__).with_name('transfer-calls.txt')
if '--dump-single-json' in args:
    with calls.open('a') as out: out.write('inspect\n')
    print(json.dumps({'id':'fixture','title':'Paired fixture','webpage_url':'https://www.youtube.com/watch?v=W9-MV7HWb3g','formats':[
        {'format_id':'video','url':'https://media.blender.org/video.mp4','ext':'mp4','protocol':'https','vcodec':'h264','acodec':'none','height':1080},
        {'format_id':'audio','url':'https://media.blender.org/audio.m4a','ext':'m4a','protocol':'https','vcodec':'none','acodec':'aac','abr':128}]}))
elif '--load-info-json' in args:
    assert args[args.index('--load-info-json')+1]=='-'
    assert not any('youtube.com/' in arg for arg in args)
    info=json.load(sys.stdin)
    assert 'webpage_url' not in info and len(info['formats'])==2
    with calls.open('a') as out: out.write('transfer\n')
    pathlib.Path(args[args.index('--output')+1]).write_bytes(b'controlled fixture')
else:
    raise AssertionError('unexpected extra extraction')
"#;
    let executable = root.path().join("yt-dlp");
    tokio::fs::write(&executable, script).await.unwrap();
    tokio::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
        .await
        .unwrap();
    let (_, checksum) = sha256(&executable, &CancellationToken::new())
        .await
        .unwrap();
    let mut specification: crate::tools::ToolManifest =
        serde_json::from_slice(&tokio::fs::read(&manifest).await.unwrap()).unwrap();
    specification.tools.get_mut("yt-dlp").unwrap().sha256 = checksum;
    tokio::fs::write(&manifest, serde_json::to_vec(&specification).unwrap())
        .await
        .unwrap();
    let backend = crate::ytdlp::YtdlpBackend::new(&manifest, &root.path().join("downloads"))
        .await
        .unwrap();
    let media = backend
        .inspect("https://download.blender.org/fixture.mp4")
        .await
        .unwrap();
    let mut job = job();
    job.format = media.formats[0].clone();
    job.media = media;
    let mut stale = job.clone();
    stale.format.id = "no-longer-available".into();
    let (progress, _receiver) = tokio::sync::mpsc::channel(8);
    assert_eq!(
        backend
            .transfer(stale, progress, CancellationToken::new())
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    let (progress, _receiver) = tokio::sync::mpsc::channel(8);
    // Inert FFmpeg fixture stops here after both stream downloads. This test
    // makes no claim about producing real media or network access.
    assert_eq!(
        backend
            .transfer(job, progress, CancellationToken::new())
            .await
            .unwrap_err()
            .code,
        ErrorCode::Dependency
    );
    assert_eq!(
        tokio::fs::read_to_string(root.path().join("transfer-calls.txt"))
            .await
            .unwrap(),
        "inspect\ntransfer\ntransfer\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn piped_input_is_bounded_and_does_not_block_output_draining_or_deadlines() {
    use crate::process::run_with_input;
    let root = tempfile::tempdir().unwrap();
    let input: std::sync::Arc<[u8]> = vec![b'x'; 128 * 1024].into();
    let output = run_with_input(Path::new("/usr/bin/python3"), &args(&["-c", "import sys;sys.stdout.write('o'*100000);sys.stdout.flush();data=sys.stdin.buffer.read();sys.stdout.write(str(len(data)))"]), root.path(), Duration::from_secs(5), CancellationToken::new(), None, Some(input)).await.unwrap();
    assert!(output.success);
    assert!(output.stdout.ends_with(b"131072"));
    let input: std::sync::Arc<[u8]> = vec![0; 8 * 1024 * 1024].into();
    let error = run_with_input(
        Path::new("/usr/bin/python3"),
        &args(&["-c", "import time;time.sleep(30)"]),
        root.path(),
        Duration::from_millis(100),
        CancellationToken::new(),
        None,
        Some(input),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.code, ErrorCode::Timeout);
    let input: std::sync::Arc<[u8]> = vec![0; 8 * 1024 * 1024 + 1].into();
    assert_eq!(
        run_with_input(
            Path::new("/not/executed"),
            &[],
            root.path(),
            Duration::from_secs(1),
            CancellationToken::new(),
            None,
            Some(input)
        )
        .await
        .err()
        .unwrap()
        .code,
        ErrorCode::Validation
    );
}

#[cfg(unix)]
#[tokio::test]
async fn repeated_rate_limited_lookups_do_not_spawn_another_process() {
    use download_core::extractor::Extractor;
    let root = tempfile::tempdir().unwrap();
    let backend = scripted_backend(
        root.path(),
        "ERROR: [youtube] fixture: HTTP Error 429: Too Many Requests",
        1,
    )
    .await;
    for source in [
        "https://youtube.com/shorts/D_8JUUup5js",
        "https://youtu.be/D_8JUUup5js?si=share",
        "https://youtube.com/watch?v=BaW_jenozKc",
    ] {
        let error = backend.inspect(source).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::RateLimited);
        assert!(error
            .retry_after_seconds
            .is_some_and(|seconds| seconds <= 60 && seconds > 0));
    }
    assert_eq!(
        tokio::fs::read(root.path().join("downloads/calls.txt"))
            .await
            .unwrap(),
        b"x"
    );
    let mut transfer = job();
    transfer.media.url = "https://youtube.com/watch?v=D_8JUUup5js".into();
    transfer.format.id = "mp4".into();
    let (progress, _receiver) = tokio::sync::mpsc::channel(1);
    let error = download_core::engine::TransferEngine::transfer(
        &backend,
        transfer,
        progress,
        CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::RateLimited);
    assert_eq!(
        tokio::fs::read(root.path().join("downloads/calls.txt"))
            .await
            .unwrap(),
        b"x"
    );
    // Other sources remain available and actually reach the fixture tool.
    assert!(backend
        .inspect("https://download.blender.org/video.mp4")
        .await
        .unwrap_err()
        .retry_after_seconds
        .is_none());
    assert_eq!(
        tokio::fs::read(root.path().join("downloads/calls.txt"))
            .await
            .unwrap(),
        b"xx"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn concurrent_youtube_aliases_reuse_one_verified_inspection() {
    use download_core::extractor::Extractor;
    let root = tempfile::tempdir().unwrap();
    let backend = scripted_backend(root.path(), r#"{"title":"Fixture","formats":[{"format_id":"mp4","ext":"mp4","protocol":"https","url":"https://download.blender.org/video.mp4","vcodec":"h264","acodec":"aac","height":480}]}"#, 0).await;
    let (short, shared) = tokio::join!(
        backend.inspect("https://youtube.com/shorts/D_8JUUup5js"),
        backend.inspect("https://youtu.be/D_8JUUup5js?si=share")
    );
    let short = short.unwrap();
    assert_eq!(short, shared.unwrap());
    assert_eq!(short.url, "https://www.youtube.com/watch?v=D_8JUUup5js");
    assert_eq!(
        tokio::fs::read(root.path().join("downloads/calls.txt"))
            .await
            .unwrap(),
        b"x"
    );
}

#[tokio::test]
async fn startup_does_not_execute_tools_and_inspection_still_checks_integrity() {
    use crate::ytdlp::YtdlpBackend;
    use download_core::{engine::TransferEngine, extractor::Extractor};
    let directory = tempfile::tempdir().unwrap();
    let manifest = fixture_manifest(directory.path()).await;
    let backend = YtdlpBackend::new(&manifest, &directory.path().join("downloads"))
        .await
        .unwrap();
    assert!(
        backend.info().available,
        "startup must verify hashes without running version processes"
    );
    tokio::fs::write(directory.path().join("yt-dlp"), b"changed after startup")
        .await
        .unwrap();
    let error = backend
        .inspect("https://youtu.be/sample")
        .await
        .unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::Dependency,
        "changed tools must never execute"
    );
}

#[tokio::test]
async fn repaired_tools_can_reconnect_without_recreating_the_backend() {
    use crate::ytdlp::YtdlpBackend;
    use download_core::engine::TransferEngine;
    let directory = tempfile::tempdir().unwrap();
    let manifest = directory.path().join("engine.json");
    let backend = YtdlpBackend::new(&manifest, &directory.path().join("downloads"))
        .await
        .unwrap();
    assert!(!backend.info().available);
    fixture_manifest(directory.path()).await;
    backend.check_tools().await.unwrap();
    assert!(backend.info().available);
}

fn job() -> Job {
    let mut job: Job = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/job.json"
    ))
    .unwrap();
    job.is_mock = false;
    job.media.is_mock = false;
    job.output = None;
    job
}

#[tokio::test]
async fn finalization_has_receipt_recovery_and_never_overwrites() {
    let temporary = tempfile::tempdir().unwrap();
    let store = FileStore::new(temporary.path()).await.unwrap();
    let mut job = job();
    let directory = store.prepare(&job).await.unwrap();
    let staged = directory.join("verified.mp4");
    tokio::fs::write(&staged, b"verified fixture content")
        .await
        .unwrap();
    let output = store
        .publish(&job, &staged, 1.5, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        tokio::fs::read(&output.path).await.unwrap(),
        b"verified fixture content"
    );
    let recovered = FileStore::new(temporary.path())
        .await
        .unwrap()
        .recover(&job, &CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered, output);
    assert!(store
        .publish(&job, &staged, 1.5, &CancellationToken::new())
        .await
        .is_err());
    job.output = Some(output);
    store.committed(&job).await.unwrap();
    assert!(!directory.exists());
    assert!(store.validate_output(&job, true).await.is_ok());
}

#[tokio::test]
async fn a_crash_before_publication_recovers_from_the_durable_receipt() {
    let temporary = tempfile::tempdir().unwrap();
    let store = FileStore::new(temporary.path()).await.unwrap();
    let job = job();
    let directory = store.prepare(&job).await.unwrap();
    let staged = directory.join("verified.mp4");
    tokio::fs::write(&staged, b"same-volume media bytes")
        .await
        .unwrap();
    let output = store
        .publish(&job, &staged, 2.0, &CancellationToken::new())
        .await
        .unwrap();
    tokio::fs::remove_file(&output.path).await.unwrap();
    assert_eq!(
        store
            .recover(&job, &CancellationToken::new())
            .await
            .unwrap()
            .unwrap(),
        output
    );
}

#[tokio::test]
async fn rejects_changed_files_bad_receipts_traversal_and_empty_outputs() {
    let temporary = tempfile::tempdir().unwrap();
    let store = FileStore::new(temporary.path()).await.unwrap();
    let mut job = job();
    job.media.title = "../../CON:movie".into();
    assert_eq!(
        store.final_path(&job).unwrap().parent().unwrap(),
        temporary.path().canonicalize().unwrap()
    );
    let directory = store.prepare(&job).await.unwrap();
    let staged = directory.join("verified.mp4");
    tokio::fs::write(&staged, []).await.unwrap();
    assert!(store
        .publish(&job, &staged, 1.0, &CancellationToken::new())
        .await
        .is_err());
    tokio::fs::write(&staged, b"one").await.unwrap();
    let output = store
        .publish(&job, &staged, 1.0, &CancellationToken::new())
        .await
        .unwrap();
    job.output = Some(output.clone());
    tokio::fs::write(&output.path, b"two").await.unwrap();
    assert!(store.validate_output(&job, true).await.is_err());
    assert!(
        store.discard(&job, &output).await.is_err(),
        "never delete a changed file"
    );
    job.id = "../../escape".into();
    assert!(store.prepare(&job).await.is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn symbolic_links_are_not_accepted_as_working_or_output_files() {
    use std::os::unix::fs::PermissionsExt;
    let temporary = tempfile::tempdir().unwrap();
    let store = FileStore::new(temporary.path()).await.unwrap();
    let job = job();
    let directory = store.prepare(&job).await.unwrap();
    assert_eq!(
        std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    std::os::unix::fs::symlink("/etc/passwd", directory.join("stream-0.download")).unwrap();
    assert!(store.prepare(&job).await.is_err());
}

#[tokio::test]
async fn tool_checksum_change_disables_execution() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("tool");
    tokio::fs::write(&path, b"trusted bytes").await.unwrap();
    let (_, checksum) = sha256(&path, &CancellationToken::new()).await.unwrap();
    let tool = ToolSpec {
        path: path.clone(),
        sha256: checksum,
        version: "test".into(),
    };
    verify(&tool).await.unwrap();
    tokio::fs::write(&path, b"modified bytes").await.unwrap();
    assert_eq!(verify(&tool).await.unwrap_err().code, ErrorCode::Dependency);
}

#[test]
fn metadata_filters_drm_non_http_and_unsafe_selectors_and_builds_stream_pairs() {
    let metadata = serde_json::json!({"title":"<script>title</script>","duration":12.5,"formats":[
        {"format_id":"137","ext":"mp4","protocol":"https","url":"https://cdn.example.org/video","vcodec":"avc1","acodec":"none","height":1080,"filesize":1000},
        {"format_id":"140","ext":"m4a","protocol":"https","url":"https://cdn.example.org/audio","vcodec":"none","acodec":"aac","abr":128,"filesize":100},
        {"format_id":"evil","ext":"mp4","protocol":"rtmp","url":"rtmp://evil.org/file"},
        {"format_id":"drm","ext":"mp4","protocol":"https","url":"https://cdn.example.org/drm","has_drm":true},
        {"format_id":"unsafe[exec]","ext":"mp4","protocol":"https","url":"https://cdn.example.org/a"}
    ]});
    let media = metadata::parse(
        &serde_json::to_vec(&metadata).unwrap(),
        "https://youtube.com/watch?v=one",
    )
    .unwrap();
    assert_eq!(media.formats.len(), 2);
    assert_eq!(media.formats[0].id, "137+140");
    assert_eq!(media.formats[0].estimated_bytes, 1100);
    assert!(!media.is_mock);
    for value in ["--exec", "137;touch x", "137[protocol=rtmp]", "a+b+c", ""] {
        assert!(metadata::format_parts(value).is_err());
    }
    assert!(metadata::parse(br#"{"is_live":true}"#, "https://youtube.com/watch?v=one").is_err());
}

#[test]
fn audio_selection_keeps_m4a_when_webm_has_a_higher_bitrate() {
    let raw = serde_json::json!({"formats":[
        {"format_id":"18","ext":"mp4","protocol":"https","url":"https://cdn.example.org/video.mp4","vcodec":"avc1","acodec":"mp4a.40.2","height":360},
        {"format_id":"139","ext":"m4a","protocol":"https","url":"https://cdn.example.org/low.m4a","vcodec":"none","acodec":"mp4a.40.2","abr":48},
        {"format_id":"140","ext":"m4a","protocol":"https","url":"https://cdn.example.org/high.m4a","vcodec":"none","acodec":"mp4a.40.2","abr":128},
        {"format_id":"251","ext":"webm","protocol":"https","url":"https://cdn.example.org/audio.webm","vcodec":"none","acodec":"opus","abr":160},
        {"format_id":"drm","ext":"m4a","protocol":"https","url":"https://cdn.example.org/drm.m4a","vcodec":"none","acodec":"aac","abr":320,"has_drm":true}
    ]});
    let media = metadata::parse(
        &serde_json::to_vec(&raw).unwrap(),
        "https://example.org/video",
    )
    .unwrap();
    let audio: Vec<_> = media.formats.iter().filter(|f| f.audio_only).collect();
    assert_eq!(audio.len(), 2);
    assert_eq!(audio[0].id, "140");
    assert_eq!(audio[0].extension, "m4a");
    assert_eq!(audio[1].id, "251");
    assert_eq!(audio[1].extension, "webm");
    assert!(metadata::download_info(&serde_json::to_vec(&raw).unwrap(), &media).is_ok());
}

#[test]
fn combined_mp4_offers_a_distinct_audio_selection_using_the_inspected_source() {
    let raw = serde_json::json!({"formats":[
        {"format_id":"big","ext":"mp4","protocol":"https","url":"https://cdn.example.org/big.mp4","vcodec":"avc1","acodec":"aac","height":720,"filesize":2000},
        {"format_id":"small","ext":"mp4","protocol":"https","url":"https://cdn.example.org/small.mp4","vcodec":"avc1","acodec":"mp4a.40.2","height":360,"filesize":1000}
    ]});
    let bytes = serde_json::to_vec(&raw).unwrap();
    let media = metadata::parse(&bytes, "https://example.org/video").unwrap();
    let audio = media.formats.iter().find(|f| f.audio_only).unwrap();
    assert_eq!(audio.id, "audio:small");
    assert_eq!(audio.extension, "m4a");
    assert_eq!(audio.estimated_bytes, 1000);
    assert_eq!(metadata::format_parts(&audio.id).unwrap(), vec!["small"]);
    let info: serde_json::Value =
        serde_json::from_slice(&metadata::download_info(&bytes, &media).unwrap()).unwrap();
    let sources = info["formats"].as_array().unwrap();
    assert_eq!(sources.len(), 2); // The audio choice reuses one source, without a synthetic yt-dlp selector.
    assert!(sources.iter().any(|f| f["format_id"] == "small"));
    assert!(!sources.iter().any(|f| f["format_id"] == "audio:small"));
}

#[test]
fn audio_extraction_does_not_offer_known_silent_or_incompatible_video_tracks() {
    for codec in ["none", "opus", "vorbis"] {
        let raw = serde_json::json!({"formats":[
            {"format_id":"mp4","ext":"mp4","protocol":"https","url":"https://cdn.example.org/video.mp4","vcodec":"avc1","acodec":codec}
        ]});
        let media = metadata::parse(
            &serde_json::to_vec(&raw).unwrap(),
            "https://example.org/video",
        );
        if codec == "none" {
            assert_eq!(media.unwrap_err().code, ErrorCode::Unsupported);
            continue;
        }
        let media = media.unwrap();
        assert!(
            !media.formats.iter().any(|f| f.audio_only),
            "unexpected M4A for {codec}"
        );
    }
    for id in [
        "audio:",
        "audio:a+b",
        "audio:--exec",
        "audio:audio:18",
        "audio:18;touch x",
    ] {
        assert!(
            metadata::format_parts(id).is_err(),
            "accepted unsafe selector {id}"
        );
    }
}

#[test]
fn direct_media_preserves_audio_codecs_and_allows_verified_extraction_when_unknown() {
    let direct_audio = br#"{"format_id":"direct","ext":"m4a","protocol":"https","url":"https://cdn.example.org/audio.m4a","vcodec":"none","acodec":"aac","abr":128}"#;
    let media = metadata::parse(direct_audio, "https://example.org/audio").unwrap();
    assert_eq!(media.formats.len(), 1);
    assert!(media.formats[0].audio_only);
    assert_eq!(media.formats[0].id, "direct");
    assert!(metadata::download_info(direct_audio, &media).is_ok());
    let unknown_audio = br#"{"format_id":"direct","ext":"m4a","protocol":"https","url":"https://cdn.example.org/audio.m4a"}"#;
    let media = metadata::parse(unknown_audio, "https://example.org/audio").unwrap();
    assert_eq!(media.formats.len(), 1);
    assert!(media.formats[0].audio_only);
    assert!(metadata::download_info(unknown_audio, &media).is_ok());
    let direct_video = br#"{"format_id":"mp4","ext":"mp4","protocol":"https","url":"https://cdn.example.org/video.mp4"}"#;
    let media = metadata::parse(direct_video, "https://example.org/video").unwrap();
    assert!(media
        .formats
        .iter()
        .any(|f| f.id == "audio:mp4" && f.audio_only));
    assert!(metadata::download_info(direct_video, &media).is_ok());
}

#[cfg(unix)]
#[tokio::test]
async fn process_deadline_output_limit_and_literal_arguments_are_enforced() -> Result<(), AppError>
{
    let directory = tempfile::tempdir().unwrap();
    let python = Path::new("/usr/bin/python3");
    let output = run(
        python,
        &args(&[
            "-c",
            "import json,sys;print(json.dumps(sys.argv[1:]))",
            ";touch injected",
            "$(echo bad)",
        ]),
        directory.path(),
        Duration::from_secs(3),
        CancellationToken::new(),
        None,
    )
    .await?;
    assert!(output.success);
    assert!(!directory.path().join("injected").exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains(";touch injected"));
    let error = run(
        python,
        &args(&["-c", "import time;time.sleep(30)"]),
        directory.path(),
        Duration::from_millis(80),
        CancellationToken::new(),
        None,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.code, ErrorCode::Timeout);
    let error = run(
        python,
        &args(&[
            "-c",
            "import sys,time;sys.stdout.write('x'*10000000);sys.stdout.flush();time.sleep(30)",
        ]),
        directory.path(),
        Duration::from_secs(5),
        CancellationToken::new(),
        None,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.code, ErrorCode::Internal);
    let output = run(
        python,
        &args(&["-c", "import sys;sys.stderr.write('x'*100000)"]),
        directory.path(),
        Duration::from_secs(3),
        CancellationToken::new(),
        None,
    )
    .await?;
    assert!(output.success);
    assert_eq!(output.stderr.len(), 65536);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_kills_descendants_before_releasing_the_worker() {
    let directory = tempfile::tempdir().unwrap();
    let token = CancellationToken::new();
    let cancel = token.clone();
    let path = directory.path().to_path_buf();
    let worker = tokio::spawn(async move {
        run(Path::new("/usr/bin/python3"), &args(&["-c", "import subprocess,sys,time;subprocess.Popen([sys.executable,'-c',\"import time;time.sleep(0.6);open('late-child','w').write('alive')\"]);open('ready','w').write('ready');time.sleep(30)"]), &path, Duration::from_secs(5), cancel, None).await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !directory.path().join("ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    token.cancel();
    assert_eq!(
        worker.await.unwrap().err().unwrap().code,
        ErrorCode::Interrupted
    );
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(!directory.path().join("late-child").exists());
}
