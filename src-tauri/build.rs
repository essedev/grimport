fn main() {
    // The `gui` feature pulls in `tauri-build`. Without it (Linux server
    // build) we have nothing platform-specific to do.
    #[cfg(feature = "gui")]
    tauri_build::build();
}
