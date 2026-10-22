use download_core::{AppError, Job, Media, Settings, Snapshot};
use serde::Deserialize;
use std::sync::Arc;
use tauri::State;

type Services<'a> = State<'a, Arc<crate::services::Services>>;

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
pub async fn get_snapshot(services: Services<'_>) -> Result<Snapshot, AppError> {
    let runtime = services.get().await?;
    if let Some(engine) = &runtime.engine {
        // Retry a missing/repaired manifest on reconnect without restarting the
        // manager or losing history. Dependency errors remain visible in info.
        let _ = engine.check_tools().await;
    }
    runtime.manager.snapshot().await
}
#[tauri::command]
pub async fn inspect_media(services: Services<'_>, input: InspectInput) -> Result<Media, AppError> {
    services.get().await?.manager.inspect(&input.url).await
}
#[tauri::command]
pub async fn enqueue_job(services: Services<'_>, input: EnqueueInput) -> Result<Job, AppError> {
    services
        .get()
        .await?
        .manager
        .enqueue(&input.url, &input.format_id)
        .await
}
#[tauri::command]
pub async fn cancel_job(services: Services<'_>, input: JobInput) -> Result<Job, AppError> {
    services.get().await?.manager.cancel(&input.id).await
}
#[tauri::command]
pub async fn retry_job(services: Services<'_>, input: JobInput) -> Result<Job, AppError> {
    services.get().await?.manager.retry(&input.id).await
}
#[tauri::command]
pub async fn update_settings(
    services: Services<'_>,
    input: Settings,
) -> Result<Settings, AppError> {
    services.get().await?.manager.update_settings(input).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChooseDirectoryInput {
    reset: bool,
}

#[tauri::command]
pub async fn choose_download_directory<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    services: Services<'_>,
    input: ChooseDirectoryInput,
) -> Result<Option<Snapshot>, AppError> {
    use media_engine::files::FileStore;
    use tauri_plugin_dialog::DialogExt;
    let _selection = services
        .destination_change
        .try_lock()
        .map_err(|_| AppError::validation("A folder selection is already open."))?;
    let runtime = services.get().await?;
    let engine = runtime.engine.as_ref().ok_or_else(|| {
        AppError::validation("Download folders are available with the desktop media engine.")
    })?;
    let directory = if input.reset {
        FileStore::check_destination(engine.files().root()).await?;
        None
    } else {
        let snapshot = runtime.manager.snapshot().await?;
        let mut picker = window
            .dialog()
            .file()
            .set_title("Choose download folder")
            .set_parent(&window);
        if let Some(directory) = snapshot.engine_info.output_directory {
            picker = picker.set_directory(directory);
        }
        let (sender, receiver) = tokio::sync::oneshot::channel();
        picker.pick_folder(move |folder| {
            let _ = sender.send(folder);
        });
        let folder = receiver.await.map_err(|_| {
            AppError::validation("The folder picker closed unexpectedly. Try again.")
        })?;
        let Some(folder) = folder else {
            return Ok(None);
        };
        let folder = folder
            .into_path()
            .map_err(|_| AppError::validation("Choose a local download folder."))?;
        let folder = FileStore::check_destination(&folder).await?;
        Some(
            folder
                .to_str()
                .ok_or_else(|| {
                    AppError::validation("The folder name cannot be used. Choose another folder.")
                })?
                .to_owned(),
        )
    };
    runtime.manager.set_download_directory(directory).await?;
    runtime.manager.snapshot().await.map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenInput {
    id: String,
    reveal: bool,
}

#[tauri::command]
pub async fn open_output(
    app: tauri::AppHandle,
    services: Services<'_>,
    input: OpenInput,
) -> Result<(), AppError> {
    use tauri_plugin_opener::OpenerExt;
    let runtime = services.get().await?;
    let job = runtime.manager.job(&input.id).await?;
    if job.status != download_core::JobStatus::Completed || job.is_mock {
        return Err(AppError::validation(
            "Only completed real downloads have saved media files.",
        ));
    }
    let engine = runtime
        .engine
        .as_ref()
        .ok_or_else(|| AppError::validation("No real engine is available."))?;
    let path = engine
        .files_for(&job)
        .await?
        .validate_output(&job, true)
        .await?;
    if input.reveal {
        app.opener()
            .reveal_item_in_dir(path)
            .map_err(AppError::storage)
    } else {
        app.opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(AppError::storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use download_core::{
        engine::MockEngine, extractor::MockExtractor, DownloadManager, Repository,
    };
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
                update_settings,
                choose_download_directory
            ])
            .build(crate::app_context())
            .unwrap();
        app.manage(crate::services::Services::from_manager(manager));
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
        for body in [
            json!({"input":{"reset":false,"path":"/arbitrary"}}),
            json!({"input":{"reset":true}}),
        ] {
            assert!(
                get_ipc_response(&main, request("choose_download_directory", body, local)).is_err()
            );
        }
        assert!(get_ipc_response(
            &main,
            request(
                "update_settings",
                json!({"input":{"concurrency":2,"theme":"dark","downloadDirectory":"/arbitrary"}}),
                local
            )
        )
        .is_err());
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
