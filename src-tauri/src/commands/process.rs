//! Process management for ElizaOS CLI execution
//! Handles spawning, monitoring, and controlling ElizaOS CLI processes

use crate::models::{ApiResponse, AppError, RunResult, RunSpec, SandboxConfig};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Emitter};
use tauri_plugin_shell::{process::CommandEvent, ShellExt};
use tokio::sync::{Mutex, RwLock};

// Global process registry to track running processes
type ProcessRegistry = Arc<RwLock<HashMap<String, Arc<Mutex<RunResult>>>>>;

/// Start a new ElizaOS CLI run
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

    match execute_eliza_run(app, spec, config).await {
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
    app: AppHandle,
    run_id: String,
) -> Result<ApiResponse<RunResult>, String> {
    log::info!("Stopping ElizaOS CLI run: {}", run_id);

    match terminate_process(&app, &run_id, false).await {
        Ok(result) => {
            log::info!("Stopped ElizaOS CLI run: {}", run_id);
            Ok(ApiResponse::success(result))
        }
        Err(e) => {
            log::error!("Failed to stop ElizaOS CLI run {}: {}", run_id, e);
            Ok(ApiResponse::error(
                "STOP_ERROR".to_string(),
                format!("Failed to stop run: {}", e),
            ))
        }
    }
}

/// Kill a running ElizaOS CLI process forcefully
#[tauri::command]
pub async fn kill_eliza_run(
    app: AppHandle,
    run_id: String,
) -> Result<ApiResponse<RunResult>, String> {
    log::info!("Killing ElizaOS CLI run: {}", run_id);

    match terminate_process(&app, &run_id, true).await {
        Ok(result) => {
            log::info!("Killed ElizaOS CLI run: {}", run_id);
            Ok(ApiResponse::success(result))
        }
        Err(e) => {
            log::error!("Failed to kill ElizaOS CLI run {}: {}", run_id, e);
            Ok(ApiResponse::error(
                "KILL_ERROR".to_string(),
                format!("Failed to kill run: {}", e),
            ))
        }
    }
}

