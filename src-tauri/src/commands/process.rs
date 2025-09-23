//! Process management for ElizaOS CLI execution
//! Handles spawning, monitoring, and controlling ElizaOS CLI processes

use crate::models::{
    ApiResponse, AppError, LogEvent, RunMode, RunResult, RunSpec, RunStatus, SandboxConfig,
};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::{Mutex, RwLock};

// Structure to track running processes
#[derive(Debug, Clone)]
pub struct ProcessHandle {
    pub run_result: RunResult,
    pub can_control: bool, // Whether the process can be controlled
}

impl ProcessHandle {
    pub fn new(run_result: RunResult) -> Self {
        Self {
            run_result,
            can_control: true,
        }
    }

    pub fn update_result(&mut self, new_result: RunResult) {
        self.run_result = new_result;
    }

    pub fn mark_completed(&mut self) {
        self.can_control = false;
    }
}

// Global process registry to track running processes
type ProcessRegistry = Arc<RwLock<HashMap<String, Arc<Mutex<ProcessHandle>>>>>;

/// Start a new ElizaOS CLI run with live log streaming
#[tauri::command]
pub async fn start_eliza_run_streaming(
    app: AppHandle,
    spec: RunSpec,
    config: SandboxConfig,
) -> Result<ApiResponse<RunResult>, String> {
    log::info!(
        "Starting ElizaOS CLI run with live streaming: {} {:?}",
        spec.mode,
        spec.args
    );

    if !config.is_valid() {
        return Ok(ApiResponse::error(
            "INVALID_CONFIG".to_string(),
            "Invalid Sandbox configuration".to_string(),
        ));
    }

    match execute_eliza_run_streaming(app, spec, config).await {
        Ok(result) => {
            log::info!("Started streaming ElizaOS CLI run: {}", result.id);
            Ok(ApiResponse::success(result))
        }
        Err(e) => {
            log::error!("Failed to start streaming ElizaOS CLI run: {}", e);
            Ok(ApiResponse::error(
                "START_ERROR".to_string(),
                format!("Failed to start streaming run: {}", e),
            ))
        }
    }
}

/// Start a new ElizaOS CLI run (simplified version for MVP - kept for compatibility)
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
    app: AppHandle,
    run_id: String,
) -> Result<ApiResponse<RunResult>, String> {
    log::info!("Stopping ElizaOS CLI run: {}", run_id);

    let registry = get_process_registry(&app);
    let mut guard = registry.write().await;

    match guard.get_mut(&run_id) {
        Some(process_handle_arc) => {
            let mut process_handle = process_handle_arc.lock().await;

            if process_handle.can_control {
                if let Some(pid) = process_handle.run_result.pid {
                    // Use system command to send SIGTERM
                    log::info!("Sending SIGTERM to process: PID={}, run_id={}", pid, run_id);

                    #[cfg(unix)]
                    {
                        use nix::sys::signal::{kill, Signal};
                        use nix::unistd::Pid;

                        match kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                            Ok(_) => {
                                log::info!("Successfully sent SIGTERM to PID: {}", pid);
                                process_handle.run_result.status = RunStatus::Killed;
                                process_handle.run_result.ended_at =
                                    Some(crate::models::current_timestamp());
                                process_handle.mark_completed();

                                let result = process_handle.run_result.clone();
                                Ok(ApiResponse::success(result))
                            }
                            Err(e) => {
                                log::error!("Failed to send SIGTERM to PID {}: {}", pid, e);
                                Ok(ApiResponse::error(
                                    "STOP_ERROR".to_string(),
                                    format!("Failed to stop process (PID: {}): {}", pid, e),
                                ))
                            }
                        }
                    }

                    #[cfg(not(unix))]
                    {
                        // On non-Unix systems, use std::process to terminate
                        match std::process::Command::new("taskkill")
                            .args(["/PID", &pid.to_string(), "/T", "/F"])
                            .output()
                        {
                            Ok(output) => {
                                if output.status.success() {
                                    log::info!("Successfully terminated process PID: {}", pid);
                                    process_handle.run_result.status = RunStatus::Killed;
                                    process_handle.run_result.ended_at =
                                        Some(crate::models::current_timestamp());
                                    process_handle.mark_completed();

                                    let result = process_handle.run_result.clone();
                                    Ok(ApiResponse::success(result))
                                } else {
                                    let error = String::from_utf8_lossy(&output.stderr);
                                    Ok(ApiResponse::error(
                                        "STOP_ERROR".to_string(),
                                        format!("Failed to stop process: {}", error),
                                    ))
                                }
                            }
                            Err(e) => Ok(ApiResponse::error(
                                "STOP_ERROR".to_string(),
                                format!("Failed to stop process: {}", e),
                            )),
                        }
                    }
                } else {
                    Ok(ApiResponse::error(
                        "NO_PID".to_string(),
                        "Process has no PID available for control".to_string(),
                    ))
                }
            } else {
                // Process already finished
                let result = process_handle.run_result.clone();
                Ok(ApiResponse::success(result))
            }
        }
        None => Ok(ApiResponse::error(
            "NOT_FOUND".to_string(),
            format!("Process {} not found or already completed", run_id),
        )),
    }
}

