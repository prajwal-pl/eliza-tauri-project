//! Telemetry management for usage analytics
//! Handles posting telemetry data to Sandbox API

use crate::models::{ApiResponse, AppError, SandboxConfig, TelemetryEvent};
use reqwest::Client;
use std::time::Duration;

const TELEMETRY_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RETRY_ATTEMPTS: usize = 3;
const RETRY_DELAY: Duration = Duration::from_millis(1000);

/// Post telemetry event to Sandbox API
#[tauri::command]
pub async fn post_telemetry(
    config: SandboxConfig,
    event: TelemetryEvent,
) -> Result<ApiResponse<()>, String> {
    log::info!(
        "Posting telemetry event: {} {} ({}ms)",
        event.command,
        event.args.join(" "),
        event.duration_ms
    );

    if !config.is_valid() {
        log::warn!("Invalid configuration for telemetry");
        return Ok(ApiResponse::error(
            "INVALID_CONFIG".to_string(),
            "Invalid Sandbox configuration".to_string(),
        ));
    }

    match post_telemetry_event(&config, &event).await {
        Ok(_) => {
            log::info!("Telemetry event posted successfully");
            Ok(ApiResponse::success(()))
        }
        Err(e) => {
            log::error!("Failed to post telemetry: {}", e);
            // Don't fail the operation if telemetry fails
            Ok(ApiResponse::error(
                "TELEMETRY_ERROR".to_string(),
                format!("Failed to post telemetry: {}", e),
            ))
        }
    }
}

/// Generate device ID for telemetry
#[tauri::command]
pub async fn get_device_id() -> Result<ApiResponse<String>, String> {
    let device_id = crate::models::generate_device_id();
    log::debug!("Generated device ID: {}", device_id);
    Ok(ApiResponse::success(device_id))
}

/// Post telemetry event with retry logic
async fn post_telemetry_event(
    config: &SandboxConfig,
    event: &TelemetryEvent,
) -> Result<(), AppError> {
    let client = Client::builder()
        .timeout(TELEMETRY_TIMEOUT)
        .user_agent("ElizaOS-Desktop/0.1.0")
        .build()
        .map_err(|e| AppError::Network(format!("Failed to create HTTP client: {}", e)))?;

    let telemetry_url = format!(
        "{}/telemetry/cli",
        config.base_url.trim_end_matches('/')
    );

    let mut last_error = None;

    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        log::debug!("Telemetry attempt {} to {}", attempt, telemetry_url);

        match send_telemetry_request(&client, &telemetry_url, config, event).await {
            Ok(_) => {
                if attempt > 1 {
                    log::info!("Telemetry succeeded on attempt {}", attempt);
                }
                return Ok(());
            }
            Err(e) => {
                last_error = Some(e);
                if attempt < MAX_RETRY_ATTEMPTS {
                    log::warn!("Telemetry attempt {} failed, retrying...", attempt);
                    tokio::time::sleep(RETRY_DELAY * attempt as u32).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        AppError::Network("All telemetry attempts failed".to_string())
    }))
}

/// Send telemetry HTTP request
async fn send_telemetry_request(
    client: &Client,
    url: &str,
    config: &SandboxConfig,
    event: &TelemetryEvent,
) -> Result<(), AppError> {
    // Prepare the telemetry payload
    let payload = prepare_telemetry_payload(event);

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("X-Project-ID", &config.project_id)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                AppError::Network("Telemetry request timed out".to_string())
            } else if e.is_connect() {
                AppError::Network("Failed to connect to telemetry endpoint".to_string())
            } else {
                AppError::Network(format!("Telemetry request failed: {}", e))
            }
        })?;

    let status = response.status();

    if status.is_success() {
        log::debug!("Telemetry posted successfully: {}", status);
        Ok(())
    } else if status.as_u16() == 401 {
        Err(AppError::Network(
            "Telemetry authentication failed - check API key".to_string(),
        ))
    } else if status.as_u16() == 429 {
        Err(AppError::Network(
            "Telemetry rate limited - too many requests".to_string(),
        ))
    } else {
        let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(AppError::Network(format!(
            "Telemetry failed with status {}: {}",
            status, error_body
        )))
    }
}

/// Prepare telemetry payload for transmission
fn prepare_telemetry_payload(event: &TelemetryEvent) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "source": "desktop_client",
        "version": "0.1.0",
        "timestamp": event.started_at,
        "event": {
            "device_id": event.device_id,
            "command": event.command,
            "args": sanitize_args_for_telemetry(&event.args),
            "started_at": event.started_at,
            "duration_ms": event.duration_ms,
            "exit_code": event.exit_code,
            "bytes_out": event.bytes_out,
        }
    });

    // Add optional fields if present
    if let Some(tokens) = event.approx_tokens {
        payload["event"]["approx_tokens"] = serde_json::Value::Number(serde_json::Number::from(tokens));
    }

    if let Some(ref error) = event.error {
        payload["event"]["error"] = serde_json::Value::String(sanitize_error_for_telemetry(error));
    }

    if let Some(ref metadata) = event.metadata {
        payload["event"]["metadata"] = serde_json::to_value(metadata).unwrap_or(serde_json::Value::Null);
    }

    payload
}

