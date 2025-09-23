/**
 * Terminal Store - Manages terminal sessions, commands, and real-time output
 */

import { create } from 'zustand';
import { devtools, subscribeWithSelector } from 'zustand/middleware';
import type { TerminalSession, TerminalCommand } from '../types';
import { invoke } from '@tauri-apps/api/core';

interface TerminalStoreState {
  // Terminal sessions
  sessions: TerminalSession[];
  activeSessionId: string | null;

  // Current command state
  currentCommand: string;
  commandHistory: string[];
  historyIndex: number;

  // Terminal configuration
  workingDirectory: string;
  fontSize: number;
  theme: 'dark' | 'light';
  maxOutputLines: number;

  // UI state
  isExecuting: boolean;
  isConnected: boolean;
  error: string | null;

  // Actions - Session Management
  createSession: (title?: string) => string;
  closeSession: (sessionId: string) => void;
  setActiveSession: (sessionId: string) => void;

  // Actions - Command Management
  setCurrentCommand: (command: string) => void;
  executeCommand: (command: string, sessionId?: string) => Promise<void>;
  cancelCommand: (commandId: string) => Promise<void>;
  clearSession: (sessionId: string) => void;
  cleanupOldCommands: (sessionId?: string) => void;

  // Actions - History Management
  navigateHistory: (direction: 'up' | 'down') => void;
  addToHistory: (command: string) => void;
  clearHistory: () => void;

  // Actions - Configuration
  setWorkingDirectory: (path: string) => void;
  setFontSize: (size: number) => void;
  setTheme: (theme: 'dark' | 'light') => void;
  setMaxOutputLines: (lines: number) => void;

  // Actions - Connection Management
  connect: () => Promise<void>;
  disconnect: () => void;

  // Actions - Utilities
  clearError: () => void;
  getActiveSession: () => TerminalSession | null;
  getSessionCommands: (sessionId: string) => TerminalCommand[];
}

// Periodic cleanup to prevent memory leaks
let cleanupInterval: NodeJS.Timeout | null = null;

