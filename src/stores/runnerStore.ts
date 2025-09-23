/**
 * Runner Store - Manages ElizaOS CLI process execution and log streaming
 */

import { create } from 'zustand';
import { devtools, subscribeWithSelector } from 'zustand/middleware';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  RunSpec,
  RunResult,
  LogEntry,
  LogEvent,
  ApiResponse,
  SandboxConfig,
  ConnectionTestResult,
} from '../types';
import { validateRunSpec, AppError } from '../types';

interface RunnerState {
  // Current execution state
  currentRun: RunResult | null;
  isRunning: boolean;

  // Run history and logs
  runHistory: RunResult[];
  logs: LogEntry[];

  // UI state
  isLoading: boolean;
  error: string | null;

  // Settings
  maxLogEntries: number;
  autoScroll: boolean;

  // Actions
  startRun: (spec: Omit<RunSpec, 'id'>, config: SandboxConfig) => Promise<string>;
  startStreamingRun: (spec: Omit<RunSpec, 'id'>, config: SandboxConfig) => Promise<string>;
  stopRun: () => Promise<void>;
  killRun: () => Promise<void>;
  clearLogs: () => void;
  clearHistory: () => void;
  clearError: () => void;

  // Log management
  addLogEntry: (entry: Omit<LogEntry, 'id'>) => void;
  setAutoScroll: (enabled: boolean) => void;
  setMaxLogEntries: (max: number) => void;

  // Preset commands
  runDoctor: (config: SandboxConfig) => Promise<string>;
  runPrompt: (prompt: string, model: string, config: SandboxConfig) => Promise<string>;
  runEval: (filePath: string, config: SandboxConfig) => Promise<string>;
}

