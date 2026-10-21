use download_core::{AppError, DownloadManager, Job, Media, Settings, Snapshot};
use serde::Deserialize;
use std::sync::Arc;
use tauri::State;

type Manager<'a> = State<'a, Arc<DownloadManager>>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectInput {
    url: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnqueueInput {
    url: String,
    format_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobInput {
    id: String,
}

#[tauri::command]
pub async fn get_snapshot(manager: Manager<'_>) -> Result<Snapshot, AppError> {
    manager.snapshot().await
}
#[tauri::command]
pub async fn inspect_media(manager: Manager<'_>, input: InspectInput) -> Result<Media, AppError> {
    manager.inspect(&input.url).await
}
#[tauri::command]
pub async fn enqueue_job(manager: Manager<'_>, input: EnqueueInput) -> Result<Job, AppError> {
    manager.enqueue(&input.url, &input.format_id).await
}
#[tauri::command]
pub async fn cancel_job(manager: Manager<'_>, input: JobInput) -> Result<Job, AppError> {
    manager.cancel(&input.id).await
}
#[tauri::command]
pub async fn retry_job(manager: Manager<'_>, input: JobInput) -> Result<Job, AppError> {
    manager.retry(&input.id).await
}
#[tauri::command]
pub async fn update_settings(manager: Manager<'_>, input: Settings) -> Result<Settings, AppError> {
    manager.update_settings(input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use download_core::{engine::MockEngine, extractor::MockExtractor, Repository};
    use serde_json::{json, Value};
    use tauri::{
        ipc::{CallbackFn, InvokeBody},
        test::{get_ipc_response, mock_builder, INVOKE_KEY},
        webview::InvokeRequest,
        Manager as _,
    };

    #[test]
    fn ipc_validates_payloads_and_restricts_commands_to_the_local_main_window() {
        let directory = tempfile::tempdir().unwrap();
        let manager = tauri::async_runtime::block_on(async {
            let repository = Repository::open(&directory.path().join("ipc.db"))
                .await
                .unwrap();
            DownloadManager::new(
                repository,
                Arc::new(MockExtractor),
                Arc::new(MockEngine::default()),
                Arc::new(|_| {}),
            )
            .await
            .unwrap()
        });
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                get_snapshot,
                inspect_media,
                enqueue_job,
                cancel_job,
                retry_job,
                update_settings
            ])
            .build(crate::app_context())
            .unwrap();
        app.manage(manager);
        let main = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let request = |cmd: &str, body: Value, origin: &str| InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: origin.parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        };
        let local = if cfg!(windows) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        };
        let response = get_ipc_response(&main, request("get_snapshot", json!({}), local))
            .unwrap()
            .deserialize::<Snapshot>()
            .unwrap();
        assert_eq!(response.persistence, "sqlite");
        assert!(get_ipc_response(
            &main,
            request(
                "inspect_media",
                json!({"input":{"url":"file:///etc/passwd"}}),
                local
            )
        )
        .is_err());
        assert!(get_ipc_response(
            &main,
            request(
                "update_settings",
                json!({"input":{"concurrency":99,"theme":"dark"}}),
                local
            )
        )
        .is_err());
        let queued = get_ipc_response(
            &main,
            request(
                "enqueue_job",
                json!({"input":{"url":"https://youtu.be/sample", "formatId":"mock-1080p"}}),
                local,
            ),
        )
        .unwrap()
        .deserialize::<Job>()
        .unwrap();
        let cancelled = get_ipc_response(
            &main,
            request("cancel_job", json!({"input":{"id":queued.id}}), local),
        )
        .unwrap()
        .deserialize::<Job>()
        .unwrap();
        assert_eq!(cancelled.status, download_core::JobStatus::Cancelled);
        let retry = get_ipc_response(
            &main,
            request("retry_job", json!({"input":{"id":queued.id}}), local),
        )
        .unwrap()
        .deserialize::<Job>()
        .unwrap();
        assert_eq!(retry.status, download_core::JobStatus::Queued);
        assert!(get_ipc_response(
            &main,
            request("get_snapshot", json!({}), "https://untrusted.example.org")
        )
        .is_err());
        let untrusted = tauri::WebviewWindowBuilder::new(&app, "untrusted", Default::default())
            .build()
            .unwrap();
        assert!(get_ipc_response(&untrusted, request("get_snapshot", json!({}), local)).is_err());
    }
}
