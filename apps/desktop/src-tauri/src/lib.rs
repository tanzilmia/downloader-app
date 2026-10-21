mod commands;

use download_core::{engine::MockEngine, extractor::MockExtractor, DownloadManager, Repository};
use std::sync::Arc;
use tauri::{Emitter, Manager};

// Generate the macOS embedded plist once; share this context with IPC tests.
pub(crate) fn app_context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_focus();
        }
    }));
    builder
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let handle = app.handle().clone();
            let manager = tauri::async_runtime::block_on(async {
                let repository = Repository::open(&directory.join("downpour.db")).await?;
                DownloadManager::new(
                    repository,
                    Arc::new(MockExtractor),
                    Arc::new(MockEngine::default()),
                    Arc::new(move |job| {
                        // Events target this application's local window only.
                        if let Err(error) = handle.emit_to("main", "job:updated", job) {
                            eprintln!("Downpour event: {error}");
                        }
                    }),
                )
                .await
            })?;
            // Tokio scheduling runs on Tauri's async runtime, never the webview thread.
            let runtime_manager = Arc::clone(&manager);
            tauri::async_runtime::spawn(async move {
                let _scheduler = runtime_manager.start();
            });
            app.manage(manager);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::inspect_media,
            commands::enqueue_job,
            commands::cancel_job,
            commands::retry_job,
            commands::update_settings
        ])
        .run(app_context())
        .expect(
            "Downpour could not start. Check app-data permissions and the startup error above.",
        );
}
