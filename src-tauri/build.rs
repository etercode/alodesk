fn main() {
    // Listing the app's own commands makes Tauri generate a permission for
    // each (allow-set-ptt-key), so the remote site gets only what
    // capabilities/remote.json grants.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["set_ptt_key"])),
    )
    .expect("failed to run tauri-build");
}
