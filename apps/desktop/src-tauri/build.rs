fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_snapshot",
            "inspect_media",
            "enqueue_job",
            "cancel_job",
            "retry_job",
            "update_settings",
            "open_output",
            "choose_download_directory",
        ]),
    ))
    .expect("could not build Tauri application manifest");
}
