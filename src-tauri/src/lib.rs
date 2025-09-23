//! MVP Tauri ElizaOS CLI - Main application entry point
//! Desktop client for running ElizaOS CLI with Sandbox integration

pub mod commands;
pub mod models;
pub mod cli_handler;

use commands::process::get_run_result;
use commands::*;
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
        .filter_level(log::LevelFilter::Debug) // Enable debug logging
        .format_timestamp_secs()
        .init();

    info!(
        "Starting MVP Tauri ElizaOS CLI v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Initialize process registry
    let process_registry = init_process_registry();

    // Initialize terminal registry
    let terminal_registry = init_terminal_registry();

    tauri::Builder::default()
        .plugin(tauri_plugin_cli::init())
        // Initialize plugins
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        // Register global state
        .manage(process_registry)
        .manage(terminal_registry)
        // Register command handlers
        .invoke_handler(tauri::generate_handler![
            // Basic IPC commands
            greet,
            // Configuration commands
            save_sandbox_config,
            load_sandbox_config,
            clear_sandbox_config,
            test_sandbox_connection,
            test_api_prompt,
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
            // Terminal commands
            initialize_terminal,
            execute_terminal_command,
            cancel_terminal_command,
            get_terminal_processes,
            get_terminal_cwd,
            change_terminal_cwd,
            cleanup_terminal_processes,
        ])
        // Set up window configuration
        .setup(|app| {
            info!("Application setup complete");

            // Log system information
            info!(
                "System: {} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            );

            // Handle CLI arguments
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = cli_handler::handle_cli(&app_handle).await {
                    log::error!("CLI handler error: {}", e);
                }
            });

            Ok(())
        })
        // Run the application
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
