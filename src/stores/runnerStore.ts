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
  ApiResponse,
  SandboxConfig,
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

      // Stop the current run gracefully
      stopRun: async () => {
        const { currentRun } = get();
        if (!currentRun || !get().isRunning) {
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
            throw new AppError(
              response.error?.message || 'Failed to stop run',
              response.error?.code || 'STOP_ERROR'
            );
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
        if (!currentRun || !get().isRunning) {
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
            throw new AppError(
              response.error?.message || 'Failed to kill run',
              response.error?.code || 'KILL_ERROR'
            );
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

      // Preset command: Run doctor
      runDoctor: async (config: SandboxConfig) => {
        return get().startRun({
          mode: 'doctor',
          args: ['doctor'],
          env: {},
        }, config);
      },

      // Preset command: Run with prompt
      runPrompt: async (prompt: string, model: string, config: SandboxConfig) => {
        return get().startRun({
          mode: 'run',
          args: ['run', '-m', model, '-p', prompt],
          env: {},
        }, config);
      },

      // Preset command: Run evaluation
      runEval: async (filePath: string, config: SandboxConfig) => {
        return get().startRun({
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
    // Listen for stdout events
    await listen<{ runId: string; content: string }>('eliza-stdout', (event) => {
      useRunnerStore.getState().addLogEntry({
        timestamp: new Date(),
        type: 'stdout',
        content: event.payload.content,
        source: event.payload.runId,
      });
    });

    // Listen for stderr events
    await listen<{ runId: string; content: string }>('eliza-stderr', (event) => {
      useRunnerStore.getState().addLogEntry({
        timestamp: new Date(),
        type: 'stderr',
        content: event.payload.content,
        source: event.payload.runId,
      });
    });

    // Listen for system events
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