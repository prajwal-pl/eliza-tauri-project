//! Terminal Commands - Handles terminal execution and process management
//! Provides safe terminal command execution with output streaming

use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use serde::{Deserialize, Serialize};
use tauri::State;
use crate::models::{ApiResponse, AppError};

// ============================================================================
// Terminal Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCommandResult {
    pub success: bool,
    pub output: Vec<String>,
    pub error: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalProcess {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: String,
    pub pid: Option<u32>,
    pub started_at: String,
    pub status: String, // "running", "completed", "failed", "killed"
}

// ============================================================================
// Terminal Process Registry
// ============================================================================

pub type TerminalRegistry = Arc<Mutex<HashMap<String, TerminalProcess>>>;

pub fn init_terminal_registry() -> TerminalRegistry {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Cleanup old completed processes to prevent memory leaks
fn cleanup_old_processes(registry: &mut HashMap<String, TerminalProcess>) {
    const MAX_COMPLETED_PROCESSES: usize = 100;

    // Get completed processes sorted by start time
    let mut completed_processes: Vec<_> = registry
        .iter()
        .filter(|(_, process)| process.status == "completed" || process.status == "failed")
        .map(|(id, process)| (id.clone(), process.started_at.clone()))
        .collect();

    if completed_processes.len() > MAX_COMPLETED_PROCESSES {
        // Sort by start time (oldest first)
        completed_processes.sort_by(|a, b| a.1.cmp(&b.1));

        // Remove oldest completed processes
        let to_remove = completed_processes.len() - MAX_COMPLETED_PROCESSES;
        for (id, _) in completed_processes.iter().take(to_remove) {
            registry.remove(id);
            log::debug!("Cleaned up old completed process: {}", id);
        }

        log::info!("Cleaned up {} old processes from registry", to_remove);
    }
}

// ============================================================================
// Terminal Commands
// ============================================================================

/// Initialize terminal backend
#[tauri::command]
pub async fn initialize_terminal() -> Result<ApiResponse<bool>, AppError> {
    log::info!("Initializing terminal backend");

    // Perform any necessary terminal setup
    // For now, this is just a placeholder

    Ok(ApiResponse::success(true))
}

/// Execute a terminal command with real-time output capture
#[tauri::command]
pub async fn execute_terminal_command(
    command: String,
    args: Vec<String>,
    working_dir: Option<String>,
    registry: State<'_, TerminalRegistry>,
) -> Result<TerminalCommandResult, AppError> {
    log::info!("Executing terminal command: {} with args: {:?}", command, args);

    let start_time = std::time::Instant::now();

    // Resolve working directory properly
    let work_dir = match working_dir {
        Some(dir) => resolve_working_directory(dir),
        None => get_default_working_directory(),
    };

    log::debug!("Working directory: {}", work_dir);

    // Validate command for security
    let security_check = is_safe_command(&command);
    log::debug!("Security check for command '{}': {}", command, security_check);

    if !security_check {
        log::warn!("Command '{}' blocked for security reasons", command);
        return Ok(TerminalCommandResult {
            success: false,
            output: vec![],
            error: Some(format!("Command '{}' is not allowed for security reasons", command)),
            exit_code: Some(1),
            duration_ms: start_time.elapsed().as_millis() as u64,
        });
    }

    let process_id = format!("term_{}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
        rand::random::<u16>()
    );

    // Create terminal process entry
    let terminal_process = TerminalProcess {
        id: process_id.clone(),
        command: command.clone(),
        args: args.clone(),
        working_dir: work_dir.clone(),
        pid: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        status: "running".to_string(),
    };

    // Register process
    {
        let mut reg = registry.lock().unwrap();
        reg.insert(process_id.clone(), terminal_process);
    }

    log::debug!("About to execute command: {} with args: {:?} in dir: {}", command, args, work_dir);

    // Execute command using appropriate method (shell vs binary)
    let execution_result = if should_use_shell(&command) {
        log::debug!("Using shell execution for command: {}", command);
        execute_shell_command(&command, &args, &work_dir).await
    } else {
        log::debug!("Using binary execution for command: {}", command);
        match execute_binary_command(&command, &args, &work_dir).await {
            Ok(result) => Ok(result),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::debug!("Binary '{}' not found, falling back to shell execution", command);
                execute_shell_command(&command, &args, &work_dir).await
            }
            Err(e) => Err(e),
        }
    };

    // Process execution result
    match execution_result {
        Ok((stdout_output, stderr_output, exit_code)) => {
            let success = exit_code == Some(0) || exit_code.is_none();
            log::debug!("Command completed. Exit code: {:?}, Success: {}", exit_code, success);
            log::debug!("Output - stdout lines: {}, stderr lines: {}", stdout_output.len(), stderr_output.len());

            // Combine stdout and stderr for output (with size limits to prevent memory issues)
            let mut combined_output = stdout_output;
            if !stderr_output.is_empty() {
                combined_output.extend(stderr_output.iter().map(|line| format!("stderr: {}", line)));
            }

            // Truncate output if it's too large to prevent memory issues
            const MAX_OUTPUT_LINES: usize = 1000;
            if combined_output.len() > MAX_OUTPUT_LINES {
                let truncated_count = combined_output.len() - MAX_OUTPUT_LINES;
                combined_output.truncate(MAX_OUTPUT_LINES);
                combined_output.push(format!("... ({} more lines truncated to prevent memory issues)", truncated_count));
            }

            // Update registry and cleanup old processes
            {
                let mut reg = registry.lock().unwrap();
                if let Some(process) = reg.get_mut(&process_id) {
                    process.status = if success { "completed" } else { "failed" }.to_string();
                }

                // Cleanup old completed processes to prevent memory leaks
                cleanup_old_processes(&mut reg);
            }

            Ok(TerminalCommandResult {
                success,
                output: combined_output,
                error: if stderr_output.is_empty() { None } else { Some(stderr_output.join("\n")) },
                exit_code,
                duration_ms: start_time.elapsed().as_millis() as u64,
            })
        }
        Err(e) => {
            log::error!("Command execution failed: {}", e);

            // Update registry and cleanup old processes
            {
                let mut reg = registry.lock().unwrap();
                if let Some(process) = reg.get_mut(&process_id) {
                    process.status = "failed".to_string();
                }

                // Cleanup old completed processes to prevent memory leaks
                cleanup_old_processes(&mut reg);
            }

            Ok(TerminalCommandResult {
                success: false,
                output: vec![],
                error: Some(format!("Failed to spawn command: {}", e)),
                exit_code: Some(1),
                duration_ms: start_time.elapsed().as_millis() as u64,
            })
        }
    }
}