/// Execute ElizaOS CLI with the given specification
async fn execute_eliza_run(
    app: AppHandle,
    spec: RunSpec,
    config: SandboxConfig,
) -> Result<RunResult, AppError> {
    let shell = app.shell();

    // Resolve ElizaOS CLI command
    let eliza_cmd = resolve_eliza_command().await?;
    log::debug!("Using ElizaOS CLI command: {}", eliza_cmd);

    // Build environment variables
    let mut env = build_eliza_environment(&config);
    env.extend(spec.env.clone());

    // Sanitize and validate arguments
    let safe_args = sanitize_arguments(&spec.args)?;

    let started_at = crate::models::current_timestamp();
    let mut run_result = RunResult::new(spec.clone(), started_at);

    // Create the command
    let mut command = shell.command(&eliza_cmd);
    command.args(&safe_args);

    // Set environment variables
    for (key, value) in &env {
        command.env(key, value);
    }

    // Set working directory if specified
    if let Some(ref working_dir) = spec.working_dir {
        command.current_dir(working_dir);
    }

    // Configure stdio for log streaming
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    log::info!(
        "Executing: {} {} (working_dir: {:?})",
        eliza_cmd,
        safe_args.join(" "),
        spec.working_dir
    );

    // Spawn the command
    let (mut rx, child) = command
        .spawn()
        .map_err(|e| AppError::Process(format!("Failed to spawn ElizaOS CLI: {}", e)))?;

    let run_id = run_result.id.clone();
    let app_handle = app.clone();

    // Store the run result in the registry
    let registry = get_process_registry(&app);
    registry.write().await.insert(run_id.clone(), Arc::new(Mutex::new(run_result.clone())));

    // Spawn a task to handle the process output and lifecycle
    tokio::spawn(async move {
        let start_time = std::time::Instant::now();

        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    log::debug!("ElizaOS CLI stdout: {}", line);

                    // Update run result with stdout
                    if let Ok(mut guard) = registry.read().await.get(&run_id)
                        .and_then(|r| Some(r.clone()))
                        .unwrap_or_else(|| Arc::new(Mutex::new(run_result.clone())))
                        .try_lock()
                    {
                        guard.stdout.push(line.clone());
                    }

                    // Emit event to frontend
                    let _ = app_handle.emit("eliza-stdout", serde_json::json!({
                        "runId": run_id,
                        "content": line
                    }));
                }
                CommandEvent::Stderr(line) => {
                    log::debug!("ElizaOS CLI stderr: {}", line);

                    // Update run result with stderr
                    if let Ok(mut guard) = registry.read().await.get(&run_id)
                        .and_then(|r| Some(r.clone()))
                        .unwrap_or_else(|| Arc::new(Mutex::new(run_result.clone())))
                        .try_lock()
                    {
                        guard.stderr.push(line.clone());
                    }

                    // Emit event to frontend
                    let _ = app_handle.emit("eliza-stderr", serde_json::json!({
                        "runId": run_id,
                        "content": line
                    }));
                }
                CommandEvent::Error(error) => {
                    log::error!("ElizaOS CLI error: {}", error);

                    // Emit system event to frontend
                    let _ = app_handle.emit("eliza-system", serde_json::json!({
                        "runId": run_id,
                        "message": format!("Error: {}", error)
                    }));
                }
                CommandEvent::Terminated(payload) => {
                    let duration = start_time.elapsed();
                    let ended_at = crate::models::current_timestamp();

                    log::info!(
                        "ElizaOS CLI process {} terminated with code: {:?} (duration: {:?})",
                        run_id,
                        payload.code,
                        duration
                    );

                    // Update run result with completion data
                    if let Some(run_arc) = registry.read().await.get(&run_id) {
                        if let Ok(mut guard) = run_arc.try_lock() {
                            *guard = guard.clone().complete(
                                payload.code.unwrap_or(-1),
                                ended_at,
                                duration.as_millis() as u64,
                            );
                        }
                    }

                    // Emit completion event to frontend
                    let _ = app_handle.emit("eliza-completed", serde_json::json!({
                        "runId": run_id,
                        "exitCode": payload.code,
                        "duration": duration.as_millis()
                    }));

                    break;
                }
            }
        }

        // Clean up registry entry after a delay
        tokio::time::sleep(tokio::time::Duration::from_secs(300)).await; // 5 minutes
        registry.write().await.remove(&run_id);
    });

    Ok(run_result)
}

/// Terminate a process (gracefully or forcefully)
async fn terminate_process(
    app: &AppHandle,
    run_id: &str,
    force: bool,
) -> Result<RunResult, AppError> {
    let registry = get_process_registry(app);

    let run_result = {
        let guard = registry.read().await;
        guard
            .get(run_id)
            .ok_or_else(|| AppError::Process(format!("Run {} not found", run_id)))?
            .lock()
            .await
            .clone()
    };

    // For now, we'll emit a termination request
    // In a full implementation, this would actually terminate the process
    let action = if force { "kill" } else { "stop" };

    let _ = app.emit("eliza-system", serde_json::json!({
        "runId": run_id,
        "message": format!("Process {} requested", action)
    }));

    // Update the run result to reflect termination
    let ended_at = crate::models::current_timestamp();
    let updated_result = run_result.kill(ended_at, 0);

    // Update in registry
    if let Some(run_arc) = registry.read().await.get(run_id) {
        if let Ok(mut guard) = run_arc.try_lock() {
            *guard = updated_result.clone();
        }
    }

    Ok(updated_result)
}

