/**
 * App Store - Manages overall application state and coordination
 */

import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import type { AppState } from '../types';

interface AppStoreState extends AppState {
  // Navigation state
  currentView: 'settings' | 'runner' | 'terminal';
  sidebarCollapsed: boolean;

  // Global UI state
  isLoading: boolean;
  error: string | null;
  notifications: Notification[];

  // Application metadata
  version: string;
  isFirstLaunch: boolean;

  // Actions
  setCurrentView: (view: 'settings' | 'runner' | 'terminal') => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  clearError: () => void;
  addNotification: (notification: Omit<Notification, 'id' | 'timestamp'>) => void;
  removeNotification: (id: string) => void;
  clearNotifications: () => void;
  setIsConfigured: (configured: boolean) => void;
  setFirstLaunch: (isFirst: boolean) => void;
  initialize: () => Promise<void>;
}

interface Notification {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  title: string;
  message?: string;
  timestamp: Date;
  duration?: number; // Auto-dismiss after N ms
  action?: {
    label: string;
    onClick: () => void;
  };
}

export const useAppStore = create<AppStoreState>()(
  devtools(
    (set, get) => ({
      // Initial state
      currentView: 'settings',
      sidebarCollapsed: false,
      isConfigured: false,
      isLoading: false,
      error: null,
      notifications: [],
      version: '0.1.0',
      isFirstLaunch: true,

      // Set current view/page
      setCurrentView: (view: 'settings' | 'runner' | 'terminal') => {
        set({ currentView: view });
      },

      // Toggle sidebar collapsed state
      setSidebarCollapsed: (collapsed: boolean) => {
        set({ sidebarCollapsed: collapsed });
      },

      // Set global loading state
      setLoading: (loading: boolean) => {
        set({ isLoading: loading });
      },

      // Set global error state
      setError: (error: string | null) => {
        set({ error });
      },

      // Clear global error
      clearError: () => {
        set({ error: null });
      },

      // Add a notification
      addNotification: (notification: Omit<Notification, 'id' | 'timestamp'>) => {
        const newNotification: Notification = {
          ...notification,
          id: `notif_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
          timestamp: new Date(),
        };

        set(state => ({
          notifications: [...state.notifications, newNotification],
        }));

        // Auto-remove notification after duration
        if (notification.duration && notification.duration > 0) {
          setTimeout(() => {
            get().removeNotification(newNotification.id);
          }, notification.duration);
        }
      },

      // Remove a specific notification
      removeNotification: (id: string) => {
        set(state => ({
          notifications: state.notifications.filter(n => n.id !== id),
        }));
      },

      // Clear all notifications
      clearNotifications: () => {
        set({ notifications: [] });
      },

      // Update configuration status
      setIsConfigured: (configured: boolean) => {
        set({ isConfigured: configured });

        // Navigate to runner if just configured
        if (configured && get().currentView === 'settings' && !get().isFirstLaunch) {
          set({ currentView: 'runner' });
        }
      },

      // Set first launch status
      setFirstLaunch: (isFirst: boolean) => {
        set({ isFirstLaunch: isFirst });
      },

      // Initialize application
      initialize: async () => {
        set({ isLoading: true, error: null });

        try {
          // Check if this is first launch by looking for existing config
          // This would integrate with the config store
          const hasExistingConfig = false; // Placeholder

          set({
            isFirstLaunch: !hasExistingConfig,
            currentView: hasExistingConfig ? 'runner' : 'settings',
            isLoading: false,
          });

          // Show welcome notification on first launch
          if (!hasExistingConfig) {
            get().addNotification({
              type: 'info',
              title: 'Welcome to ElizaOS CLI Desktop',
              message: 'Please configure your Sandbox settings to get started.',
              duration: 5000,
            });
          }
        } catch (error) {
          console.error('Error initializing app:', error);
          set({
            isLoading: false,
            error: error instanceof Error ? error.message : 'Failed to initialize application',
          });
        }
      },
    }),
    {
      name: 'app-store',
      enabled: typeof window !== 'undefined' && (window as any).__DEV__ === true,
    }
  )
);

// Selectors and utility hooks
export const useCurrentView = () => {
  const { currentView } = useAppStore();
  return currentView;
};

export const useIsReady = () => {
  const { isConfigured, isLoading } = useAppStore();
  return isConfigured && !isLoading;
};

export const useNotifications = () => {
  const { notifications, removeNotification, clearNotifications } = useAppStore();
  return {
    notifications,
    removeNotification,
    clearNotifications,
  };
};

// Notification helper functions
export const useNotify = () => {
  const { addNotification } = useAppStore();

  return {
    success: (title: string, message?: string, duration = 4000) => {
      addNotification({ type: 'success', title, message, duration });
    },

    error: (title: string, message?: string, duration = 6000) => {
      addNotification({ type: 'error', title, message, duration });
    },

    warning: (title: string, message?: string, duration = 5000) => {
      addNotification({ type: 'warning', title, message, duration });
    },

    info: (title: string, message?: string, duration = 4000) => {
      addNotification({ type: 'info', title, message, duration });
    },

    persistent: (type: 'info' | 'success' | 'warning' | 'error', title: string, message?: string) => {
      addNotification({ type, title, message }); // No duration = persistent
    },
  };
};

// Global error boundary integration
export const useGlobalError = () => {
  const { error, setError, clearError } = useAppStore();
  const notify = useNotify();

  const handleError = (error: unknown, context?: string) => {
    const message = error instanceof Error ? error.message : 'An unexpected error occurred';
    const fullMessage = context ? `${context}: ${message}` : message;

    setError(fullMessage);
    notify.error('Error', fullMessage);

    console.error('Global error:', error, context);
  };

  return {
    error,
    clearError,
    handleError,
  };
};