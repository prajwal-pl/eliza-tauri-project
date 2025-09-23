import React, { useEffect, useRef, useState, KeyboardEvent } from 'react';
import { useTerminalStore, useActiveSession, useTerminalState } from '../../stores/terminalStore';
import { useConfigStore } from '../../stores/configStore';

// Safe date formatting utility
const formatTimestamp = (date: Date): string => {
  try {
    if (!date || isNaN(date.getTime())) {
      return 'Invalid Date';
    }
    return date.toLocaleTimeString();
  } catch {
    return 'Invalid Date';
  }
};

const TerminalPage: React.FC = () => {
  const {
    sessions,
    currentCommand,
    commandHistory,
    workingDirectory,
    fontSize,
    createSession,
    closeSession,
    setActiveSession,
    setCurrentCommand,
    executeCommand,
    navigateHistory,
    connect,
    clearSession,
  } = useTerminalStore();

  const activeSession = useActiveSession();
  const { isExecuting, isConnected, error } = useTerminalState();
  const { isConfigured } = useConfigStore();

  const outputRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  // Initialize terminal on component mount
  useEffect(() => {
    if (isConfigured && !isConnected && sessions.length === 0) {
      connect().then(() => {
        createSession('Main Terminal');
      });
    }
  }, [isConfigured, isConnected, sessions.length]);

  // Auto-scroll to bottom when new output arrives
  useEffect(() => {
    if (autoScroll && outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [activeSession?.commands, autoScroll]);

  // Focus input when session changes
  useEffect(() => {
    if (inputRef.current && activeSession) {
      inputRef.current.focus();
    }
  }, [activeSession]);

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    switch (event.key) {
      case 'Enter':
        event.preventDefault();
        if (currentCommand.trim() && !isExecuting) {
          executeCommand(currentCommand.trim());
        }
        break;

      case 'ArrowUp':
        event.preventDefault();
        navigateHistory('up');
        break;

      case 'ArrowDown':
        event.preventDefault();
        navigateHistory('down');
        break;

      case 'Tab':
        event.preventDefault();
        // TODO: Implement tab completion
        break;

      case 'c':
        if (event.ctrlKey || event.metaKey) {
          event.preventDefault();
          // TODO: Implement command cancellation
        }
        break;
    }
  };

  const handleCreateSession = () => {
    const sessionName = `Terminal ${sessions.length + 1}`;
    createSession(sessionName);
  };

  const handleClearSession = () => {
    if (activeSession) {
      clearSession(activeSession.id);
    }
  };

  const renderCommandOutput = (command: any) => {
    const getStatusColor = (status: string) => {
      switch (status) {
        case 'running': return 'text-yellow-400 bg-yellow-900/20 border-yellow-600/30';
        case 'completed': return 'text-green-400 bg-green-900/20 border-green-600/30';
        case 'failed': return 'text-red-400 bg-red-900/20 border-red-600/30';
        default: return 'text-gray-400 bg-gray-900/20 border-gray-600/30';
      }
    };

    const getStatusIcon = (status: string) => {
      switch (status) {
        case 'running': return '⏳';
        case 'completed': return '✅';
        case 'failed': return '❌';
        default: return '⚪';
      }
    };

    // Format the command string
    const fullCommand = `${command.command} ${command.args.join(' ')}`.trim();

    return (
      <div key={command.id} className="mb-6 bg-gray-800/30 rounded-lg border border-gray-700/50 overflow-hidden shadow-lg">
        {/* Command Header */}
        <div className="flex items-center justify-between px-4 py-3 bg-gray-800/60 border-b border-gray-700/50">
          <div className="flex items-center gap-3 min-w-0 flex-1">
            {/* Terminal Prompt */}
            <div className="flex items-center gap-2 font-mono text-sm">
              <span className="px-2 py-1 bg-green-600/80 text-green-100 rounded font-semibold">
                user@eliza
              </span>
              <span className="text-gray-400">:</span>
              <span className="px-2 py-1 bg-blue-600/80 text-blue-100 rounded font-medium truncate max-w-40" title={workingDirectory}>
                {workingDirectory === '/home/prajwal' ? '~' : workingDirectory.replace('/home/prajwal', '~')}
              </span>
              <span className="text-white font-bold text-lg">$</span>
            </div>

            {/* Command Text */}
            <span className="text-white font-mono text-sm bg-gray-900/50 px-3 py-1 rounded border border-gray-600/30">
              {fullCommand}
            </span>
          </div>

          {/* Status Badge */}
          <div className={`flex items-center gap-2 px-3 py-1.5 rounded-full border text-xs font-medium ${getStatusColor(command.status)}`}>
            <span>{getStatusIcon(command.status)}</span>
            <span className="capitalize">{command.status}</span>
            <span className="text-gray-400 ml-1">• {formatTimestamp(command.timestamp)}</span>
          </div>
        </div>

        {/* Command Output */}
        <div className="p-4">
          {command.output && command.output.length > 0 ? (
            <div className="bg-black/40 rounded-lg p-4 border border-gray-600/30">
              <div className="flex items-center gap-2 mb-3 pb-2 border-b border-gray-700/30">
                <div className="w-3 h-3 rounded-full bg-green-500"></div>
                <span className="text-gray-400 text-xs font-medium">OUTPUT</span>
              </div>
              <div className="space-y-1 max-h-96 overflow-y-auto custom-scrollbar">
                {command.output.map((line: string, index: number) => {
                  const isStderr = line.startsWith('stderr:');
                  const cleanLine = isStderr ? line.replace('stderr: ', '') : line;

                  return (
                    <div
                      key={index}
                      className={`font-mono text-sm leading-relaxed ${
                        isStderr
                          ? 'text-red-300 bg-red-900/20 px-2 py-1 rounded border-l-2 border-red-500'
                          : 'text-gray-100 hover:bg-gray-800/30 px-1 rounded'
                      }`}
                    >
                      <span className="whitespace-pre-wrap">{cleanLine}</span>
                    </div>
                  );
                })}
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-center py-8 text-gray-500">
              <div className="text-center space-y-2">
                <div className="w-8 h-8 rounded-full bg-gray-700 flex items-center justify-center mx-auto">
                  <span className="text-xs">📝</span>
                </div>
                <p className="text-sm">No output</p>
              </div>
            </div>
          )}

          {/* Error Output */}
          {command.error && (
            <div className="mt-4 bg-red-900/30 rounded-lg p-4 border border-red-600/30">
              <div className="flex items-center gap-2 mb-2">
                <div className="w-3 h-3 rounded-full bg-red-500"></div>
                <span className="text-red-400 text-xs font-bold">ERROR</span>
              </div>
              <div className="text-red-300 font-mono text-sm leading-relaxed">
                {command.error}
              </div>
            </div>
          )}

          {/* Exit Code */}
          {command.exitCode !== undefined && (
            <div className="mt-3 pt-3 border-t border-gray-700/30">
              <span className="text-xs text-gray-500">
                Exit code: <span className={command.exitCode === 0 ? 'text-green-400' : 'text-red-400'}>{command.exitCode}</span>
              </span>
            </div>
          )}
        </div>
      </div>
    );
  };

  if (!isConfigured) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <h2 className="text-xl font-semibold mb-4">Terminal Not Available</h2>
          <p className="text-gray-600 mb-4">
            Please configure your sandbox settings first to use the terminal.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col h-full bg-gradient-to-br from-slate-900 via-gray-900 to-slate-800">
      {/* Terminal Header */}
      <div className="flex items-center justify-between px-6 py-4 bg-gradient-to-r from-gray-800 via-gray-900 to-gray-800 border-b border-gray-700 shadow-lg">
        <div className="flex items-center gap-6">
          {/* Terminal title with modern styling */}
          <div className="flex items-center gap-4">
            <div className="flex gap-2">
              <div className="w-3 h-3 rounded-full bg-red-500 hover:bg-red-400 transition-colors cursor-pointer shadow-sm"></div>
              <div className="w-3 h-3 rounded-full bg-yellow-500 hover:bg-yellow-400 transition-colors cursor-pointer shadow-sm"></div>
              <div className="w-3 h-3 rounded-full bg-green-500 hover:bg-green-400 transition-colors cursor-pointer shadow-sm"></div>
            </div>
            <div className="flex items-center gap-3">
              <div className="w-6 h-6 rounded bg-blue-600 flex items-center justify-center">
                <span className="text-white text-xs font-bold">$</span>
              </div>
              <h2 className="text-lg font-semibold text-white">ElizaOS Terminal</h2>
            </div>
          </div>

          <div className="flex items-center gap-3 px-3 py-1.5 bg-gray-800 rounded-full border border-gray-600">
            <div className={`w-2.5 h-2.5 rounded-full ${isConnected ? 'bg-green-400' : 'bg-red-400'} ${isConnected ? 'animate-pulse' : ''}`} />
            <span className="text-sm text-gray-300 font-medium">
              {isConnected ? 'Connected' : 'Disconnected'}
            </span>
          </div>
        </div>

        <div className="flex items-center gap-3">
          {/* Session Tabs */}
          <div className="flex gap-2">
            {sessions.map((session) => (
              <div
                key={session.id}
                className={`flex items-center gap-2 px-4 py-2 rounded-lg transition-all duration-200 ${
                  activeSession?.id === session.id
                    ? 'bg-blue-600 text-white shadow-lg border border-blue-500'
                    : 'bg-gray-700 text-gray-300 hover:bg-gray-600 border border-gray-600 hover:border-gray-500'
                }`}
              >
                <button
                  onClick={() => setActiveSession(session.id)}
                  className="text-sm font-medium"
                >
                  {session.title}
                </button>
                {sessions.length > 1 && (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      closeSession(session.id);
                    }}
                    className="w-4 h-4 rounded-full hover:bg-red-500 hover:text-white flex items-center justify-center text-xs transition-colors"
                  >
                    ×
                  </button>
                )}
              </div>
            ))}
          </div>

          <div className="flex gap-2">
            <button
              onClick={handleCreateSession}
              className="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-green-600 hover:bg-green-500 text-white rounded-lg shadow-md border border-green-500 transition-all duration-200 hover:shadow-lg"
            >
              <span className="text-lg">+</span>
              New Tab
            </button>

            <button
              onClick={handleClearSession}
              disabled={!activeSession}
              className="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-orange-600 hover:bg-orange-500 disabled:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg shadow-md border border-orange-500 disabled:border-gray-500 transition-all duration-200 hover:shadow-lg"
            >
              <span className="text-sm">🗑</span>
              Clear
            </button>
          </div>
        </div>
      </div>

      {/* Terminal Content */}
      <div className="flex-1 flex flex-col">
        {activeSession ? (
          <>
            {/* Terminal Output Area */}
            <div
              ref={outputRef}
              className="flex-1 overflow-y-auto bg-gradient-to-b from-gray-900 via-slate-900 to-gray-900 custom-scrollbar"
              onClick={() => inputRef.current?.focus()}
              style={{
                background: 'radial-gradient(ellipse at center top, rgba(30, 41, 59, 0.5) 0%, rgba(15, 23, 42, 0.8) 50%, rgba(0, 0, 0, 0.9) 100%)'
              }}
            >
              {/* Content wrapper with proper padding */}
              <div className="p-6 min-h-full">
                {activeSession.commands.length === 0 ? (
                  /* Welcome Screen */
                  <div className="space-y-6">
                    {/* Header */}
                    <div className="text-center space-y-3 py-8">
                      <div className="w-16 h-16 mx-auto bg-gradient-to-br from-blue-500 to-purple-600 rounded-2xl flex items-center justify-center shadow-lg">
                        <span className="text-white text-2xl font-bold">$</span>
                      </div>
                      <h3 className="text-2xl font-bold text-white">ElizaOS Terminal</h3>
                      <p className="text-gray-400 text-lg">Ready for your commands</p>
                    </div>

                    {/* Quick start guide */}
                    <div className="bg-gradient-to-r from-blue-900/30 to-purple-900/30 rounded-xl p-6 border border-blue-800/30">
                      <h4 className="text-lg font-semibold text-blue-300 mb-4 flex items-center gap-2">
                        <span className="w-5 h-5 bg-blue-500 rounded-full flex items-center justify-center text-xs">i</span>
                        Quick Start Guide
                      </h4>
                      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
                        <div className="space-y-2">
                          <p className="text-gray-300 font-medium">Basic Commands:</p>
                          <div className="space-y-1 pl-4">
                            <div className="flex items-center gap-2">
                              <code className="bg-gray-800 px-2 py-1 rounded text-green-400">help</code>
                              <span className="text-gray-400">Show available commands</span>
                            </div>
                            <div className="flex items-center gap-2">
                              <code className="bg-gray-800 px-2 py-1 rounded text-green-400">ls</code>
                              <span className="text-gray-400">List directory contents</span>
                            </div>
                            <div className="flex items-center gap-2">
                              <code className="bg-gray-800 px-2 py-1 rounded text-green-400">pwd</code>
                              <span className="text-gray-400">Show current directory</span>
                            </div>
                          </div>
                        </div>
                        <div className="space-y-2">
                          <p className="text-gray-300 font-medium">System Info:</p>
                          <div className="space-y-1 pl-4">
                            <div className="flex items-center gap-2">
                              <code className="bg-gray-800 px-2 py-1 rounded text-green-400">whoami</code>
                              <span className="text-gray-400">Show current user</span>
                            </div>
                            <div className="flex items-center gap-2">
                              <code className="bg-gray-800 px-2 py-1 rounded text-green-400">date</code>
                              <span className="text-gray-400">Show current date/time</span>
                            </div>
                            <div className="flex items-center gap-2">
                              <code className="bg-gray-800 px-2 py-1 rounded text-green-400">uname -a</code>
                              <span className="text-gray-400">System information</span>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                ) : (
                  /* Command History */
                  <div className="space-y-4">
                    {activeSession.commands.map(renderCommandOutput)}
                  </div>
                )}

                {/* Current Input Line */}
                <div className="mt-6 pt-4 border-t border-gray-700/50">
                  <div className="flex items-center gap-3 p-4 bg-gray-800/40 rounded-lg border border-gray-700/50 shadow-inner">
                    {/* Prompt */}
                    <div className="flex items-center gap-2 text-sm font-mono shrink-0">
                      <span className="px-2 py-1 bg-gradient-to-r from-green-500 to-emerald-500 text-white rounded font-semibold">
                        user@eliza
                      </span>
                      <span className="text-gray-400">:</span>
                      <span className="px-2 py-1 bg-blue-600/80 text-blue-100 rounded font-medium" title={workingDirectory}>
                        {workingDirectory === '/home/prajwal' ? '~' : workingDirectory.replace('/home/prajwal', '~')}
                      </span>
                      <span className="text-white font-bold text-lg">$</span>
                    </div>

                    {/* Input */}
                    <input
                      ref={inputRef}
                      type="text"
                      value={currentCommand}
                      onChange={(e) => setCurrentCommand(e.target.value)}
                      onKeyDown={handleKeyDown}
                      disabled={isExecuting}
                      className="flex-1 bg-transparent outline-none text-white font-mono text-lg placeholder-gray-500 caret-green-400"
                      placeholder={isExecuting ? "Executing..." : "Type a command..."}
                      autoComplete="off"
                      spellCheck={false}
                      style={{ fontSize: `${fontSize + 2}px` }}
                    />

                    {/* Execution indicator */}
                    {isExecuting && (
                      <div className="flex items-center gap-2 text-yellow-400">
                        <div className="animate-spin w-4 h-4 border-2 border-yellow-400 border-t-transparent rounded-full"></div>
                        <span className="text-sm">Running...</span>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </div>

            {/* Status Bar */}
            <div className="p-2 bg-gray-900 border-t border-gray-800 text-xs font-mono flex items-center justify-between">
              <div className="flex items-center gap-4 text-gray-400">
                <span className="text-blue-400">PWD:</span>
                <span className="text-gray-300">{workingDirectory.replace('/home/prajwal', '~')}</span>
                <span className="text-green-400">Commands:</span>
                <span className="text-gray-300">{activeSession.commands.length}</span>
                <span className="text-yellow-400">History:</span>
                <span className="text-gray-300">{commandHistory.length}</span>
              </div>

              <div className="flex items-center gap-4 text-gray-400">
                <label className="flex items-center gap-2 cursor-pointer hover:text-gray-300">
                  <input
                    type="checkbox"
                    checked={autoScroll}
                    onChange={(e) => setAutoScroll(e.target.checked)}
                    className="w-3 h-3 accent-green-500"
                  />
                  <span>Auto-scroll</span>
                </label>
                <div className="flex items-center gap-1">
                  <span className="text-purple-400">Size:</span>
                  <span className="text-gray-300">{fontSize}px</span>
                </div>
              </div>
            </div>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center text-gray-500">
            <div className="text-center">
              <p className="mb-4">No terminal session active</p>
              <button
                onClick={handleCreateSession}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded"
              >
                Create New Session
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Error Display */}
      {error && (
        <div className="p-3 bg-red-900 border-t border-red-800 text-red-300">
          <div className="flex items-center justify-between">
            <span>⚠️ {error}</span>
            <button
              onClick={() => useTerminalStore.getState().clearError()}
              className="text-red-400 hover:text-red-300"
            >
              ×
            </button>
          </div>
        </div>
      )}
    </div>
  );
};

export default TerminalPage;