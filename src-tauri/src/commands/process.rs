//! Process management for ElizaOS CLI execution
//! Handles spawning, monitoring, and controlling ElizaOS CLI processes

use crate::models::{ApiResponse, AppError, RunResult, RunSpec, RunStatus, RunMode, SandboxConfig};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, RwLock};

// Global process registry to track running processes
type ProcessRegistry = Arc<RwLock<HashMap<String, Arc<Mutex<RunResult>>>>>;

/// Start a new ElizaOS CLI run (simplified version for MVP)
#[tauri::command]
pub async fn start_eliza_run(
    app: AppHandle,
    spec: RunSpec,
    config: SandboxConfig,
) -> Result<ApiResponse<RunResult>, String> {
    log::info!("Starting ElizaOS CLI run: {} {:?}", spec.mode, spec.args);

    if !config.is_valid() {
        return Ok(ApiResponse::error(
            "INVALID_CONFIG".to_string(),
            "Invalid Sandbox configuration".to_string(),
        ));
    }

    match execute_eliza_run_simple(app, spec, config).await {
        Ok(result) => {
            log::info!("Started ElizaOS CLI run: {}", result.id);
            Ok(ApiResponse::success(result))
        }
        Err(e) => {
            log::error!("Failed to start ElizaOS CLI run: {}", e);
            Ok(ApiResponse::error(
                "START_ERROR".to_string(),
                format!("Failed to start run: {}", e),
            ))
        }
    }
}

/// Stop a running ElizaOS CLI process gracefully
#[tauri::command]
pub async fn stop_eliza_run(
    _app: AppHandle,
    run_id: String,
) -> Result<ApiResponse<()>, String> {
    log::info!("Stopping ElizaOS CLI run: {}", run_id);

    // For MVP, we'll implement simple process tracking
    // In production, this would track and terminate actual processes
    Ok(ApiResponse::success(()))
}

/// Kill a running ElizaOS CLI process forcefully
#[tauri::command]
pub async fn kill_eliza_run(
    _app: AppHandle,
    run_id: String,
) -> Result<ApiResponse<()>, String> {
    log::info!("Killing ElizaOS CLI run: {}", run_id);

    // For MVP, we'll implement simple process tracking
    // In production, this would forcefully terminate processes
    Ok(ApiResponse::success(()))
}

/// Execute ElizaOS CLI run with simplified process management
async fn execute_eliza_run_simple(
    _app: AppHandle,
    spec: RunSpec,
    config: SandboxConfig,
) -> Result<RunResult, AppError> {
    // Generate unique run ID
    let run_id = format!("run_{}", crate::models::current_timestamp());

    // Create initial run result
    let mut run_result = RunResult::new(spec.clone(), run_id.clone());

    // Determine ElizaOS CLI command
    let eliza_cmd = resolve_eliza_command().await?;

    log::debug!("Using ElizaOS command: {}", eliza_cmd);

    // Build command arguments based on mode
    let args = build_eliza_args(&spec, &config)?;

    // Sanitize arguments for logging (remove sensitive information)
    let safe_args: Vec<String> = args.iter()
        .map(|arg| {
            if arg.starts_with("eliza_") {
                format!("{}***", &arg[..12])
            } else {
                arg.clone()
            }
        })
        .collect();

    log::info!(
        "Executing: {} {} (working_dir: {:?})",
        eliza_cmd,
        safe_args.join(" "),
        spec.working_dir
    );

    // For MVP, we'll simulate execution and provide test output
    run_result.status = RunStatus::Completed;
    run_result.stdout.push("ElizaOS CLI execution started (simulated for MVP)".to_string());
    run_result.stdout.push(format!("Mode: {}", spec.mode));
    run_result.stdout.push(format!("Args: {:?}", safe_args));
    run_result.ended_at = Some(crate::models::current_timestamp());
    run_result.duration_ms = Some(1000); // Simulate 1 second execution
    run_result.exit_code = Some(0);

    Ok(run_result)
}

/// Resolve the ElizaOS CLI command to use
async fn resolve_eliza_command() -> Result<String, AppError> {
    // Try to find eliza CLI directly first
    if let Ok(output) = Command::new("which").arg("eliza").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                log::debug!("Found eliza CLI at: {}", path);
                return Ok("eliza".to_string());
            }
        }
    }

    // Fall back to npx approach
    log::debug!("ElizaOS CLI not found in PATH, using npx");
    Ok("npx".to_string())
}