/// Kill a running ElizaOS CLI process forcefully
#[tauri::command]
pub async fn kill_eliza_run(
    app: AppHandle,
    run_id: String,
) -> Result<ApiResponse<RunResult>, String> {
    log::info!("Killing ElizaOS CLI run: {}", run_id);

    let registry = get_process_registry(&app);
    let mut guard = registry.write().await;

    match guard.get_mut(&run_id) {
        Some(process_handle_arc) => {
            let mut process_handle = process_handle_arc.lock().await;

            if process_handle.can_control {
                if let Some(pid) = process_handle.run_result.pid {
                    // Force kill the process (SIGKILL)
                    log::info!("Force killing process: PID={}, run_id={}", pid, run_id);

                    #[cfg(unix)]
                    {
                        use nix::sys::signal::{kill, Signal};
                        use nix::unistd::Pid;

                        match kill(Pid::from_raw(pid as i32), Signal::SIGKILL) {
                            Ok(_) => {
                                log::info!("Successfully sent SIGKILL to PID: {}", pid);
                                process_handle.run_result.status = RunStatus::Killed;
                                process_handle.run_result.ended_at =
                                    Some(crate::models::current_timestamp());
                                process_handle.mark_completed();

                                let result = process_handle.run_result.clone();
                                Ok(ApiResponse::success(result))
                            }
                            Err(e) => {
                                log::error!("Failed to send SIGKILL to PID {}: {}", pid, e);
                                Ok(ApiResponse::error(
                                    "KILL_ERROR".to_string(),
                                    format!("Failed to kill process (PID: {}): {}", pid, e),
                                ))
                            }
                        }
                    }

                    #[cfg(not(unix))]
                    {
                        // On non-Unix systems, use taskkill with /F (force)
                        match std::process::Command::new("taskkill")
                            .args(["/PID", &pid.to_string(), "/T", "/F"])
                            .output()
                        {
                            Ok(output) => {
                                if output.status.success() {
                                    log::info!(
                                        "Successfully force-terminated process PID: {}",
                                        pid
                                    );
                                    process_handle.run_result.status = RunStatus::Killed;
                                    process_handle.run_result.ended_at =
                                        Some(crate::models::current_timestamp());
                                    process_handle.mark_completed();

                                    let result = process_handle.run_result.clone();
                                    Ok(ApiResponse::success(result))
                                } else {
                                    let error = String::from_utf8_lossy(&output.stderr);
                                    Ok(ApiResponse::error(
                                        "KILL_ERROR".to_string(),
                                        format!("Failed to kill process: {}", error),
                                    ))
                                }
                            }
                            Err(e) => Ok(ApiResponse::error(
                                "KILL_ERROR".to_string(),
                                format!("Failed to kill process: {}", e),
                            )),
                        }
                    }
                } else {
                    Ok(ApiResponse::error(
                        "NO_PID".to_string(),
                        "Process has no PID available for control".to_string(),
                    ))
                }
            } else {
                // Process already finished
                let result = process_handle.run_result.clone();
                Ok(ApiResponse::success(result))
            }
        }
        None => Ok(ApiResponse::error(
            "NOT_FOUND".to_string(),
            format!("Process {} not found or already completed", run_id),
        )),
    }
}

