//! Configuration management commands
//! Handles saving, loading, and testing Sandbox configurations using JSON file storage

use crate::models::{
    ApiResponse, AppError, ConnectionMetadata, ConnectionTestResult, SandboxConfig,
};
use tauri::Manager;
use reqwest::Client;
use serde_json;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::timeout;

const CONFIG_FILE: &str = "sandbox_config.json";
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

/// Save Sandbox configuration to JSON file
#[tauri::command]
pub async fn save_sandbox_config(
    app: tauri::AppHandle,
    config: SandboxConfig,
) -> Result<ApiResponse<()>, String> {
    log::info!("Saving Sandbox configuration");

    if !config.is_valid() {
        log::warn!("Invalid configuration provided: {}", sanitize_config_for_log(&config));
        return Ok(ApiResponse::error(
            "INVALID_CONFIG".to_string(),
            "Configuration is invalid".to_string(),
        ));
    }

    match save_config_to_file(&app, &config).await {
        Ok(_) => {
            log::info!("Configuration saved successfully");
            Ok(ApiResponse::success(()))
        }
        Err(e) => {
            log::error!("Failed to save configuration: {}", e);
            Ok(ApiResponse::error(
                "SAVE_ERROR".to_string(),
                format!("Failed to save configuration: {}", e),
            ))
        }
    }
}

/// Load Sandbox configuration from JSON file
#[tauri::command]
pub async fn load_sandbox_config(
    app: tauri::AppHandle,
) -> Result<ApiResponse<SandboxConfig>, String> {
    log::info!("Loading Sandbox configuration");

    match load_config_from_file(&app).await {
        Ok(Some(config)) => {
            log::info!("Configuration loaded successfully");
            Ok(ApiResponse::success(config))
        }
        Ok(None) => {
            log::info!("No configuration found");
            Ok(ApiResponse::error(
                "NO_CONFIG".to_string(),
                "No configuration found".to_string(),
            ))
        }
        Err(e) => {
            log::error!("Failed to load configuration: {}", e);
            Ok(ApiResponse::error(
                "LOAD_ERROR".to_string(),
                format!("Failed to load configuration: {}", e),
            ))
        }
    }
}

/// Clear saved Sandbox configuration
#[tauri::command]
pub async fn clear_sandbox_config(
    app: tauri::AppHandle,
) -> Result<ApiResponse<()>, String> {
    log::info!("Clearing Sandbox configuration");

    match clear_config_file(&app).await {
        Ok(_) => {
            log::info!("Configuration cleared successfully");
            Ok(ApiResponse::success(()))
        }
        Err(e) => {
            log::error!("Failed to clear configuration: {}", e);
            Ok(ApiResponse::error(
                "CLEAR_ERROR".to_string(),
                format!("Failed to clear configuration: {}", e),
            ))
        }
    }
}

/// Test connection to Sandbox API
#[tauri::command]
pub async fn test_sandbox_connection(
    config: SandboxConfig,
) -> Result<ApiResponse<ConnectionTestResult>, String> {
    log::info!("Testing connection to Sandbox API: {}", config.base_url);

    if !config.is_valid() {
        return Ok(ApiResponse::success(ConnectionTestResult {
            success: false,
            latency_ms: None,
            error: Some("Invalid configuration".to_string()),
            metadata: None,
        }));
    }

    match test_connection(&config).await {
        Ok(result) => {
            if result.success {
                log::info!(
                    "Connection test successful ({}ms)",
                    result.latency_ms.unwrap_or(0)
                );
            } else {
                log::warn!("Connection test failed: {:?}", result.error);
            }
            Ok(ApiResponse::success(result))
        }
        Err(e) => {
            log::error!("Connection test error: {}", e);
            Ok(ApiResponse::success(ConnectionTestResult {
                success: false,
                latency_ms: None,
                error: Some(e.to_string()),
                metadata: None,
            }))
        }
    }
}

/// Get the configuration file path
fn get_config_path(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| AppError::Config(format!("Failed to get app data directory: {}", e)))?;

    // Ensure the directory exists
    fs::create_dir_all(&app_data_dir)
        .map_err(|e| AppError::Config(format!("Failed to create app data directory: {}", e)))?;

    Ok(app_data_dir.join(CONFIG_FILE))
}

/// Save configuration to JSON file
async fn save_config_to_file(app: &tauri::AppHandle, config: &SandboxConfig) -> Result<(), AppError> {
    let config_path = get_config_path(app)?;

    let json_data = serde_json::to_string_pretty(config)
        .map_err(|e| AppError::Serialization(e))?;

    fs::write(&config_path, json_data)
        .map_err(|e| AppError::Config(format!("Failed to write config file: {}", e)))?;

    log::debug!("Configuration saved to: {:?}", config_path);
    Ok(())
}