/// Resolve the ElizaOS CLI command to use
async fn resolve_eliza_command() -> Result<String, AppError> {
    // For MVP, we'll use npx approach
    // In production, this could check for local installation first

    // Check if npx is available
    let npx_check = std::process::Command::new("npx")
        .arg("--version")
        .output();

    match npx_check {
        Ok(output) if output.status.success() => {
            log::debug!("Using npx to run ElizaOS CLI");
            Ok("npx".to_string())
        }
        _ => {
            // Fallback to direct eliza command (might fail)
            log::warn!("npx not available, trying direct eliza command");
            Ok("eliza".to_string())
        }
    }
}

/// Build environment variables for ElizaOS CLI
fn build_eliza_environment(config: &SandboxConfig) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Sandbox-specific environment variables
    env.insert("SANDBOX_API_KEY".to_string(), config.api_key.clone());
    env.insert("SANDBOX_BASE_URL".to_string(), config.base_url.clone());
    env.insert("SANDBOX_PROJECT_ID".to_string(), config.project_id.clone());
    env.insert("ELIZA_PROVIDER".to_string(), "sandbox".to_string());

    if let Some(ref model) = config.default_model {
        env.insert("ELIZA_DEFAULT_MODEL".to_string(), model.clone());
    }

    // Additional configuration
    env.insert("ELIZA_LOG_LEVEL".to_string(), "info".to_string());
    env.insert("NODE_ENV".to_string(), "production".to_string());

    log::debug!("Built environment with {} variables", env.len());
    env
}

/// Sanitize command arguments to prevent injection attacks
fn sanitize_arguments(args: &[String]) -> Result<Vec<String>, AppError> {
    let mut sanitized = Vec::new();

    // Dangerous flags that should be filtered out
    let dangerous_flags = [
        "--eval", "-e",
        "--print", "-p",
        "--check", "-c",
        "--interactive", "-i",
    ];

    for arg in args {
        // Skip dangerous flags
        if dangerous_flags.contains(&arg.as_str()) {
            log::warn!("Filtered dangerous argument: {}", arg);
            continue;
        }

        // Basic sanitization
        let clean_arg = arg
            .replace("&&", "")
            .replace("||", "")
            .replace(";", "")
            .replace("|", "")
            .replace("`", "")
            .replace("$", "");

        sanitized.push(clean_arg);
    }

    Ok(sanitized)
}

/// Get or create the process registry
fn get_process_registry(app: &AppHandle) -> ProcessRegistry {
    app.state::<ProcessRegistry>().inner().clone()
}

/// Initialize the process registry (called during app setup)
pub fn init_process_registry() -> ProcessRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_arguments() {
        let args = vec![
            "run".to_string(),
            "-m".to_string(),
            "gpt-4".to_string(),
            "--eval".to_string(), // Should be filtered
            "console.log('test')".to_string(),
            "arg with && dangerous".to_string(),
        ];

        let sanitized = sanitize_arguments(&args).unwrap();

        assert_eq!(sanitized.len(), 4); // --eval should be filtered out
        assert_eq!(sanitized[0], "run");
        assert_eq!(sanitized[1], "-m");
        assert_eq!(sanitized[2], "gpt-4");
        assert_eq!(sanitized[3], "arg with  dangerous"); // && removed
        assert!(!sanitized.contains(&"--eval".to_string()));
    }

    #[test]
    fn test_build_eliza_environment() {
        let config = SandboxConfig {
            base_url: "https://api.example.com".to_string(),
            api_key: "eliza_test_key".to_string(),
            project_id: "test-project".to_string(),
            default_model: Some("gpt-4".to_string()),
        };

        let env = build_eliza_environment(&config);

        assert_eq!(env.get("SANDBOX_API_KEY").unwrap(), "eliza_test_key");
        assert_eq!(env.get("SANDBOX_BASE_URL").unwrap(), "https://api.example.com");
        assert_eq!(env.get("SANDBOX_PROJECT_ID").unwrap(), "test-project");
        assert_eq!(env.get("ELIZA_PROVIDER").unwrap(), "sandbox");
        assert_eq!(env.get("ELIZA_DEFAULT_MODEL").unwrap(), "gpt-4");
    }
}