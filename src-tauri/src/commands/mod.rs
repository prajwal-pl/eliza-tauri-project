//! Command modules for Tauri IPC
//! Exports all command functions for the Tauri application

pub mod config;
pub mod preflight;
pub mod process;
pub mod telemetry;

// Re-export all command functions for easy access
pub use config::{
    clear_sandbox_config, load_sandbox_config, save_sandbox_config, test_sandbox_connection,
};
pub use preflight::preflight_check;
pub use process::{kill_eliza_run, start_eliza_run, start_eliza_run_streaming, stop_eliza_run};
pub use telemetry::{get_device_id, post_telemetry};

// Process registry initialization
pub use process::init_process_registry;