/// Cancel a running terminal command
#[tauri::command]
pub async fn cancel_terminal_command(
    command_id: String,
    registry: State<'_, TerminalRegistry>,
) -> Result<ApiResponse<bool>, AppError> {
    log::info!("Cancelling terminal command: {}", command_id);

    let mut reg = registry.lock().unwrap();

    if let Some(process) = reg.get_mut(&command_id) {
        if let Some(pid) = process.pid {
            #[cfg(unix)]
            {
                use nix::sys::signal::{self, Signal};
                use nix::unistd::Pid;

                match signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                    Ok(_) => {
                        process.status = "killed".to_string();
                        log::info!("Successfully sent SIGTERM to process {}", pid);
                        return Ok(ApiResponse::success(true));
                    }
                    Err(e) => {
                        log::error!("Failed to kill process {}: {}", pid, e);
                        return Ok(ApiResponse::error(
                            "KILL_FAILED".to_string(),
                            format!("Failed to kill process: {}", e)
                        ));
                    }
                }
            }

            #[cfg(windows)]
            {
                // On Windows, we would use different approach
                log::warn!("Process termination on Windows not yet implemented");
                return Ok(ApiResponse::error(
                    "NOT_IMPLEMENTED".to_string(),
                    "Process termination on Windows not yet implemented".to_string()
                ));
            }
        } else {
            return Ok(ApiResponse::error(
                "NO_PID".to_string(),
                "Process has no PID available".to_string()
            ));
        }
    } else {
        return Ok(ApiResponse::error(
            "NOT_FOUND".to_string(),
            "Command not found in registry".to_string()
        ));
    }
}

/// Get list of running terminal processes
#[tauri::command]
pub async fn get_terminal_processes(
    registry: State<'_, TerminalRegistry>,
) -> Result<ApiResponse<Vec<TerminalProcess>>, AppError> {
    let reg = registry.lock().unwrap();
    let processes: Vec<TerminalProcess> = reg.values().cloned().collect();
    Ok(ApiResponse::success(processes))
}

/// Get current working directory
#[tauri::command]
pub async fn get_terminal_cwd() -> Result<ApiResponse<String>, AppError> {
    let cwd = get_default_working_directory();
    log::debug!("Current working directory: {}", cwd);
    Ok(ApiResponse::success(cwd))
}