/// Execute ElizaOS CLI run with simplified process management
async fn execute_eliza_run_simple(
    _app: AppHandle,
    spec: RunSpec,
    config: SandboxConfig,
) -> Result<RunResult, AppError> {
    // Generate unique run ID using safe format
    let run_id = crate::models::generate_safe_run_id();

    // Create initial run result
    let mut run_result = RunResult::new(spec.clone(), run_id.clone());

    // Determine ElizaOS CLI command
    let (eliza_cmd, use_npx) = resolve_eliza_command().await?;

    log::debug!("Using ElizaOS command: {} (npx: {})", eliza_cmd, use_npx);

    // Build command arguments based on mode
    let args = build_eliza_args(&spec, &config, use_npx)?;

    // Sanitize arguments for logging (remove sensitive information)
    let safe_args: Vec<String> = args
        .iter()
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

    // Build environment variables for ElizaOS CLI execution
    let env = build_eliza_env(&config);

    // Spawn the real ElizaOS CLI process
    let mut command = Command::new(&eliza_cmd);
    command.args(&args);
    command.envs(&env);

    if let Some(ref wd) = spec.working_dir {
        command.current_dir(wd);
    }

    // Configure for stdout/stderr capture
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let start_time = std::time::Instant::now();
    run_result.status = RunStatus::Running;

    log::info!(
        "Spawning real ElizaOS CLI process: {} {:?}",
        eliza_cmd,
        safe_args
    );

    // Execute and capture output
    match command.spawn() {
        Ok(child) => {
            // Wait for completion and capture output
            match child.wait_with_output() {
                Ok(output) => {
                    // Update run result with real data
                    run_result.status = if output.status.success() {
                        RunStatus::Completed
                    } else {
                        RunStatus::Failed
                    };

                    run_result.stdout = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .map(|s| s.to_string())
                        .collect();

                    run_result.stderr = String::from_utf8_lossy(&output.stderr)
                        .lines()
                        .map(|s| s.to_string())
                        .collect();

                    run_result.exit_code = output.status.code();
                    run_result.ended_at = Some(crate::models::current_timestamp());
                    run_result.duration_ms = Some(start_time.elapsed().as_millis() as u64);

                    log::info!(
                        "ElizaOS CLI process completed: exit_code={:?}, duration={}ms",
                        output.status.code(),
                        start_time.elapsed().as_millis()
                    );
                }
                Err(e) => {
                    run_result.status = RunStatus::Failed;
                    run_result
                        .stderr
                        .push(format!("Failed to wait for process: {}", e));
                    run_result.ended_at = Some(crate::models::current_timestamp());
                    run_result.duration_ms = Some(start_time.elapsed().as_millis() as u64);
                    log::error!("Failed to wait for ElizaOS CLI process: {}", e);
                }
            }
        }
        Err(e) => {
            run_result.status = RunStatus::Failed;
            run_result
                .stderr
                .push(format!("Failed to start process: {}", e));
            run_result.ended_at = Some(crate::models::current_timestamp());
            run_result.duration_ms = Some(start_time.elapsed().as_millis() as u64);
            log::error!("Failed to spawn ElizaOS CLI process: {}", e);
        }
    }

    Ok(run_result)
}