/// Sanitize command arguments for telemetry (remove sensitive data)
fn sanitize_args_for_telemetry(args: &[String]) -> Vec<String> {
    args.iter()
        .map(|arg| {
            // Replace potential file paths and sensitive data
            if arg.contains('/') || arg.contains('\\') {
                "[FILE_PATH]".to_string()
            } else if arg.len() > 50 {
                // Truncate very long arguments (might be prompts or data)
                format!("{}...[TRUNCATED]", &arg[..47])
            } else if arg.starts_with("sk-") || arg.starts_with("eliza_") {
                "[API_KEY]".to_string()
            } else {
                arg.clone()
            }
        })
        .collect()
}

/// Sanitize error messages for telemetry
fn sanitize_error_for_telemetry(error: &str) -> String {
    // Remove potential sensitive information from error messages
    error
        .replace("sk-", "[API_KEY]")
        .replace("eliza_", "[API_KEY]")
        .chars()
        .take(500) // Limit error message length
        .collect()
}

/// Estimate token usage from output text
pub fn estimate_token_usage(text: &str) -> u64 {
    // Rough estimation: ~4 characters per token
    (text.len() / 4) as u64
}

/// Create telemetry event from run result
pub fn create_telemetry_event_from_run(
    device_id: String,
    command: &str,
    args: &[String],
    started_at: &str,
    duration_ms: u64,
    exit_code: i32,
    stdout: &[String],
    stderr: &[String],
) -> TelemetryEvent {
    let combined_output = format!("{}\n{}", stdout.join("\n"), stderr.join("\n"));
    let bytes_out = combined_output.len() as u64;
    let approx_tokens = estimate_token_usage(&combined_output);

    let error = if exit_code != 0 && !stderr.is_empty() {
        Some(stderr.join("\n"))
    } else {
        None
    };

    TelemetryEvent::new(
        device_id,
        command.to_string(),
        args.to_vec(),
        started_at.to_string(),
        duration_ms,
        exit_code,
        bytes_out,
    )
    .with_tokens(approx_tokens)
    .with_error(error.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_args_for_telemetry() {
        let args = vec![
            "run".to_string(),
            "-m".to_string(),
            "gpt-4".to_string(),
            "/path/to/file.txt".to_string(),
            "eliza_abc123def456".to_string(),
            "a".repeat(60), // Long argument
        ];

        let sanitized = sanitize_args_for_telemetry(&args);

        assert_eq!(sanitized[0], "run");
        assert_eq!(sanitized[1], "-m");
        assert_eq!(sanitized[2], "gpt-4");
        assert_eq!(sanitized[3], "[FILE_PATH]");
        assert_eq!(sanitized[4], "[API_KEY]");
        assert!(sanitized[5].ends_with("...[TRUNCATED]"));
    }

    #[test]
    fn test_sanitize_error_for_telemetry() {
        let error = "Authentication failed with key eliza_secret123 for user at /home/user/file.txt";
        let sanitized = sanitize_error_for_telemetry(error);

        assert!(sanitized.contains("[API_KEY]"));
        assert!(!sanitized.contains("eliza_secret123"));
        assert!(sanitized.len() <= 500);
    }

    #[test]
    fn test_estimate_token_usage() {
        let text = "This is a test message with some content";
        let tokens = estimate_token_usage(text);

        // Should be roughly text.len() / 4
        assert!(tokens >= (text.len() / 5) as u64);
        assert!(tokens <= (text.len() / 3) as u64);
    }

    #[test]
    fn test_create_telemetry_event_from_run() {
        let stdout = vec!["Line 1".to_string(), "Line 2".to_string()];
        let stderr = vec!["Error 1".to_string()];

        let event = create_telemetry_event_from_run(
            "device123".to_string(),
            "run",
            &["run".to_string(), "-m".to_string(), "gpt-4".to_string()],
            "2023-01-01T00:00:00Z",
            5000,
            0,
            &stdout,
            &stderr,
        );

        assert_eq!(event.device_id, "device123");
        assert_eq!(event.command, "run");
        assert_eq!(event.args, vec!["run", "-m", "gpt-4"]);
        assert_eq!(event.duration_ms, 5000);
        assert_eq!(event.exit_code, 0);
        assert!(event.bytes_out > 0);
        assert!(event.approx_tokens.is_some());
    }
}