/// Change working directory
#[tauri::command]
pub async fn change_terminal_cwd(path: String) -> Result<ApiResponse<String>, AppError> {
    let resolved_path = resolve_working_directory(path.clone());
    log::debug!("Changing directory from '{}' to '{}'", path, resolved_path);

    match std::env::set_current_dir(&resolved_path) {
        Ok(_) => {
            let new_path = get_default_working_directory();
            log::info!("Working directory changed to: {}", new_path);
            Ok(ApiResponse::success(new_path))
        }
        Err(e) => {
            log::error!("Failed to change directory to '{}': {}", resolved_path, e);
            Ok(ApiResponse::error(
                "CWD_CHANGE_ERROR".to_string(),
                format!("Failed to change directory to '{}': {}", resolved_path, e)
            ))
        }
    }
}

// ============================================================================
// Security and Validation
// ============================================================================

/// Check if a command is safe to execute
fn is_safe_command(command: &str) -> bool {
    log::debug!("Checking security for command: '{}'", command);

    // Allow common safe commands
    const ALLOWED_COMMANDS: &[&str] = &[
        // Basic file operations
        "ls", "dir", "pwd", "cd", "echo", "cat", "type", "head", "tail", "less", "more",
        "touch", "mkdir", "cp", "mv", "ln",
        // Text processing
        "grep", "sed", "awk", "sort", "uniq", "wc", "cut", "tr",
        // System info
        "which", "whereis", "whoami", "id", "date", "uptime", "uname", "hostname",
        "ps", "top", "htop", "df", "du", "free", "env", "printenv",
        // Development tools
        "git", "npm", "node", "python", "python3", "pip", "cargo", "rustc",
        "make", "cmake", "gcc", "g++", "clang",
        // Network tools (read-only)
        "ping", "curl", "wget", "dig", "nslookup",
        // Help and documentation
        "help", "man", "info", "--help", "-h",
        // ElizaOS CLI commands
        "elizaos", "eliza", "doctor",
    ];

    // Block dangerous commands
    const BLOCKED_COMMANDS: &[&str] = &[
        // File system dangers
        "rm", "del", "format", "fdisk", "mkfs", "dd", "shred",
        // Privilege escalation
        "sudo", "su", "doas",
        // Permission changes
        "chmod", "chown", "chgrp", "setfacl", "chattr",
        // System control
        "systemctl", "service", "init", "shutdown", "reboot", "halt", "poweroff",
        // Network configuration
        "iptables", "netsh", "route", "ifconfig", "ip",
        // Mount operations
        "mount", "umount", "losetup",
        // Job control
        "crontab", "at", "batch",
    ];

    let cmd = command.to_lowercase();
    log::debug!("Normalized command: '{}'", cmd);

    // Check if command is explicitly blocked
    for &blocked in BLOCKED_COMMANDS {
        if cmd == blocked || cmd.starts_with(&format!("{} ", blocked)) {
            log::warn!("Command '{}' matches blocked command '{}'", command, blocked);
            return false;
        }
    }

    // Allow if command is in allowed list
    for &allowed in ALLOWED_COMMANDS {
        if cmd == allowed || cmd.starts_with(&format!("{} ", allowed)) {
            log::debug!("Command '{}' matches allowed command '{}'", command, allowed);
            return true;
        }
    }

    // Be more permissive for development - allow commands that don't match dangerous patterns
    if cmd.chars().all(|c| c.is_alphanumeric() || " -_./".contains(c)) {
        log::info!("Command '{}' passed character validation, allowing", command);
        return true;
    }

    log::warn!("Command '{}' blocked - no explicit allow match and failed character validation", command);
    false
}

// ============================================================================
// Working Directory Resolution
// ============================================================================

/// Resolve working directory with proper ~ expansion and validation
fn resolve_working_directory(dir: String) -> String {
    log::debug!("Resolving working directory: '{}'", dir);

    let expanded_dir = if dir.starts_with('~') {
        match dirs::home_dir() {
            Some(home) => {
                let path = if dir == "~" {
                    home
                } else if dir.starts_with("~/") {
                    home.join(&dir[2..])
                } else {
                    // Handle cases like ~username (not supported, fallback to current dir)
                    log::warn!("Unsupported path format: '{}', using current directory", dir);
                    return get_default_working_directory();
                };
                path.to_string_lossy().to_string()
            }
            None => {
                log::warn!("Cannot resolve home directory for path: '{}', using current directory", dir);
                return get_default_working_directory();
            }
        }
    } else {
        dir
    };

    // Validate that the directory exists
    if std::path::Path::new(&expanded_dir).is_dir() {
        log::debug!("Working directory resolved to: '{}'", expanded_dir);
        expanded_dir
    } else {
        log::warn!("Directory '{}' does not exist, using current directory", expanded_dir);
        get_default_working_directory()
    }
}

