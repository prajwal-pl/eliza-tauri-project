/**
 * Configuration Store - Manages Sandbox configuration and connection state
 */

import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import { invoke } from '@tauri-apps/api/core';
import type {
  SandboxConfig,
  PreflightResult,
  ConnectionTestResult,
  ApiResponse,
} from '../types';
import { validateSandboxConfig, AppError } from '../types';

interface ConfigState {
  // Configuration state
  sandboxConfig: SandboxConfig | null;
  preflightResult: PreflightResult | null;
  connectionTest: ConnectionTestResult | null;

  // UI state
  isConfigured: boolean;
  isLoading: boolean;
  isTesting: boolean;
  error: string | null;

  // Actions
  loadConfig: () => Promise<void>;
  saveConfig: (config: SandboxConfig) => Promise<void>;
  testConnection: () => Promise<boolean>;
  runPreflightCheck: () => Promise<void>;
  clearError: () => void;
  resetConfig: () => Promise<void>;
}

export const useConfigStore = create<ConfigState>()(
  devtools(
    (set, get) => ({
      // Initial state
      sandboxConfig: null,
      preflightResult: null,
      connectionTest: null,
      isConfigured: false,
      isLoading: false,
      isTesting: false,
      error: null,

      // Load configuration from secure storage
      loadConfig: async () => {
        set({ isLoading: true, error: null });

        try {
          const response = await invoke<ApiResponse<SandboxConfig>>('load_sandbox_config');

          if (response.success && response.data) {
            const config = validateSandboxConfig(response.data);
            set({
              sandboxConfig: config,
              isConfigured: true,
              isLoading: false,
            });
          } else {
            set({
              sandboxConfig: null,
              isConfigured: false,
              isLoading: false,
              error: response.error?.message || 'Failed to load configuration',
            });
          }
        } catch (error) {
          console.error('Error loading config:', error);
          set({
            sandboxConfig: null,
            isConfigured: false,
            isLoading: false,
            error: error instanceof Error ? error.message : 'Failed to load configuration',
          });
        }
      },

      // Save configuration to secure storage
      saveConfig: async (config: SandboxConfig) => {
        set({ isLoading: true, error: null });

        try {
          // Validate configuration before saving
          const validatedConfig = validateSandboxConfig(config);

          const response = await invoke<ApiResponse<void>>('save_sandbox_config', {
            config: validatedConfig,
          });

          if (response.success) {
            set({
              sandboxConfig: validatedConfig,
              isConfigured: true,
              isLoading: false,
              error: null,
            });
          } else {
            throw new AppError(
              response.error?.message || 'Failed to save configuration',
              response.error?.code || 'SAVE_ERROR'
            );
          }
        } catch (error) {
          console.error('Error saving config:', error);
          const errorMessage = error instanceof Error ? error.message : 'Failed to save configuration';
          set({
            isLoading: false,
            error: errorMessage,
          });
          throw error;
        }
      },

      // Test connection to Sandbox API
      testConnection: async () => {
        const { sandboxConfig } = get();
        if (!sandboxConfig) {
          set({ error: 'No configuration available to test' });
          return false;
        }

        set({ isTesting: true, error: null, connectionTest: null });

        try {
          const response = await invoke<ApiResponse<ConnectionTestResult>>('test_sandbox_connection', {
            config: sandboxConfig,
          });

          if (response.success && response.data) {
            set({
              connectionTest: response.data,
              isTesting: false,
              error: response.data.success ? null : response.data.error || 'Connection test failed',
            });
            return response.data.success;
          } else {
            const errorMessage = response.error?.message || 'Connection test failed';
            set({
              connectionTest: {
                success: false,
                error: errorMessage,
              },
              isTesting: false,
              error: errorMessage,
            });
            return false;
          }
        } catch (error) {
          console.error('Error testing connection:', error);
          const errorMessage = error instanceof Error ? error.message : 'Connection test failed';
          set({
            connectionTest: {
              success: false,
              error: errorMessage,
            },
            isTesting: false,
            error: errorMessage,
          });
          return false;
        }
      },

      // Run preflight check for system requirements
      runPreflightCheck: async () => {
        set({ isLoading: true, error: null });

        try {
          const response = await invoke<ApiResponse<PreflightResult>>('preflight_check');

          if (response.success && response.data) {
            set({
              preflightResult: response.data,
              isLoading: false,
            });
          } else {
            throw new AppError(
              response.error?.message || 'Preflight check failed',
              response.error?.code || 'PREFLIGHT_ERROR'
            );
          }
        } catch (error) {
          console.error('Error running preflight check:', error);
          set({
            preflightResult: null,
            isLoading: false,
            error: error instanceof Error ? error.message : 'Preflight check failed',
          });
        }
      },

      // Clear any error state
      clearError: () => {
        set({ error: null });
      },

      // Reset configuration (clear all data)
      resetConfig: async () => {
        set({ isLoading: true, error: null });

        try {
          await invoke<ApiResponse<void>>('clear_sandbox_config');
          set({
            sandboxConfig: null,
            preflightResult: null,
            connectionTest: null,
            isConfigured: false,
            isLoading: false,
            error: null,
          });
        } catch (error) {
          console.error('Error resetting config:', error);
          set({
            isLoading: false,
            error: error instanceof Error ? error.message : 'Failed to reset configuration',
          });
        }
      },
    }),
    {
      name: 'config-store',
      enabled: typeof window !== 'undefined' && (window as any).__DEV__ === true,
    }
  )
);

// Selectors for common state combinations
export const useIsReady = () => {
  const { isConfigured, preflightResult } = useConfigStore();
  return isConfigured && preflightResult?.overallStatus === 'ready';
};

export const useHasConfiguration = () => {
  const { sandboxConfig } = useConfigStore();
  return sandboxConfig !== null;
};

export const useConfigurationStatus = () => {
  const { isConfigured, preflightResult, connectionTest, isLoading } = useConfigStore();

  if (isLoading) return 'loading';
  if (!isConfigured) return 'not_configured';
  if (!preflightResult) return 'checking_requirements';
  if (preflightResult.overallStatus === 'critical_issues') return 'critical_issues';
  if (preflightResult.overallStatus === 'needs_setup') return 'needs_setup';
  if (!connectionTest) return 'needs_connection_test';
  if (!connectionTest.success) return 'connection_failed';

  return 'ready';
};