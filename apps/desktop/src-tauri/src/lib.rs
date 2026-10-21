mod commands;
mod services;

use std::sync::Arc;
use tauri::{Emitter, Manager};

// Generate the macOS embedded plist once; share this context with IPC tests.
pub(crate) fn app_context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(all(feature = "native-e2e", not(debug_assertions)))]
    compile_error!("The native test driver must never be enabled in release builds.");
    #[cfg(feature = "native-e2e")]
    let builder = builder.plugin(tauri_plugin_webdriver::init());
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_focus();
        }
    }));
    let app = builder
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            #[cfg(feature = "native-e2e")]
            let directory = std::env::var_os("DOWNPOUR_TEST_DATA")
                .map(std::path::PathBuf::from)
                .unwrap_or(directory);
            std::fs::create_dir_all(&directory)?;
            let handle = app.handle().clone();
            let health_handle = app.handle().clone();
            let packaged_manifest = app
                .path()
                .resource_dir()?
                .join("engines")
                .join(media_engine::tools::host_target())
                .join("engine.json");
            let manifest = std::env::var_os("DOWNPOUR_ENGINE_MANIFEST")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    if packaged_manifest.is_file() {
                        packaged_manifest
                    } else {
                        directory.join("engine.json")
                    }
                });
            let output = app.path().download_dir()?.join("Downpour");
            #[cfg(feature = "native-e2e")]
            let output = std::env::var_os("DOWNPOUR_TEST_OUTPUT")
                .map(std::path::PathBuf::from)
                .unwrap_or(output);
            let services = services::Services::new(
                manifest,
                output,
                directory.join("downpour.db"),
                Arc::new(move |job| {
                    // Events target this application's local window only.
                    if let Err(error) = handle.emit_to("main", "job:updated", job) {
                        eprintln!("Downpour event: {error}");
                    }
                }),
                Arc::new(move |error| {
                    let _ = health_handle.emit_to("main", "engine:health", error);
                }),
            );
            app.manage(Arc::clone(&services));
            tauri::async_runtime::spawn(async move {
                if let Err(error) = services.get().await {
                    eprintln!("Downpour startup: {}", error.message);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::inspect_media,
            commands::enqueue_job,
            commands::cancel_job,
            commands::retry_job,
            commands::update_settings,
            commands::open_output
        ])
        .build(app_context())
        .expect(
            "Downpour could not start. Check app-data permissions and the startup error above.",
        );
    let shutting_down = std::sync::atomic::AtomicBool::new(false);
    app.run(move |handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if !shutting_down.swap(true, std::sync::atomic::Ordering::SeqCst) {
                api.prevent_exit();
                let handle = handle.clone();
                let services = handle.state::<Arc<services::Services>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    services.stop().await;
                    handle.exit(0);
                });
            }
        }
    });
}
