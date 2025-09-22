//! MVP Tauri ElizaOS CLI - Main application entry point
//! Desktop client for running ElizaOS CLI with Sandbox integration

pub mod commands;
pub mod models;

use commands::*;
use commands::process::get_run_result;
use log::info;

/// Basic greet command for IPC testing
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! ElizaOS Desktop is running.", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_secs()
        .init();

    info!("Starting MVP Tauri ElizaOS CLI v{}", env!("CARGO_PKG_VERSION"));

    // Initialize process registry
    let process_registry = init_process_registry();

    tauri::Builder::default()
        // Initialize plugins
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())

        // Register global state
        .manage(process_registry)

        // Register command handlers
        .invoke_handler(tauri::generate_handler![
            // Basic IPC commands
            greet,

            // Configuration commands
            save_sandbox_config,
            load_sandbox_config,
            clear_sandbox_config,
            test_sandbox_connection,

            // Preflight commands
            preflight_check,

            // Process management commands
            start_eliza_run,
            start_eliza_run_streaming,
            stop_eliza_run,
            kill_eliza_run,
            get_run_result,

            // Telemetry commands
            post_telemetry,
            get_device_id,
        ])

        // Set up window configuration
        .setup(|app| {
            info!("Application setup complete");

            // Log system information
            info!("System: {} {}", std::env::consts::OS, std::env::consts::ARCH);

            Ok(())
        })

        // Run the application
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
