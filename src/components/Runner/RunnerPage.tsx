import React from 'react';
import { useRunnerStore } from '../../stores/runnerStore';
import { useConfigStore } from '../../stores/configStore';

const RunnerPage: React.FC = () => {
  const {
    currentRun,
    isRunning,
    logs,
    isLoading,
    error,
    runDoctor,
    runPrompt,
    clearLogs
  } = useRunnerStore();

  const { sandboxConfig, isConfigured } = useConfigStore();

  const handleRunDoctor = async () => {
    if (!sandboxConfig) {
      console.error('No configuration available');
      return;
    }

    try {
      await runDoctor(sandboxConfig);
    } catch (error) {
      console.error('Failed to run doctor:', error);
    }
  };

  const handleRunPrompt = async () => {
    if (!sandboxConfig) {
      console.error('No configuration available');
      return;
    }

    try {
      await runPrompt('Hello, ElizaOS!', sandboxConfig.defaultModel || 'gpt-4o-mini', sandboxConfig);
    } catch (error) {
      console.error('Failed to run prompt:', error);
    }
  };

  if (!isConfigured) {
    return (
      <div className="runner-page">
        <div className="no-config-message">
          <h3>Configuration Required</h3>
          <p>Please configure your Sandbox settings in the Settings tab before running ElizaOS CLI commands.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="runner-page">
      <div className="runner-controls">
        <h2>ElizaOS CLI Runner</h2>

        <div className="preset-buttons">
          <button
            onClick={handleRunDoctor}
            disabled={isRunning || isLoading}
            className="preset-button doctor"
          >
            {isRunning ? 'Running...' : 'Run Doctor'}
          </button>

          <button
            onClick={handleRunPrompt}
            disabled={isRunning || isLoading}
            className="preset-button prompt"
          >
            {isRunning ? 'Running...' : 'Run Test Prompt'}
          </button>

          <button
            onClick={clearLogs}
            disabled={logs.length === 0}
            className="clear-logs-button"
          >
            Clear Logs
          </button>
        </div>

        {currentRun && (
          <div className="current-run-info">
            <div className="run-header">
              <span>Run ID: {currentRun.id}</span>
              <span className={`status ${currentRun.status}`}>
                Status: {currentRun.status}
              </span>
            </div>

            <div className="run-details">
              <div>Mode: {currentRun.spec.mode}</div>
              <div>Started: {new Date(currentRun.startedAt).toLocaleTimeString()}</div>
              {currentRun.endedAt && (
                <div>Ended: {new Date(currentRun.endedAt).toLocaleTimeString()}</div>
              )}
              {currentRun.durationMs && (
                <div>Duration: {currentRun.durationMs}ms</div>
              )}
            </div>
          </div>
        )}
      </div>

      <div className="log-viewer">
        <div className="log-header">
          <h3>Logs</h3>
          <span className="log-count">{logs.length} entries</span>
        </div>

        <div className="log-content">
          {logs.length === 0 ? (
            <div className="no-logs">No logs available. Run a command to see output.</div>
          ) : (
            <div className="log-entries">
              {logs.map((log) => (
                <div key={log.id} className={`log-entry ${log.type}`}>
                  <span className="log-timestamp">
                    {log.timestamp.toLocaleTimeString()}
                  </span>
                  <span className="log-type">{log.type.toUpperCase()}</span>
                  <span className="log-content">{log.content}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {error && (
        <div className="error-message">
          <strong>Error:</strong> {error}
        </div>
      )}
    </div>
  );
};

export default RunnerPage;