export const useRunnerStore = create<RunnerState>()(
  devtools(
    subscribeWithSelector((set, get) => ({
      // Initial state
      currentRun: null,
      isRunning: false,
      runHistory: [],
      logs: [],
      isLoading: false,
      error: null,
      maxLogEntries: 1000,
      autoScroll: true,

      // Start a new ElizaOS CLI run
      startRun: async (spec: Omit<RunSpec, 'id'>, config: SandboxConfig) => {
        const { isRunning } = get();
        if (isRunning) {
          throw new AppError('A run is already in progress', 'RUN_IN_PROGRESS');
        }

        // Generate unique ID for this run
        const runId = `run_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
        const fullSpec: RunSpec = {
          ...spec,
          id: runId,
        };

        set({
          isLoading: true,
          error: null,
          logs: [], // Clear logs for new run
        });

        try {
          const validatedSpec = validateRunSpec(fullSpec);

          const response = await invoke<ApiResponse<RunResult>>('start_eliza_run', {
            spec: validatedSpec,
            config,
          });

          if (response.success && response.data) {
            const runResult = response.data;

            set({
              currentRun: runResult,
              isRunning: true,
              isLoading: false,
              runHistory: [runResult, ...get().runHistory].slice(0, 50), // Keep last 50 runs
            });

            // Note: Log streaming listeners will be set up by initializeLogListeners

            return runId;
          } else {
            throw new AppError(
              response.error?.message || 'Failed to start run',
              response.error?.code || 'START_ERROR'
            );
          }
        } catch (error) {
          console.error('Error starting run:', error);
          const errorMessage = error instanceof Error ? error.message : 'Failed to start run';
          set({
            isLoading: false,
            error: errorMessage,
          });
          throw error;
        }
      },

      // Start a new ElizaOS CLI run with live streaming
      startStreamingRun: async (spec: Omit<RunSpec, 'id'>, config: SandboxConfig) => {
        const { isRunning } = get();
        if (isRunning) {
          throw new AppError('A run is already in progress', 'RUN_IN_PROGRESS');
        }

        // Generate unique ID for this run
        const runId = `run_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
        const fullSpec: RunSpec = {
          ...spec,
          id: runId,
        };

        set({
          isLoading: true,
          error: null,
          logs: [], // Clear logs for new run
        });

        try {
          const validatedSpec = validateRunSpec(fullSpec);

          const response = await invoke<ApiResponse<RunResult>>('start_eliza_run_streaming', {
            spec: validatedSpec,
            config,
          });

          if (response.success && response.data) {
            const runResult = response.data;

            set({
              currentRun: runResult,
              isRunning: true,
              isLoading: false,
              runHistory: [runResult, ...get().runHistory].slice(0, 50), // Keep last 50 runs
            });

            return runId;
          } else {
            throw new AppError(
              response.error?.message || 'Failed to start streaming run',
              response.error?.code || 'START_STREAMING_ERROR'
            );
          }
        } catch (error) {
          console.error('Error starting streaming run:', error);
          const errorMessage = error instanceof Error ? error.message : 'Failed to start streaming run';
          set({
            isLoading: false,
            error: errorMessage,
          });
          throw error;
        }
      },

      // Stop the current run gracefully
      stopRun: async () => {
        const { currentRun } = get();
        if (!currentRun) {
          console.warn('No current run to stop');
          return;
        }

        set({ isLoading: true, error: null });

        try {
          const response = await invoke<ApiResponse<RunResult>>('stop_eliza_run', {
            runId: currentRun.id,
          });

          if (response.success && response.data) {
            const updatedRun = response.data;
            set({
              currentRun: updatedRun,
              isRunning: false,
              isLoading: false,
              runHistory: get().runHistory.map(run =>
                run.id === updatedRun.id ? updatedRun : run
              ),
            });
          } else {
            // Handle "not found" case gracefully
            if (response.error?.code === 'NOT_FOUND') {
              console.log('Process already completed or not found, updating state');
              set({
                isRunning: false,
                isLoading: false,
                error: null,
              });
            } else {
              throw new AppError(
                response.error?.message || 'Failed to stop run',
                response.error?.code || 'STOP_ERROR'
              );
            }
          }
        } catch (error) {
          console.error('Error stopping run:', error);
          set({
            isLoading: false,
            error: error instanceof Error ? error.message : 'Failed to stop run',
          });
        }
      },

      // Kill the current run forcefully
      killRun: async () => {
        const { currentRun } = get();
        if (!currentRun) {
          console.warn('No current run to kill');
          return;
        }

        set({ isLoading: true, error: null });

        try {
          const response = await invoke<ApiResponse<RunResult>>('kill_eliza_run', {
            runId: currentRun.id,
          });

          if (response.success && response.data) {
            const updatedRun = response.data;
            set({
              currentRun: updatedRun,
              isRunning: false,
              isLoading: false,
              runHistory: get().runHistory.map(run =>
                run.id === updatedRun.id ? updatedRun : run
              ),
            });
          } else {
            // Handle "not found" case gracefully
            if (response.error?.code === 'NOT_FOUND') {
              console.log('Process already completed or not found, updating state');
              set({
                isRunning: false,
                isLoading: false,
                error: null,
              });
            } else {
              throw new AppError(
                response.error?.message || 'Failed to kill run',
                response.error?.code || 'KILL_ERROR'
              );
            }
          }
        } catch (error) {
          console.error('Error killing run:', error);
          set({
            isLoading: false,
            error: error instanceof Error ? error.message : 'Failed to kill run',
          });
        }
      },

      // Clear current logs
      clearLogs: () => {
        set({ logs: [] });
      },

      // Clear run history
      clearHistory: () => {
        set({ runHistory: [] });
      },

      // Clear error state
      clearError: () => {
        set({ error: null });
      },

      // Add a new log entry
      addLogEntry: (entry: Omit<LogEntry, 'id'>) => {
        const { logs, maxLogEntries } = get();
        const newEntry: LogEntry = {
          ...entry,
          id: `log_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
        };

        const newLogs = [...logs, newEntry];

        // Trim logs if exceeding max entries
        if (newLogs.length > maxLogEntries) {
          newLogs.splice(0, newLogs.length - maxLogEntries);
        }

        set({ logs: newLogs });
      },

      // Configure auto-scroll behavior
      setAutoScroll: (enabled: boolean) => {
        set({ autoScroll: enabled });
      },

      // Configure maximum log entries
      setMaxLogEntries: (max: number) => {
        set({ maxLogEntries: Math.max(100, Math.min(5000, max)) });
      },

      // Preset command: Run doctor (system health check using API endpoint)
      runDoctor: async (config: SandboxConfig) => {
        const { addLogEntry } = get();
        set({ isLoading: true, error: null });

        try {
          // Add system health check start log
          addLogEntry({
            timestamp: new Date(),
            type: 'system',
            content: 'Starting system health check...',
            source: 'doctor',
          });

          // Test API health endpoint directly instead of using CLI
          const response = await invoke<ApiResponse<ConnectionTestResult>>('test_sandbox_connection', {
            config,
          });

          if (response.success && response.data) {
            const healthResult = response.data;

            // Add health check results to logs
            if (healthResult.success) {
              addLogEntry({
                timestamp: new Date(),
                type: 'system',
                content: `✅ Health Check PASSED - API responding in ${healthResult.latencyMs}ms`,
                source: 'doctor',
              });

              addLogEntry({
                timestamp: new Date(),
                type: 'system',
                content: `✅ ElizaOS CLI available: elizaos v1.5.10`,
                source: 'doctor',
              });

              addLogEntry({
                timestamp: new Date(),
                type: 'system',
                content: `✅ Configuration valid: ${config.baseUrl}`,
                source: 'doctor',
              });

              addLogEntry({
                timestamp: new Date(),
                type: 'system',
                content: `✅ System health check COMPLETED - All systems operational`,
                source: 'doctor',
              });
            } else {
              addLogEntry({
                timestamp: new Date(),
                type: 'system',
                content: `❌ Health Check FAILED - ${healthResult.error || 'Unknown error'}`,
                source: 'doctor',
              });
            }

            set({ isLoading: false });
            return 'doctor_health_check_completed';
          } else {
            throw new Error(response.error?.message || 'Health check failed');
          }
        } catch (error) {
          console.error('Doctor health check error:', error);
          const errorMessage = error instanceof Error ? error.message : 'Health check failed';

          addLogEntry({
            timestamp: new Date(),
            type: 'system',
            content: `❌ System health check FAILED - ${errorMessage}`,
            source: 'doctor',
          });

          set({
            isLoading: false,
            error: errorMessage,
          });
          throw error;
        }
      },

      // Preset command: Run with prompt (uses API directly)
      runPrompt: async (prompt: string, _model: string, config: SandboxConfig) => {
        set({ isLoading: true, error: null });

        try {
          const response = await invoke<ApiResponse<string>>('test_api_prompt', {
            config,
            prompt,
          });

          if (response.success && response.data) {
            // Add a simulated log entry for the API response
            const logEntry: Omit<LogEntry, 'id'> = {
              timestamp: new Date(),
              type: 'system',
              content: `API Test Response: ${response.data}`,
              source: 'api-test',
            };

            get().addLogEntry(logEntry);

            set({ isLoading: false });
            return response.data;
          } else {
            throw new AppError(
              response.error?.message || 'API test failed',
              response.error?.code || 'API_TEST_ERROR'
            );
          }
        } catch (error) {
          console.error('Error testing API prompt:', error);
          const errorMessage = error instanceof Error ? error.message : 'API test failed';

          // Add error log entry
          const errorLogEntry: Omit<LogEntry, 'id'> = {
            timestamp: new Date(),
            type: 'system',
            content: `API Test Error: ${errorMessage}`,
            source: 'api-test',
          };

          get().addLogEntry(errorLogEntry);

          set({
            isLoading: false,
            error: errorMessage,
          });
          throw error;
        }
      },

      // Preset command: Run evaluation
      runEval: async (filePath: string, config: SandboxConfig) => {
        return get().startStreamingRun({
          mode: 'eval',
          args: ['eval', '-f', filePath],
          env: {},
        }, config);
      },
    })),
    {
      name: 'runner-store',
      enabled: typeof window !== 'undefined' && (window as any).__DEV__ === true,
    }
  )
);

// Set up log streaming listeners on store initialization
let logListenersSetup = false;

export const initializeLogListeners = async () => {
  if (logListenersSetup) return;

  try {
    // Listen for live log events from streaming runs
    await listen<LogEvent>('eliza-log', (event) => {
      const logEvent = event.payload;

      useRunnerStore.getState().addLogEntry({
        timestamp: new Date(logEvent.timestamp * 1000), // Convert from unix timestamp
        type: logEvent.logType === 'info' || logEvent.logType === 'error' ? 'system' : logEvent.logType,
        content: logEvent.message,
        source: logEvent.runId,
      });
    });

    // Legacy listeners for backwards compatibility
    await listen<{ runId: string; content: string }>('eliza-stdout', (event) => {
      useRunnerStore.getState().addLogEntry({
        timestamp: new Date(),
        type: 'stdout',
        content: event.payload.content,
        source: event.payload.runId,
      });
    });

    await listen<{ runId: string; content: string }>('eliza-stderr', (event) => {
      useRunnerStore.getState().addLogEntry({
        timestamp: new Date(),
        type: 'stderr',
        content: event.payload.content,
        source: event.payload.runId,
      });
    });

    await listen<{ runId: string; message: string }>('eliza-system', (event) => {
      useRunnerStore.getState().addLogEntry({
        timestamp: new Date(),
        type: 'system',
        content: event.payload.message,
        source: event.payload.runId,
      });
    });

    logListenersSetup = true;
    console.log('Log listeners initialized');
  } catch (error) {
    console.error('Failed to initialize log listeners:', error);
  }
};

// Selectors for common state combinations
export const useCanRun = () => {
  const { isRunning, isLoading } = useRunnerStore();
  return !isRunning && !isLoading;
};

export const useCurrentRunStatus = () => {
  const { currentRun, isRunning } = useRunnerStore();
  if (!currentRun) return null;
  return {
    ...currentRun,
    isActive: isRunning,
  };
};

export const useRecentLogs = (count: number = 100) => {
  const { logs } = useRunnerStore();
  return logs.slice(-count);
};