/// Load configuration from JSON file
async fn load_config_from_file(app: &tauri::AppHandle) -> Result<Option<SandboxConfig>, AppError> {
    let config_path = get_config_path(app)?;

    if !config_path.exists() {
        return Ok(None);
    }

    let json_data = fs::read_to_string(&config_path)
        .map_err(|e| AppError::Config(format!("Failed to read config file: {}", e)))?;

    let config: SandboxConfig = serde_json::from_str(&json_data)
        .map_err(|e| AppError::Serialization(e))?;

    log::debug!("Configuration loaded from: {:?}", config_path);
    Ok(Some(config))
}

/// Clear configuration file
async fn clear_config_file(app: &tauri::AppHandle) -> Result<(), AppError> {
    let config_path = get_config_path(app)?;

    if config_path.exists() {
        fs::remove_file(&config_path)
            .map_err(|e| AppError::Config(format!("Failed to delete config file: {}", e)))?;
        log::debug!("Configuration file deleted: {:?}", config_path);
    }

    Ok(())
}

/// Perform actual connection test to Sandbox API
async fn test_connection(config: &SandboxConfig) -> Result<ConnectionTestResult, AppError> {
    let client = Client::builder()
        .timeout(CONNECTION_TIMEOUT)
        .user_agent("ElizaOS-Desktop/0.1.0")
        .build()
        .map_err(|e| AppError::Network(format!("Failed to create HTTP client: {}", e)))?;

    // Construct test endpoint URL
    let test_url = format!("{}/health", config.base_url.trim_end_matches('/'));

    log::debug!("Testing connection to: {}", test_url);

    let start_time = Instant::now();

    // Perform the connection test with timeout
    let response_result = timeout(CONNECTION_TIMEOUT, async {
        client
            .get(&test_url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .send()
            .await
    })
    .await;

    let latency_ms = start_time.elapsed().as_millis() as u64;

    match response_result {
        Ok(Ok(response)) => {
            let status = response.status();
            let success = status.is_success() || status == 401; // 401 means auth issue but API is reachable

            let metadata = ConnectionMetadata {
                endpoint: test_url,
                timestamp: crate::models::current_timestamp(),
                version: response
                    .headers()
                    .get("X-API-Version")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string()),
            };

            let error = if !success && status != 401 {
                Some(format!("HTTP {}: {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown")))
            } else if status == 401 {
                Some("Authentication failed - please check your API key".to_string())
            } else {
                None
            };

            Ok(ConnectionTestResult {
                success,
                latency_ms: Some(latency_ms),
                error,
                metadata: Some(metadata),
            })
        }
        Ok(Err(e)) => {
            log::warn!("HTTP request failed: {}", e);

            let error_message = if e.is_timeout() {
                "Connection timed out".to_string()
            } else if e.is_connect() {
                "Failed to connect - check your internet connection and base URL".to_string()
            } else if e.is_request() {
                "Invalid request - check your base URL format".to_string()
            } else {
                format!("Network error: {}", e)
            };

            Ok(ConnectionTestResult {
                success: false,
                latency_ms: Some(latency_ms),
                error: Some(error_message),
                metadata: None,
            })
        }
        Err(_) => {
            // Timeout occurred
            Ok(ConnectionTestResult {
                success: false,
                latency_ms: Some(CONNECTION_TIMEOUT.as_millis() as u64),
                error: Some("Connection timed out after 10 seconds".to_string()),
                metadata: None,
            })
        }
    }
}

/// Validate API key format
pub fn validate_api_key(api_key: &str) -> bool {
    api_key.starts_with("eliza_") && api_key.len() == 70
}

/// Validate base URL format
pub fn validate_base_url(base_url: &str) -> bool {
    base_url.starts_with("http://") || base_url.starts_with("https://")
}

/// Sanitize configuration for logging (redact API key)
pub fn sanitize_config_for_log(config: &SandboxConfig) -> String {
    format!(
        "SandboxConfig {{ base_url: \"{}\", api_key: \"{}***\", default_model: {:?} }}",
        config.base_url,
        &config.api_key[..12], // Show first 12 chars (eliza_ + 6 chars)
        config.default_model
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_api_key() {
        assert!(validate_api_key("eliza_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"));
        assert!(!validate_api_key("eliza_short"));
        assert!(!validate_api_key("invalid_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"));
        assert!(!validate_api_key(""));
    }

    #[test]
    fn test_validate_base_url() {
        assert!(validate_base_url("https://api.example.com"));
        assert!(validate_base_url("http://localhost:3000"));
        assert!(!validate_base_url("ftp://example.com"));
        assert!(!validate_base_url("example.com"));
        assert!(!validate_base_url(""));
    }

    #[test]
    fn test_sanitize_config_for_log() {
        let config = SandboxConfig {
            base_url: "https://api.example.com".to_string(),
            api_key: "eliza_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            default_model: Some("gpt-4".to_string()),
        };

        let sanitized = sanitize_config_for_log(&config);
        assert!(sanitized.contains("eliza_123456***"));
        assert!(sanitized.contains("https://api.example.com"));
        assert!(sanitized.contains("gpt-4"));
        assert!(!sanitized.contains("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"));
    }
}