/// Get the default working directory
fn get_default_working_directory() -> String {
    std::env::current_dir()
        .unwrap_or_else(|_| {
            // Fallback to home directory if current dir is not accessible
            dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        })
        .to_string_lossy()
        .to_string()
}

// ============================================================================
// Shell Command Processing
// ============================================================================

/// Determine if a command should be run through a shell
fn should_use_shell(command: &str) -> bool {
    // Commands that are shell builtins or require shell processing
    const SHELL_BUILTINS: &[&str] = &[
        "help", "cd", "pwd", "echo", "test", "[", "alias", "bg", "bind", "break",
        "builtin", "caller", "command", "compgen", "complete", "compopt", "continue",
        "declare", "dirs", "disown", "enable", "eval", "exec", "exit", "export",
        "false", "fc", "fg", "getopts", "hash", "history", "jobs", "kill", "let",
        "local", "logout", "mapfile", "popd", "printf", "pushd", "read", "readonly",
        "return", "set", "shift", "shopt", "source", "suspend", "times", "trap",
        "true", "type", "typeset", "ulimit", "umask", "unalias", "unset", "wait"
    ];

    SHELL_BUILTINS.contains(&command)
}

/// Execute command through shell
async fn execute_shell_command(
    command: &str,
    args: &[String],
    work_dir: &str,
) -> Result<(Vec<String>, Vec<String>, Option<i32>), std::io::Error> {
    log::debug!("Executing shell command: {} {:?}", command, args);

    // Construct the full command string
    let full_command = if args.is_empty() {
        command.to_string()
    } else {
        format!("{} {}", command, args.join(" "))
    };

    log::debug!("Full shell command: '{}'", full_command);

    // Use bash to execute the command
    let mut cmd = Command::new("bash");
    cmd.arg("-c")
        .arg(&full_command)
        .current_dir(work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_output = Vec::new();
    let mut stderr_output = Vec::new();

    // Read stdout
    let mut stdout_reader = BufReader::new(stdout);
    let mut stdout_line = String::new();
    while stdout_reader.read_line(&mut stdout_line).await? > 0 {
        stdout_output.push(stdout_line.trim_end().to_string());
        stdout_line.clear();
    }

    // Read stderr
    let mut stderr_reader = BufReader::new(stderr);
    let mut stderr_line = String::new();
    while stderr_reader.read_line(&mut stderr_line).await? > 0 {
        stderr_output.push(stderr_line.trim_end().to_string());
        stderr_line.clear();
    }

    let status = child.wait().await?;
    let exit_code = status.code();

    Ok((stdout_output, stderr_output, exit_code))
}

/// Execute command directly as binary
async fn execute_binary_command(
    command: &str,
    args: &[String],
    work_dir: &str,
) -> Result<(Vec<String>, Vec<String>, Option<i32>), std::io::Error> {
    log::debug!("Executing binary command: {} {:?}", command, args);

    let mut cmd = Command::new(command);
    cmd.args(args)
        .current_dir(work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_output = Vec::new();
    let mut stderr_output = Vec::new();

    // Read stdout
    let mut stdout_reader = BufReader::new(stdout);
    let mut stdout_line = String::new();
    while stdout_reader.read_line(&mut stdout_line).await? > 0 {
        stdout_output.push(stdout_line.trim_end().to_string());
        stdout_line.clear();
    }

    // Read stderr
    let mut stderr_reader = BufReader::new(stderr);
    let mut stderr_line = String::new();
    while stderr_reader.read_line(&mut stderr_line).await? > 0 {
        stderr_output.push(stderr_line.trim_end().to_string());
        stderr_line.clear();
    }

    let status = child.wait().await?;
    let exit_code = status.code();

    Ok((stdout_output, stderr_output, exit_code))
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Clean up completed/failed processes from registry
#[tauri::command]
pub async fn cleanup_terminal_processes(
    registry: State<'_, TerminalRegistry>,
) -> Result<ApiResponse<usize>, AppError> {
    let mut reg = registry.lock().unwrap();

    let initial_count = reg.len();
    reg.retain(|_id, process| {
        process.status == "running"
    });

    let cleaned_count = initial_count - reg.len();
    log::info!("Cleaned up {} terminal processes", cleaned_count);

    Ok(ApiResponse::success(cleaned_count))
}