/// Execute ElizaOS CLI run with real-time log streaming
async fn execute_eliza_run_streaming(
    app: AppHandle,
    spec: RunSpec,
    config: SandboxConfig,
) -> Result<RunResult, AppError> {
    // Generate unique run ID using safe format
    let run_id = crate::models::generate_safe_run_id();

    // Create initial run result
    let mut run_result = RunResult::new(spec.clone(), run_id.clone());

    // Emit system log about starting
    let _ = app.emit(
        "log-event",
        LogEvent::system(
            run_id.clone(),
            "Starting ElizaOS CLI execution...".to_string(),
        ),
    );

    // Determine ElizaOS CLI command
    let (eliza_cmd, use_npx) = resolve_eliza_command().await?;

    log::debug!("Using ElizaOS command: {} (npx: {})", eliza_cmd, use_npx);

    // Build command arguments and environment
    let args = build_eliza_args(&spec, &config, use_npx)?;
    let env = build_eliza_env(&config);

    // Sanitize arguments for logging
    let safe_args: Vec<String> = args
        .iter()
        .map(|arg| {
            if arg.starts_with("eliza_") {
                format!("{}***", &arg[..12])
            } else {
                arg.clone()
            }
        })
        .collect();

    log::info!(
        "Executing with streaming: {} {} (working_dir: {:?})",
        eliza_cmd,
        safe_args.join(" "),
        spec.working_dir
    );

    // Emit command info
    let _ = app.emit(
        "log-event",
        LogEvent::info(
            run_id.clone(),
            format!("Command: {} {}", eliza_cmd, safe_args.join(" ")),
        ),
    );

    // Use tokio::process::Command for async execution
    let mut command = TokioCommand::new(&eliza_cmd);
    command.args(&args);
    command.envs(&env);

    if let Some(ref wd) = spec.working_dir {
        command.current_dir(wd);
    }

    // Configure for stdout/stderr capture
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let start_time = std::time::Instant::now();
    run_result.status = RunStatus::Running;

    // Spawn the process
    match command.spawn() {
        Ok(mut child) => {
            // Capture process ID and create initial process handle entry
            if let Some(pid) = child.id() {
                run_result.pid = Some(pid);
                log::info!("Started ElizaOS CLI process: PID={}", pid);

                // Register process in registry for control operations
                let registry = get_process_registry(&app);
                let process_handle = ProcessHandle::new(run_result.clone());
                let process_handle_arc = Arc::new(Mutex::new(process_handle));
                registry
                    .write()
                    .await
                    .insert(run_id.clone(), process_handle_arc);
            }

            // Get stdout and stderr handles
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| AppError::Process("Failed to get stdout handle".to_string()))?;

            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| AppError::Process("Failed to get stderr handle".to_string()))?;

            // Spawn tasks for streaming logs
            let app_stdout = app.clone();
            let run_id_stdout = run_id.clone();
            let stdout_task = tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                let mut stdout_lines = Vec::new();

                while let Ok(Some(line)) = lines.next_line().await {
                    stdout_lines.push(line.clone());
                    let _ =
                        app_stdout.emit("log-event", LogEvent::stdout(run_id_stdout.clone(), line));
                }
                stdout_lines
            });

            let app_stderr = app.clone();
            let run_id_stderr = run_id.clone();
            let stderr_task = tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                let mut stderr_lines = Vec::new();

                while let Ok(Some(line)) = lines.next_line().await {
                    stderr_lines.push(line.clone());
                    let _ =
                        app_stderr.emit("log-event", LogEvent::stderr(run_id_stderr.clone(), line));
                }
                stderr_lines
            });

            // Wait for process completion
            let status_result = child.wait().await;

            // Wait for log streaming tasks to complete
            let stdout_lines = stdout_task.await.unwrap_or_default();
            let mut stderr_lines = stderr_task.await.unwrap_or_default();

            // Update run result
            match status_result {
                Ok(status) => {
                    run_result.status = if status.success() {
                        RunStatus::Completed
                    } else {
                        RunStatus::Failed
                    };
                    run_result.exit_code = status.code();
                }
                Err(e) => {
                    run_result.status = RunStatus::Failed;
                    stderr_lines.push(format!("Process wait failed: {}", e));
                    log::error!("Process wait failed: {}", e);
                }
            }

            run_result.stdout = stdout_lines;
            run_result.stderr = stderr_lines;
            run_result.ended_at = Some(crate::models::current_timestamp());
            run_result.duration_ms = Some(start_time.elapsed().as_millis() as u64);

            // Update the process handle in the registry with the final result
            let registry = get_process_registry(&app);
            {
                let mut guard = registry.write().await;
                if let Some(process_handle_arc) = guard.get_mut(&run_id) {
                    let mut process_handle = process_handle_arc.lock().await;
                    process_handle.update_result(run_result.clone());
                    // Mark process as completed (no longer controllable)
                    process_handle.mark_completed();
                }
            }

            // Clean up completed processes from registry after a short delay
            let cleanup_registry = registry.clone();
            let cleanup_run_id = run_id.clone();
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                let mut guard = cleanup_registry.write().await;
                guard.remove(&cleanup_run_id);
                log::debug!(
                    "Cleaned up completed process from registry: {}",
                    cleanup_run_id
                );
            });

            // Emit completion event
            let status_msg = match run_result.status {
                RunStatus::Completed => format!(
                    "Process completed successfully (exit code: {:?})",
                    run_result.exit_code
                ),
                RunStatus::Failed => {
                    format!("Process failed (exit code: {:?})", run_result.exit_code)
                }
                _ => "Process ended".to_string(),
            };

            let _ = app.emit("log-event", LogEvent::system(run_id.clone(), status_msg));

            log::info!(
                "Streaming ElizaOS CLI process completed: exit_code={:?}, duration={}ms, stdout_lines={}, stderr_lines={}",
                run_result.exit_code,
                start_time.elapsed().as_millis(),
                run_result.stdout.len(),
                run_result.stderr.len()
            );

            Ok(run_result)
        }
        Err(e) => {
            run_result.status = RunStatus::Failed;
            run_result
                .stderr
                .push(format!("Failed to spawn process: {}", e));
            run_result.ended_at = Some(crate::models::current_timestamp());
            run_result.duration_ms = Some(start_time.elapsed().as_millis() as u64);

            let _ = app.emit(
                "log-event",
                LogEvent::error(run_id.clone(), format!("Failed to spawn process: {}", e)),
            );

            log::error!("Failed to spawn streaming ElizaOS CLI process: {}", e);
            Err(AppError::Process(format!("Failed to spawn process: {}", e)))
        }
    }
}

