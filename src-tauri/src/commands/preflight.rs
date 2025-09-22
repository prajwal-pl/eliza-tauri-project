//! Preflight checks for system requirements
//! Verifies Node.js, npm, and ElizaOS CLI availability

use crate::models::{ApiResponse, AppError, PreflightResult, ToolCheck};
use std::process::Command;
use tauri_plugin_os::platform;

/// Run comprehensive preflight checks
#[tauri::command]
pub async fn preflight_check() -> Result<ApiResponse<PreflightResult>, String> {
    log::info!("Running preflight checks");

    match run_preflight_checks().await {
        Ok(result) => {
            log::info!("Preflight checks completed: {:?}", result.overall_status);
            Ok(ApiResponse::success(result))
        }
        Err(e) => {
            log::error!("Preflight check failed: {}", e);
            Ok(ApiResponse::error(
                "PREFLIGHT_ERROR".to_string(),
                e.to_string(),
            ))
        }
    }
}

/// Internal function to run all preflight checks
async fn run_preflight_checks() -> Result<PreflightResult, AppError> {
    log::debug!("Checking Node.js installation");
    let node_check = check_nodejs().await?;

    log::debug!("Checking npm installation");
    let npm_check = check_npm().await?;

    log::debug!("Checking ElizaOS CLI installation");
    let eliza_check = check_eliza_cli().await?;

    Ok(PreflightResult::new(node_check, npm_check, eliza_check))
}

/// Check Node.js installation and version
async fn check_nodejs() -> Result<ToolCheck, AppError> {
    // Try different possible Node.js commands
    let node_commands = ["node", "nodejs"];

    for cmd in &node_commands {
        match check_tool_version(cmd, "--version").await {
            Ok(Some((version, path))) => {
                log::debug!("Found Node.js {} at {}", version, path);
                return Ok(ToolCheck::found(version, path));
            }
            Ok(None) => continue,
            Err(e) => {
                log::debug!("Error checking {}: {}", cmd, e);
                continue;
            }
        }
    }

    log::warn!("Node.js not found");
    Ok(ToolCheck::not_found())
}

/// Check npm installation and version
async fn check_npm() -> Result<ToolCheck, AppError> {
    // Try npm and pnpm
    let package_managers = [
        ("npm", "--version"),
        ("pnpm", "--version"),
        ("yarn", "--version"),
    ];

    for (cmd, version_flag) in &package_managers {
        match check_tool_version(cmd, version_flag).await {
            Ok(Some((version, path))) => {
                log::debug!("Found package manager {} {} at {}", cmd, version, path);
                return Ok(ToolCheck::found(version, path));
            }
            Ok(None) => continue,
            Err(e) => {
                log::debug!("Error checking {}: {}", cmd, e);
                continue;
            }
        }
    }

    log::warn!("No package manager found");
    Ok(ToolCheck::not_found())
}

/// Check ElizaOS CLI installation
async fn check_eliza_cli() -> Result<ToolCheck, AppError> {
    // First try to find eliza CLI directly
    match check_tool_version("eliza", "--version").await {
        Ok(Some((version, path))) => {
            log::debug!("Found ElizaOS CLI {} at {}", version, path);
            return Ok(ToolCheck::found(version, path));
        }
        Ok(None) | Err(_) => {
            log::debug!("ElizaOS CLI not found in PATH");
        }
    }

    // Try to check if it's available via npx
    match check_npx_eliza().await {
        Ok(true) => {
            log::debug!("ElizaOS CLI available via npx");
            Ok(ToolCheck::found(
                "available via npx".to_string(),
                "npx eliza".to_string(),
            ))
        }
        Ok(false) => {
            log::warn!("ElizaOS CLI not available");
            Ok(ToolCheck::not_found())
        }
        Err(e) => {
            log::warn!("Error checking npx eliza: {}", e);
            Ok(ToolCheck::not_found())
        }
    }
}

/// Check if ElizaOS CLI is available via npx
async fn check_npx_eliza() -> Result<bool, AppError> {
    let output = Command::new("npx")
        .args(["--yes", "eliza", "--help"])
        .output()
        .map_err(|e| AppError::Process(format!("Failed to run npx: {}", e)))?;

    // If the command succeeds and contains help text, eliza is available
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("ElizaOS") || stdout.contains("Usage:") || stdout.contains("Commands:"))
    } else {
        Ok(false)
    }
}