/// Build ElizaOS CLI arguments based on run specification
fn build_eliza_args(spec: &RunSpec, _config: &SandboxConfig) -> Result<Vec<String>, AppError> {
    let mut args = Vec::new();

    // If using npx, add the package specification
    if args.is_empty() {
        // This will be set if we're using npx
        args.push("-y".to_string());
        args.push("eliza@latest".to_string());
    }

    // Add the mode command
    match spec.mode {
        RunMode::Doctor => args.push("doctor".to_string()),
        RunMode::Run => args.push("run".to_string()),
        RunMode::Eval => args.push("eval".to_string()),
        RunMode::Custom => args.push("custom".to_string()),
    }

    // Add Sandbox configuration as environment variables
    // Note: In real implementation, these would be set as environment variables
    // For MVP, we'll add them as arguments for demonstration

    // Add any additional arguments from the spec
    args.extend(spec.args.clone());

    Ok(args)
}

/// Build environment variables for ElizaOS CLI execution
fn build_eliza_env(config: &SandboxConfig) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Set Sandbox configuration
    env.insert("ELIZA_SANDBOX_BASE_URL".to_string(), config.base_url.clone());
    env.insert("ELIZA_SANDBOX_API_KEY".to_string(), config.api_key.clone());
    env.insert("ELIZA_SANDBOX_PROJECT_ID".to_string(), config.project_id.clone());

    if let Some(ref model) = config.default_model {
        env.insert("ELIZA_DEFAULT_MODEL".to_string(), model.clone());
    }

    // Set other useful environment variables
    env.insert("NODE_ENV".to_string(), "production".to_string());
    env.insert("ELIZA_DESKTOP".to_string(), "true".to_string());

    env
}

/// Get or create the process registry for the app
pub fn get_process_registry(app: &AppHandle) -> ProcessRegistry {
    app.state::<ProcessRegistry>().inner().clone()
}

/// Initialize the process registry (called from main)
pub fn init_process_registry() -> ProcessRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Get current run result by ID
#[tauri::command]
pub async fn get_run_result(
    app: AppHandle,
    run_id: String,
) -> Result<ApiResponse<RunResult>, String> {
    log::debug!("Getting run result for: {}", run_id);

    let registry = get_process_registry(&app);
    let guard = registry.read().await;

    match guard.get(&run_id) {
        Some(run_mutex) => {
            let run = run_mutex.lock().await.clone();
            Ok(ApiResponse::success(run))
        }
        None => {
            Ok(ApiResponse::error(
                "NOT_FOUND".to_string(),
                format!("Run {} not found", run_id),
            ))
        }
    }
}

/// Sanitize command arguments for logging (remove API keys)
fn sanitize_args_for_logging(args: &[String]) -> Vec<String> {
    args.iter()
        .map(|arg| {
            if arg.starts_with("eliza_") && arg.len() > 20 {
                format!("{}***", &arg[..12])
            } else if arg.contains("api") && arg.len() > 20 {
                "***".to_string()
            } else {
                arg.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_args_for_logging() {
        let args = vec![
            "eliza_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            "normal_arg".to_string(),
            "api_key_1234567890abcdef".to_string(),
        ];

        let sanitized = sanitize_args_for_logging(&args);
        assert_eq!(sanitized[0], "eliza_123456***");
        assert_eq!(sanitized[1], "normal_arg");
        assert_eq!(sanitized[2], "***");
    }

    #[tokio::test]
    async fn test_build_eliza_args() {
        let spec = RunSpec {
            id: "test".to_string(),
            mode: "doctor".to_string(),
            args: vec!["--verbose".to_string()],
            working_dir: None,
        };

        let config = SandboxConfig {
            base_url: "https://api.example.com".to_string(),
            api_key: "eliza_test_key".to_string(),
            project_id: "test-project".to_string(),
            default_model: Some("gpt-4".to_string()),
        };

        let args = build_eliza_args(&spec, &config).unwrap();
        assert!(args.contains(&"doctor".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
    }

    #[test]
    fn test_build_eliza_env() {
        let config = SandboxConfig {
            base_url: "https://api.example.com".to_string(),
            api_key: "eliza_test_key".to_string(),
            project_id: "test-project".to_string(),
            default_model: Some("gpt-4".to_string()),
        };

        let env = build_eliza_env(&config);
        assert_eq!(env.get("ELIZA_SANDBOX_BASE_URL"), Some(&"https://api.example.com".to_string()));
        assert_eq!(env.get("ELIZA_SANDBOX_API_KEY"), Some(&"eliza_test_key".to_string()));
        assert_eq!(env.get("ELIZA_SANDBOX_PROJECT_ID"), Some(&"test-project".to_string()));
        assert_eq!(env.get("ELIZA_DEFAULT_MODEL"), Some(&"gpt-4".to_string()));
    }
}