/// Resolve the ElizaOS CLI command to use
async fn resolve_eliza_command() -> Result<(String, bool), AppError> {
    // Try elizaos command (from @elizaos/cli package)
    if let Ok(output) = Command::new("elizaos").arg("--version").output() {
        if output.status.success() {
            log::debug!("Found elizaos CLI locally installed");
            return Ok(("elizaos".to_string(), false));
        }
    }

    // Try npx approach with correct package
    if let Ok(output) = Command::new("npx")
        .args(["-y", "@elizaos/cli@latest", "--version"])
        .output()
    {
        if output.status.success() {
            log::debug!("ElizaOS CLI available via npx");
            return Ok(("npx".to_string(), true));
        }
    }

    Err(AppError::CliNotFound(
        "ElizaOS CLI not available. Please install with: npm install -g @elizaos/cli@latest"
            .to_string(),
    ))
}

/// Build ElizaOS CLI arguments based on run specification
fn build_eliza_args(
    spec: &RunSpec,
    _config: &SandboxConfig,
    use_npx: bool,
) -> Result<Vec<String>, AppError> {
    let mut args = Vec::new();

    // If using npx, add the package specification
    if use_npx {
        args.push("-y".to_string());
        args.push("@elizaos/cli@latest".to_string());
    }

    // Actual ElizaOS CLI commands based on real CLI capabilities
    match spec.mode {
        RunMode::Doctor => {
            // Doctor mode: run system tests to check ElizaOS capabilities
            args.push("test".to_string());
            args.push("--type".to_string());
            args.push("component".to_string());
            args.push("--skip-build".to_string());
        }
        RunMode::Run => {
            // Run mode: Start ElizaOS agent server
            args.push("start".to_string());
            if !spec.args.is_empty() {
                // Add character file if specified in args
                args.push("--character".to_string());
                args.push(spec.args[0].clone());
            }
        }
        RunMode::Eval => {
            // Eval mode: Development mode
            args.push("dev".to_string());
        }
        RunMode::Custom => {
            // Custom command from spec.args[0] if available
            if !spec.args.is_empty() {
                args.push(spec.args[0].clone());
            } else {
                // Default to showing help
                args.push("--help".to_string());
            }
        }
    }

    // Add character file if specified
    if let Some(ref character_file) = spec.character_file {
        args.push("--character".to_string());
        args.push(character_file.clone());
    }

    // Add additional arguments (skip first for Custom mode since it's the command)
    let skip_count = if matches!(spec.mode, RunMode::Custom) && !spec.args.is_empty() {
        1
    } else {
        0
    };
    args.extend(spec.args.iter().skip(skip_count).cloned());

    Ok(args)
}

