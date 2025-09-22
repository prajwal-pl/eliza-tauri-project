/**
 * Core Type Definitions for MVP Tauri ElizaOS CLI
 * These types define the data structures used throughout the application
 * and match the Rust models for proper IPC communication.
 */

import { z } from 'zod';

// ============================================================================
// Configuration Types
// ============================================================================

export interface SandboxConfig {
  baseUrl: string;
  apiKey: string;
  projectId: string;
  defaultModel?: string;
}

const SandboxConfigSchema = z.object({
  baseUrl: z.string().url('Invalid base URL format'),
  apiKey: z.string().min(1, 'API key is required').regex(/^eliza_[a-f0-9]{64}$/, 'Invalid API key format'),
  projectId: z.string().min(1, 'Project ID is required'),
  defaultModel: z.string().optional(),
});

// ============================================================================
// Process Management Types
// ============================================================================

export type RunMode = 'doctor' | 'run' | 'eval' | 'custom';

export interface RunSpec {
  id: string;
  mode: RunMode;
  args: string[];
  env: Record<string, string>;
  workingDir?: string;
}

const RunSpecSchema = z.object({
  id: z.string(),
  mode: z.enum(['doctor', 'run', 'eval', 'custom']),
  args: z.array(z.string()),
  env: z.record(z.string()),
  workingDir: z.string().optional(),
});

export interface RunResult {
  id: string;
  spec: RunSpec;
  startedAt: Date;
  endedAt?: Date;
  exitCode?: number;
  stdout: string[];
  stderr: string[];
  durationMs?: number;
  status: 'running' | 'completed' | 'failed' | 'killed';
}

// ============================================================================
// Preflight Check Types
// ============================================================================

export interface ToolCheck {
  installed: boolean;
  version?: string;
  path?: string;
}

export interface PreflightResult {
  node: ToolCheck;
  npm: ToolCheck;
  eliza: ToolCheck;
  recommendations: string[];
  overall_status: 'ready' | 'needs_setup' | 'critical_issues';
}

// ============================================================================
// Telemetry Types
// ============================================================================

export interface TelemetryEvent {
  deviceId: string;
  command: string;
  args: string[];
  startedAt: string;
  durationMs: number;
  exitCode: number;
  bytesOut: number;
  approxTokens?: number;
  error?: string;
  metadata?: Record<string, unknown>;
}

const TelemetryEventSchema = z.object({
  deviceId: z.string(),
  command: z.string(),
  args: z.array(z.string()),
  startedAt: z.string(),
  durationMs: z.number(),
  exitCode: z.number(),
  bytesOut: z.number(),
  approxTokens: z.number().optional(),
  error: z.string().optional(),
  metadata: z.record(z.unknown()).optional(),
});

// ============================================================================
// UI State Types
// ============================================================================

export interface AppState {
  isConfigured: boolean;
  currentView: 'settings' | 'runner';
  isLoading: boolean;
  error?: string | null;
}

export interface LogEntry {
  id: string;
  timestamp: Date;
  type: 'stdout' | 'stderr' | 'system';
  content: string;
  source?: string;
}

// ============================================================================
// API Response Types
// ============================================================================

export interface ApiResponse<T = unknown> {
  success: boolean;
  data?: T;
  error?: {
    code: string;
    message: string;
    details?: Record<string, unknown>;
  };
}

export interface ConnectionTestResult {
  success: boolean;
  latencyMs?: number;
  error?: string;
  metadata?: {
    endpoint: string;
    timestamp: string;
    version?: string;
  };
}

// ============================================================================
// Security Types
// ============================================================================

export interface SecurityContext {
  hasApiKey: boolean;
  isKeyValid: boolean;
  lastValidated?: Date;
  permissions?: string[];
}

// ============================================================================
// Error Types
// ============================================================================

class AppError extends Error {
  public code: string;
  public context?: Record<string, unknown>;

  constructor(
    message: string,
    code: string = 'UNKNOWN_ERROR',
    context?: Record<string, unknown>
  ) {
    super(message);
    this.name = 'AppError';
    this.code = code;
    this.context = context;
  }
}

export interface ErrorDetails {
  code: string;
  message: string;
  timestamp: Date;
  context?: Record<string, unknown>;
  recoverable: boolean;
}

// ============================================================================
// Utility Types
// ============================================================================

export type DeepPartial<T> = {
  [P in keyof T]?: T[P] extends object ? DeepPartial<T[P]> : T[P];
};

export type RequiredFields<T, K extends keyof T> = T & Required<Pick<T, K>>;

export type OptionalFields<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>;

// ============================================================================
// Constants
// ============================================================================

const RUN_MODES = {
  doctor: 'doctor',
  run: 'run',
  eval: 'eval',
  custom: 'custom',
} as const;

const DEFAULT_SANDBOX_CONFIG: Partial<SandboxConfig> = {
  baseUrl: 'https://eliza-cloud-private-production.up.railway.app/api/v1',
  defaultModel: 'gpt-4o-mini',
} as const;

const ELIZA_COMMANDS = {
  doctor: ['doctor'],
  run: ['run', '-m', '${model}', '-p', '${prompt}'],
  eval: ['eval', '-f', '${file}'],
} as const;

// ============================================================================
// Type Guards
// ============================================================================

export function isSandboxConfig(value: unknown): value is SandboxConfig {
  try {
    SandboxConfigSchema.parse(value);
    return true;
  } catch {
    return false;
  }
}

export function isRunSpec(value: unknown): value is RunSpec {
  try {
    RunSpecSchema.parse(value);
    return true;
  } catch {
    return false;
  }
}

export function isAppError(error: unknown): error is AppError {
  return error instanceof AppError;
}

export function isApiResponse<T>(value: unknown): value is ApiResponse<T> {
  return (
    typeof value === 'object' &&
    value !== null &&
    'success' in value &&
    typeof (value as any).success === 'boolean'
  );
}

// ============================================================================
// Validation Helpers
// ============================================================================

export function validateSandboxConfig(config: unknown): SandboxConfig {
  return SandboxConfigSchema.parse(config);
}

export function validateRunSpec(spec: unknown): RunSpec {
  return RunSpecSchema.parse(spec);
}

export function validateTelemetryEvent(event: unknown): TelemetryEvent {
  return TelemetryEventSchema.parse(event);
}

// ============================================================================
// Export all types and utilities
// ============================================================================

export {
  // Schemas
  SandboxConfigSchema,
  RunSpecSchema,
  TelemetryEventSchema,

  // Classes
  AppError,

  // Constants
  RUN_MODES,
  DEFAULT_SANDBOX_CONFIG,
  ELIZA_COMMANDS,
};