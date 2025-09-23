//! Core data models for MVP Tauri ElizaOS CLI
//! These structs match the TypeScript interfaces for proper IPC serialization

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Configuration Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxConfig {
    pub base_url: String,
    pub api_key: String,
    pub default_model: Option<String>,
}

impl SandboxConfig {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url,
            api_key,
            default_model: None,
        }
    }

    pub fn with_default_model(mut self, model: String) -> Self {
        self.default_model = Some(model);
        self
    }

    pub fn is_valid(&self) -> bool {
        !self.base_url.is_empty()
            && !self.api_key.is_empty()
            && self.base_url.starts_with("http")
            && self.api_key.starts_with("eliza_")
            && self.api_key.len() == 70 // "eliza_" + 64 hex chars
    }
}

// ============================================================================
// Process Management Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Doctor,
    Run,
    Eval,
    Custom,
}

impl std::fmt::Display for RunMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunMode::Doctor => write!(f, "doctor"),
            RunMode::Run => write!(f, "run"),
            RunMode::Eval => write!(f, "eval"),
            RunMode::Custom => write!(f, "custom"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSpec {
    pub id: String,
    pub mode: RunMode,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<String>,
    pub character_file: Option<String>,
}

impl RunSpec {
    pub fn new(id: String, mode: RunMode, args: Vec<String>) -> Self {
        Self {
            id,
            mode,
            args,
            env: HashMap::new(),
            working_dir: None,
            character_file: None,
        }
    }

    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    pub fn with_working_dir(mut self, dir: String) -> Self {
        self.working_dir = Some(dir);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
    Killed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub id: String,
    pub spec: RunSpec,
    pub started_at: String, // ISO 8601 timestamp
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    pub duration_ms: Option<u64>,
    pub status: RunStatus,
    pub pid: Option<u32>, // Process ID for active process management
}

impl RunResult {
    pub fn new(spec: RunSpec, started_at: String) -> Self {
        Self {
            id: spec.id.clone(),
            spec,
            started_at,
            ended_at: None,
            exit_code: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            duration_ms: None,
            status: RunStatus::Running,
            pid: None, // Will be set when process starts
        }
    }

    pub fn with_pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }

    pub fn complete(mut self, exit_code: i32, ended_at: String, duration_ms: u64) -> Self {
        self.exit_code = Some(exit_code);
        self.ended_at = Some(ended_at);
        self.duration_ms = Some(duration_ms);
        self.status = if exit_code == 0 {
            RunStatus::Completed
        } else {
            RunStatus::Failed
        };
        self
    }

    pub fn kill(mut self, ended_at: String, duration_ms: u64) -> Self {
        self.ended_at = Some(ended_at);
        self.duration_ms = Some(duration_ms);
        self.status = RunStatus::Killed;
        self
    }
}

// ============================================================================
// Preflight Check Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCheck {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

impl ToolCheck {
    pub fn not_found() -> Self {
        Self {
            installed: false,
            version: None,
            path: None,
        }
    }

    pub fn found(version: String, path: String) -> Self {
        Self {
            installed: true,
            version: Some(version),
            path: Some(path),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreflightStatus {
    Ready,
    NeedsSetup,
    CriticalIssues,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightResult {
    pub node: ToolCheck,
    pub npm: ToolCheck,
    pub eliza: ToolCheck,
    pub recommendations: Vec<String>,
    pub overall_status: PreflightStatus,
}

impl PreflightResult {
    pub fn new(node: ToolCheck, npm: ToolCheck, eliza: ToolCheck) -> Self {
        let mut recommendations = Vec::new();
        let overall_status = Self::determine_status(&node, &npm, &eliza, &mut recommendations);

        Self {
            node,
            npm,
            eliza,
            recommendations,
            overall_status,
        }
    }

    fn determine_status(
        node: &ToolCheck,
        npm: &ToolCheck,
        eliza: &ToolCheck,
        recommendations: &mut Vec<String>,
    ) -> PreflightStatus {
        let mut critical_issues = 0;
        let mut needs_setup = 0;

        if !node.installed {
            critical_issues += 1;
            recommendations.push("Install Node.js 18+ from https://nodejs.org/".to_string());
        } else if let Some(ref version) = node.version {
            if !Self::is_node_version_compatible(version) {
                critical_issues += 1;
                recommendations.push("Update Node.js to version 18 or higher".to_string());
            }
        }

        if !npm.installed {
            needs_setup += 1;
            recommendations.push("Install npm (usually comes with Node.js)".to_string());
        }

        if !eliza.installed {
            needs_setup += 1;
            recommendations.push("ElizaOS CLI will be installed automatically via npx".to_string());
        }

        if critical_issues > 0 {
            PreflightStatus::CriticalIssues
        } else if needs_setup > 0 {
            PreflightStatus::NeedsSetup
        } else {
            PreflightStatus::Ready
        }
    }

    fn is_node_version_compatible(version: &str) -> bool {
        // Extract major version number
        version
            .split('.')
            .next()
            .and_then(|v| v.trim_start_matches('v').parse::<u32>().ok())
            .map_or(false, |major| major >= 18)
    }
}

// ============================================================================
// Telemetry Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryEvent {
    pub device_id: String,
    pub command: String,
    pub args: Vec<String>,
    pub started_at: String,
    pub duration_ms: u64,
    pub exit_code: i32,
    pub bytes_out: u64,
    pub approx_tokens: Option<u64>,
    pub error: Option<String>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl TelemetryEvent {
    pub fn new(
        device_id: String,
        command: String,
        args: Vec<String>,
        started_at: String,
        duration_ms: u64,
        exit_code: i32,
        bytes_out: u64,
    ) -> Self {
        Self {
            device_id,
            command,
            args,
            started_at,
            duration_ms,
            exit_code,
            bytes_out,
            approx_tokens: None,
            error: None,
            metadata: None,
        }
    }

    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    pub fn with_tokens(mut self, tokens: u64) -> Self {
        self.approx_tokens = Some(tokens);
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

// ============================================================================
// API Response Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<HashMap<String, serde_json::Value>>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(code: String, message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiError {
                code,
                message,
                details: None,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub metadata: Option<ConnectionMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMetadata {
    pub endpoint: String,
    pub timestamp: String,
    pub version: Option<String>,
}

// ============================================================================
// Error Models
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Process error: {0}")]
    Process(String),

    #[error("CLI not found: {0}")]
    CliNotFound(String),

    #[error("Environment setup failed: {0}")]
    EnvironmentError(String),

    #[error("Character file error: {0}")]
    CharacterError(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", &self.error_code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

impl AppError {
    pub fn error_code(&self) -> &'static str {
        match self {
            AppError::Config(_) => "CONFIG_ERROR",
            AppError::Process(_) => "PROCESS_ERROR",
            AppError::CliNotFound(_) => "CLI_NOT_FOUND",
            AppError::EnvironmentError(_) => "ENVIRONMENT_ERROR",
            AppError::CharacterError(_) => "CHARACTER_ERROR",
            AppError::Network(_) => "NETWORK_ERROR",
            AppError::Io(_) => "IO_ERROR",
            AppError::Serialization(_) => "SERIALIZATION_ERROR",
            AppError::Request(_) => "REQUEST_ERROR",
            AppError::Unknown(_) => "UNKNOWN_ERROR",
        }
    }
}

// ============================================================================
// Log Streaming Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub run_id: String,
    pub message: String,
    pub log_type: LogType,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogType {
    Stdout,
    Stderr,
    Info,
    Error,
    System,
}

impl LogEvent {
    pub fn new(run_id: String, message: String, log_type: LogType) -> Self {
        Self {
            run_id,
            message,
            log_type,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    pub fn stdout(run_id: String, message: String) -> Self {
        Self::new(run_id, message, LogType::Stdout)
    }

    pub fn stderr(run_id: String, message: String) -> Self {
        Self::new(run_id, message, LogType::Stderr)
    }

    pub fn info(run_id: String, message: String) -> Self {
        Self::new(run_id, message, LogType::Info)
    }

    pub fn error(run_id: String, message: String) -> Self {
        Self::new(run_id, message, LogType::Error)
    }

    pub fn system(run_id: String, message: String) -> Self {
        Self::new(run_id, message, LogType::System)
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

pub fn generate_device_id() -> String {
    use sha2::{Digest, Sha256};

    // Create a device ID based on hostname and other system info
    let hostname = hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let system_info = format!(
        "{}:{}:{}",
        hostname,
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    let mut hasher = Sha256::new();
    hasher.update(system_info.as_bytes());
    let result = hasher.finalize();

    format!("{:x}", result)[..16].to_string()
}

pub fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

// Note: All types are already pub and can be imported directly