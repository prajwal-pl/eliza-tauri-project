//! CLI Handler - Processes command line arguments and subcommands
//! Provides headless functionality and CLI-based operations

use tauri_plugin_cli::CliExt;
use crate::commands::config;

pub async fn handle_cli(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    match app.cli().matches() {
        Ok(matches) => {
            log::debug!("CLI matches: {:?}", matches);

            // For now, just handle basic CLI functionality
            // TODO: Add proper CLI argument parsing when API is clearer

            // Handle subcommands if available
            if let Some(subcommand) = &matches.subcommand {
                log::info!("Processing CLI subcommand: {}", subcommand.name);

                match subcommand.name.as_str() {
                    "doctor" => {
                        println!("🏥 ElizaOS Desktop - System Health Check");
                        println!("=======================================");

                        match run_doctor_check(app).await {
                            Ok(_) => println!("🎉 Health check completed!"),
                            Err(e) => {
                                println!("❌ Health check failed: {}", e);
                                std::process::exit(1);
                            }
                        }
                        std::process::exit(0);
                    },
                    "terminal" => {
                        println!("💻 Launching terminal mode...");
                        // Don't exit, allow GUI to launch with terminal focused
                    },
                    _ => {
                        eprintln!("Unknown subcommand: {}", subcommand.name);
                        std::process::exit(1);
                    }
                }
            }

            // If no subcommand, launch GUI normally
            log::info!("CLI processed, launching GUI");
            Ok(())
        }
        Err(e) => {
            eprintln!("Error parsing CLI arguments: {}", e);
            std::process::exit(1);
        }
    }
}

/// Run the doctor health check
async fn run_doctor_check(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Load config and run health checks
    let config_result = config::load_sandbox_config(app.clone()).await;

    match config_result {
        Ok(config_response) => {
            if config_response.success {
                let config = config_response.data.unwrap_or_default();

                println!("📋 Configuration loaded successfully");

                // Run connection test
                match config::test_sandbox_connection(config.clone()).await {
                    Ok(result) => {
                        if result.success {
                            if let Some(connection_data) = result.data {
                                if connection_data.success {
                                    let latency = connection_data.latency_ms.map(|ms| ms.to_string()).unwrap_or_default();
                                    println!("✅ API Connection: HEALTHY ({}ms)", latency);
                                } else {
                                    println!("❌ API Connection: FAILED - {}", connection_data.error.unwrap_or_default());
                                }
                            } else {
                                println!("❌ API Connection: NO DATA");
                            }
                        } else {
                            println!("❌ API Connection: ERROR - {}", result.error.unwrap_or_default().message);
                        }
                    }
                    Err(e) => {
                        println!("❌ API Connection: ERROR - {}", e);
                    }
                }
            } else {
                println!("❌ Configuration: NOT LOADED - {}", config_response.error.unwrap_or_default().message);
            }
        }
        Err(e) => {
            println!("❌ Configuration: ERROR - {}", e);
        }
    }

    // Check ElizaOS CLI availability
    match std::process::Command::new("elizaos").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                println!("✅ ElizaOS CLI: v{}", version.trim());
            } else {
                println!("❌ ElizaOS CLI: COMMAND FAILED");
            }
        }
        Err(_) => {
            println!("❌ ElizaOS CLI: NOT FOUND");
        }
    }

    Ok(())
}