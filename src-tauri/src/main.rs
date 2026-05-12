// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // When built without the `gui` feature (the Linux server build), there is
    // no Tauri runtime to dispatch to - run the headless backend directly.
    #[cfg(not(feature = "gui"))]
    portsage_lib::run_headless();

    #[cfg(feature = "gui")]
    {
        let args: Vec<String> = std::env::args().collect();
        if portsage_lib::is_headless_argv(&args) {
            portsage_lib::run_headless();
        } else {
            portsage_lib::run();
        }
    }
}