/// Generic function to check tool version and location
async fn check_tool_version(
    command: &str,
    version_flag: &str,
) -> Result<Option<(String, String)>, AppError> {
    // First check if command exists
    let which_output = Command::new(get_which_command())
        .arg(command)
        .output()
        .map_err(|e| AppError::Process(format!("Failed to check if {} exists: {}", command, e)))?;

    if !which_output.status.success() {
        return Ok(None);
    }

    let path = String::from_utf8_lossy(&which_output.stdout).trim().to_string();
    if path.is_empty() {
        return Ok(None);
    }

    // Get version information
    let version_output = Command::new(command)
        .arg(version_flag)
        .output()
        .map_err(|e| AppError::Process(format!("Failed to get {} version: {}", command, e)))?;

    if version_output.status.success() {
        let version_text = String::from_utf8_lossy(&version_output.stdout);
        let version = extract_version(&version_text).unwrap_or_else(|| version_text.trim().to_string());

        Ok(Some((version, path)))
    } else {
        // Command exists but version check failed - still report it as found
        Ok(Some(("unknown".to_string(), path)))
    }
}

/// Get the appropriate "which" command for the current platform
fn get_which_command() -> &'static str {
    if platform().to_string().to_lowercase().contains("windows") {
        "where"
    } else {
        "which"
    }
}

/// Extract version number from version output
fn extract_version(output: &str) -> Option<String> {
    // Simple version extraction without regex
    let lines: Vec<&str> = output.lines().collect();
    for line in lines {
        let words: Vec<&str> = line.split_whitespace().collect();
        for word in words {
            // Look for version patterns like v1.2.3 or 1.2.3
            if let Some(version) = extract_version_from_word(word) {
                return Some(version);
            }
        }
    }
    None
}

/// Extract version from a single word
fn extract_version_from_word(word: &str) -> Option<String> {
    let cleaned = word.trim_start_matches('v');
    let parts: Vec<&str> = cleaned.split('.').collect();

    if parts.len() >= 2 && parts.len() <= 4 {
        let mut version_parts = Vec::new();
        for part in parts {
            if let Ok(num) = part.parse::<u32>() {
                version_parts.push(num.to_string());
            } else {
                return None; // Not a valid version number
            }
        }
        return Some(version_parts.join("."));
    }

    None
}

/// Get system information for diagnostics
pub fn get_system_info() -> String {
    format!(
        "Platform: {}, Arch: {}, OS: {}",
        platform(),
        std::env::consts::ARCH,
        std::env::consts::OS
    )
}

/// Generate installation recommendations based on platform
pub fn get_installation_recommendations() -> Vec<String> {
    let mut recommendations = Vec::new();
    let platform_str = platform().to_string().to_lowercase();

    if platform_str.contains("windows") {
        recommendations.push("Install Node.js from https://nodejs.org/ (choose LTS version)".to_string());
        recommendations.push("npm comes bundled with Node.js".to_string());
        recommendations.push("ElizaOS CLI will be installed automatically when needed".to_string());
    } else if platform_str.contains("darwin") || platform_str.contains("macos") {
        recommendations.push("Install Node.js via Homebrew: brew install node".to_string());
        recommendations.push("Or download from https://nodejs.org/ (choose LTS version)".to_string());
        recommendations.push("ElizaOS CLI will be installed automatically when needed".to_string());
    } else if platform_str.contains("linux") {
        recommendations.push("Install Node.js via package manager or from https://nodejs.org/".to_string());
        recommendations.push("Ubuntu/Debian: sudo apt install nodejs npm".to_string());
        recommendations.push("CentOS/RHEL: sudo yum install nodejs npm".to_string());
        recommendations.push("ElizaOS CLI will be installed automatically when needed".to_string());
    } else {
        recommendations.push("Install Node.js 18+ from https://nodejs.org/".to_string());
        recommendations.push("Ensure npm is available".to_string());
    }

    recommendations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version() {
        assert_eq!(extract_version("v18.17.0"), Some("18.17.0".to_string()));
        assert_eq!(extract_version("9.8.1"), Some("9.8.1".to_string()));
        assert_eq!(extract_version("node v20.5.0"), Some("20.5.0".to_string()));
        assert_eq!(extract_version("npm 10.2.4"), Some("10.2.4".to_string()));
        assert_eq!(extract_version("no version here"), None);
    }

    #[test]
    fn test_extract_version_from_word() {
        assert_eq!(extract_version_from_word("v18.17.0"), Some("18.17.0".to_string()));
        assert_eq!(extract_version_from_word("9.8.1"), Some("9.8.1".to_string()));
        assert_eq!(extract_version_from_word("1.2.3.4"), Some("1.2.3.4".to_string()));
        assert_eq!(extract_version_from_word("not-a-version"), None);
        assert_eq!(extract_version_from_word("1"), None); // Too few parts
    }

    #[tokio::test]
    async fn test_preflight_check_structure() {
        // This test just ensures the function can be called
        let result = preflight_check().await;
        assert!(result.is_ok());
    }
}