export const useTerminalStore = create<TerminalStoreState>()(
  devtools(
    subscribeWithSelector(
      (set, get) => ({
        // Initial state
        sessions: [],
        activeSessionId: null,
        currentCommand: '',
        commandHistory: [],
        historyIndex: -1,
        workingDirectory: '/', // Will be set from backend on connect
        fontSize: 14,
        theme: 'dark',
        maxOutputLines: 1000,
        isExecuting: false,
        isConnected: false,
        error: null,

        // Create a new terminal session
        createSession: (title?: string) => {
          const sessionId = `session_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
          const newSession: TerminalSession = {
            id: sessionId,
            title: title || `Terminal ${get().sessions.length + 1}`,
            startedAt: new Date(),
            workingDirectory: get().workingDirectory,
            commands: [],
            isActive: true,
          };

          set(state => ({
            sessions: [...state.sessions, newSession],
            activeSessionId: sessionId,
          }));

          return sessionId;
        },

        // Close a terminal session
        closeSession: (sessionId: string) => {
          set(state => {
            const filteredSessions = state.sessions.filter(s => s.id !== sessionId);
            const newActiveId = state.activeSessionId === sessionId
              ? (filteredSessions.length > 0 ? filteredSessions[0].id : null)
              : state.activeSessionId;

            return {
              sessions: filteredSessions,
              activeSessionId: newActiveId,
            };
          });
        },

        // Set the active session
        setActiveSession: (sessionId: string) => {
          const session = get().sessions.find(s => s.id === sessionId);
          if (session) {
            set({
              activeSessionId: sessionId,
              workingDirectory: session.workingDirectory,
            });
          }
        },

        // Set current command text
        setCurrentCommand: (command: string) => {
          set({ currentCommand: command });
        },

        // Execute a terminal command
        executeCommand: async (command: string, sessionId?: string) => {
          const targetSessionId = sessionId || get().activeSessionId;
          if (!targetSessionId) {
            set({ error: 'No active terminal session' });
            return;
          }

          const commandId = `cmd_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
          const newCommand: TerminalCommand = {
            id: commandId,
            command: command.split(' ')[0],
            args: command.split(' ').slice(1),
            timestamp: new Date(),
            status: 'pending',
            output: [],
          };

          // Add command to session and update UI state
          set(state => ({
            sessions: state.sessions.map(session =>
              session.id === targetSessionId
                ? { ...session, commands: [...session.commands, newCommand] }
                : session
            ),
            isExecuting: true,
            currentCommand: '',
            error: null,
          }));

          // Add to command history
          get().addToHistory(command);

          try {
            // Update command status to running
            set(state => ({
              sessions: state.sessions.map(session =>
                session.id === targetSessionId
                  ? {
                      ...session,
                      commands: session.commands.map(cmd =>
                        cmd.id === commandId
                          ? { ...cmd, status: 'running' as const }
                          : cmd
                      )
                    }
                  : session
              ),
            }));

            // Execute command via Tauri backend
            const result = await invoke<{
              success: boolean;
              output: string[];
              error?: string;
              exitCode?: number;
            }>('execute_terminal_command', {
              command: newCommand.command,
              args: newCommand.args,
              workingDir: get().workingDirectory,
            });

            // Update command with results (truncate large output)
            const { maxOutputLines } = get();
            const truncatedOutput = result.output.length > maxOutputLines
              ? [...result.output.slice(0, maxOutputLines), `... (${result.output.length - maxOutputLines} more lines truncated)`]
              : result.output;

            set(state => ({
              sessions: state.sessions.map(session =>
                session.id === targetSessionId
                  ? {
                      ...session,
                      commands: session.commands.map(cmd =>
                        cmd.id === commandId
                          ? {
                              ...cmd,
                              status: result.success ? 'completed' as const : 'failed' as const,
                              output: truncatedOutput,
                              error: result.error,
                              exitCode: result.exitCode,
                            }
                          : cmd
                      )
                    }
                  : session
              ),
              isExecuting: false,
            }));

            // Auto-cleanup if session has too many commands
            const session = get().sessions.find(s => s.id === targetSessionId);
            if (session && session.commands.length > 75) {
              get().cleanupOldCommands(targetSessionId);
            }

          } catch (error) {
            // Handle execution error
            const errorMessage = error instanceof Error ? error.message : 'Command execution failed';

            set(state => ({
              sessions: state.sessions.map(session =>
                session.id === targetSessionId
                  ? {
                      ...session,
                      commands: session.commands.map(cmd =>
                        cmd.id === commandId
                          ? {
                              ...cmd,
                              status: 'failed' as const,
                              error: errorMessage,
                              exitCode: 1,
                            }
                          : cmd
                      )
                    }
                  : session
              ),
              isExecuting: false,
              error: `Command execution failed: ${errorMessage}`,
            }));
          }
        },

        // Cancel a running command
        cancelCommand: async (commandId: string) => {
          try {
            await invoke('cancel_terminal_command', { commandId });

            set(state => ({
              sessions: state.sessions.map(session => ({
                ...session,
                commands: session.commands.map(cmd =>
                  cmd.id === commandId
                    ? { ...cmd, status: 'failed' as const, error: 'Cancelled by user' }
                    : cmd
                )
              })),
              isExecuting: false,
            }));
          } catch (error) {
            console.error('Failed to cancel command:', error);
          }
        },

        // Clear all commands in a session
        clearSession: (sessionId: string) => {
          set(state => ({
            sessions: state.sessions.map(session =>
              session.id === sessionId
                ? { ...session, commands: [] }
                : session
            ),
          }));
        },

        // Clean up old commands to prevent memory leaks
        cleanupOldCommands: (sessionId?: string) => {
          const { maxOutputLines } = get();
          set(state => ({
            sessions: state.sessions.map(session => {
              // Only cleanup specified session or all sessions if none specified
              if (sessionId && session.id !== sessionId) return session;

              const commands = session.commands;
              if (commands.length <= 50) return session; // Keep reasonable number of commands

              // Keep only the last 50 commands and truncate their output
              const recentCommands = commands.slice(-50).map(cmd => ({
                ...cmd,
                output: cmd.output.slice(-Math.min(maxOutputLines, 200)) // Limit output lines
              }));

              return { ...session, commands: recentCommands };
            })
          }));
        },

        // Navigate command history
        navigateHistory: (direction: 'up' | 'down') => {
          const { commandHistory, historyIndex } = get();

          let newIndex = historyIndex;
          if (direction === 'up' && historyIndex < commandHistory.length - 1) {
            newIndex = historyIndex + 1;
          } else if (direction === 'down' && historyIndex > -1) {
            newIndex = historyIndex - 1;
          }

          const command = newIndex === -1 ? '' : commandHistory[commandHistory.length - 1 - newIndex];

          set({
            historyIndex: newIndex,
            currentCommand: command,
          });
        },

        // Add command to history
        addToHistory: (command: string) => {
          const trimmedCommand = command.trim();
          if (!trimmedCommand) return;

          set(state => {
            const newHistory = [...state.commandHistory];

            // Remove duplicate if it exists
            const existingIndex = newHistory.indexOf(trimmedCommand);
            if (existingIndex !== -1) {
              newHistory.splice(existingIndex, 1);
            }

            // Add to end and limit size
            newHistory.push(trimmedCommand);
            if (newHistory.length > 100) {
              newHistory.shift();
            }

            return {
              commandHistory: newHistory,
              historyIndex: -1,
            };
          });
        },

        // Clear command history
        clearHistory: () => {
          set({
            commandHistory: [],
            historyIndex: -1,
          });
        },

        // Set working directory
        setWorkingDirectory: (path: string) => {
          set({ workingDirectory: path });

          // Update active session's working directory
          const { activeSessionId } = get();
          if (activeSessionId) {
            set(state => ({
              sessions: state.sessions.map(session =>
                session.id === activeSessionId
                  ? { ...session, workingDirectory: path }
                  : session
              ),
            }));
          }
        },

        // Set font size
        setFontSize: (size: number) => {
          set({ fontSize: Math.max(8, Math.min(32, size)) });
        },

        // Set theme
        setTheme: (theme: 'dark' | 'light') => {
          set({ theme });
        },

        // Set max output lines
        setMaxOutputLines: (lines: number) => {
          set({ maxOutputLines: Math.max(100, Math.min(10000, lines)) });
        },

        // Connect to terminal service
        connect: async () => {
          try {
            set({ isConnected: false, error: null });

            // Initialize terminal backend
            await invoke('initialize_terminal');

            // Get the current working directory from the backend
            try {
              const cwdResult = await invoke<{ success: boolean; data?: string }>('get_terminal_cwd');
              if (cwdResult.success && cwdResult.data) {
                set({ workingDirectory: cwdResult.data });
              }
            } catch (error) {
              console.warn('Failed to get working directory:', error);
            }

            // Start periodic cleanup to prevent memory leaks
            if (cleanupInterval) {
              clearInterval(cleanupInterval);
            }
            cleanupInterval = setInterval(() => {
              get().cleanupOldCommands();
            }, 5 * 60 * 1000); // Every 5 minutes

            set({ isConnected: true });
          } catch (error) {
            const errorMessage = error instanceof Error ? error.message : 'Failed to connect to terminal';
            set({
              isConnected: false,
              error: errorMessage,
            });
          }
        },

        // Disconnect from terminal service
        disconnect: () => {
          // Clear cleanup interval
          if (cleanupInterval) {
            clearInterval(cleanupInterval);
            cleanupInterval = null;
          }

          set({
            isConnected: false,
            isExecuting: false,
          });
        },

        // Clear error state
        clearError: () => {
          set({ error: null });
        },

        // Get the active session
        getActiveSession: () => {
          const { sessions, activeSessionId } = get();
          return sessions.find(s => s.id === activeSessionId) || null;
        },

        // Get commands for a specific session
        getSessionCommands: (sessionId: string) => {
          const session = get().sessions.find(s => s.id === sessionId);
          return session?.commands || [];
        },
      })
    ),
    {
      name: 'terminal-store',
      enabled: typeof window !== 'undefined' && (window as any).__DEV__ === true,
    }
  )
);

// Selector hooks for better performance
export const useActiveSession = () => {
  const activeSessionId = useTerminalStore(state => state.activeSessionId);
  const sessions = useTerminalStore(state => state.sessions);
  return sessions.find(s => s.id === activeSessionId) || null;
};

export const useTerminalConfig = () => {
  const { fontSize, theme, maxOutputLines, workingDirectory } = useTerminalStore();
  return { fontSize, theme, maxOutputLines, workingDirectory };
};

export const useTerminalState = () => {
  const { isExecuting, isConnected, error, currentCommand } = useTerminalStore();
  return { isExecuting, isConnected, error, currentCommand };
};

export const useCommandHistory = () => {
  const { commandHistory, historyIndex, navigateHistory, addToHistory, clearHistory } = useTerminalStore();
  return { commandHistory, historyIndex, navigateHistory, addToHistory, clearHistory };
};