/// Build environment variables for ElizaOS CLI execution
fn build_eliza_env(config: &SandboxConfig) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // ElizaOS Cloud API environment variables (matching real ElizaOS structure)
    env.insert("ELIZAOS_BASE_URL".to_string(), config.base_url.clone());
    env.insert("ELIZAOS_API_KEY".to_string(), config.api_key.clone());

    if let Some(ref model) = config.default_model {
        env.insert("ELIZAOS_LARGE_MODEL".to_string(), model.clone());
        env.insert("ELIZAOS_SMALL_MODEL".to_string(), model.clone());
    }

    // ElizaOS-specific environment variables
    env.insert("NODE_ENV".to_string(), "production".to_string());
    env.insert("ELIZA_DESKTOP".to_string(), "true".to_string());

    log::debug!("Built environment variables for ElizaOS CLI (API keys redacted)");

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
        Some(process_handle_arc) => {
            let process_handle = process_handle_arc.lock().await;
            let run_result = process_handle.run_result.clone();
            Ok(ApiResponse::success(run_result))
        }
        None => Ok(ApiResponse::error(
            "NOT_FOUND".to_string(),
            format!("Run {} not found", run_id),
        )),
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
            mode: RunMode::Doctor,
            args: vec!["--verbose".to_string()],
            working_dir: None,
            character_file: None,
            env: std::collections::HashMap::new(),
        };

        let config = SandboxConfig {
            base_url: "https://api.example.com".to_string(),
            api_key: "eliza_test_key".to_string(),
            default_model: Some("gpt-4".to_string()),
        };

        let args = build_eliza_args(&spec, &config, true).unwrap();
        assert!(args.contains(&"start".to_string()));
        assert!(args.contains(&"--mode".to_string()));
        assert!(args.contains(&"diagnostic".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
    }

    #[test]
    fn test_build_eliza_env() {
        let config = SandboxConfig {
            base_url: "https://api.example.com".to_string(),
            api_key: "eliza_test_key".to_string(),
            default_model: Some("gpt-4".to_string()),
        };

        let env = build_eliza_env(&config);
        assert_eq!(
            env.get("ELIZAOS_BASE_URL"),
            Some(&"https://api.example.com".to_string())
        );
        assert_eq!(
            env.get("ELIZAOS_API_KEY"),
            Some(&"eliza_test_key".to_string())
        );
        assert_eq!(env.get("ELIZAOS_LARGE_MODEL"), Some(&"gpt-4".to_string()));
        assert_eq!(env.get("ELIZAOS_SMALL_MODEL"), Some(&"gpt-4".to_string()));
    }
}
