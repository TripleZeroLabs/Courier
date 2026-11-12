fn main() {
    // Declaring the app's commands in the manifest puts them under Tauri's
    // permission system: no window can call them unless a capability grants
    // them. capabilities/settings.json grants both to the "settings" window
    // only; the Facebook window gets nothing.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["get_settings", "save_settings"])),
    )
    .expect("failed to run tauri-build");
}
