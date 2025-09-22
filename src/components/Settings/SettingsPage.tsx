import React from 'react';
import { useConfigStore } from '../../stores/configStore';

const SettingsPage: React.FC = () => {
  const { sandboxConfig, preflightResult, isLoading, error, testConnection, runPreflightCheck } = useConfigStore();

  const handleTestConnection = async () => {
    try {
      await testConnection();
    } catch (error) {
      console.error('Connection test failed:', error);
    }
  };

  const handleRunPreflight = async () => {
    try {
      await runPreflightCheck();
    } catch (error) {
      console.error('Preflight check failed:', error);
    }
  };

  return (
    <div className="settings-page">
      <div className="settings-section">
        <h2>System Requirements</h2>
        <div className="preflight-section">
          <button onClick={handleRunPreflight} disabled={isLoading}>
            {isLoading ? 'Checking...' : 'Check System Requirements'}
          </button>

          {preflightResult && (
            <div className="preflight-results">
              <div className={`status-badge ${preflightResult.overall_status}`}>
                Status: {preflightResult.overall_status.replace('_', ' ').toUpperCase()}
              </div>

              <div className="tool-checks">
                <div className="tool-check">
                  <span>Node.js:</span>
                  <span className={preflightResult.node.installed ? 'installed' : 'missing'}>
                    {preflightResult.node.installed
                      ? `✓ ${preflightResult.node.version || 'Installed'}`
                      : '✗ Not found'}
                  </span>
                </div>

                <div className="tool-check">
                  <span>NPM:</span>
                  <span className={preflightResult.npm.installed ? 'installed' : 'missing'}>
                    {preflightResult.npm.installed
                      ? `✓ ${preflightResult.npm.version || 'Installed'}`
                      : '✗ Not found'}
                  </span>
                </div>

                <div className="tool-check">
                  <span>ElizaOS CLI:</span>
                  <span className={preflightResult.eliza.installed ? 'installed' : 'missing'}>
                    {preflightResult.eliza.installed
                      ? `✓ ${preflightResult.eliza.version || 'Available'}`
                      : '✗ Will be installed via npx'}
                  </span>
                </div>
              </div>

              {preflightResult.recommendations.length > 0 && (
                <div className="recommendations">
                  <h4>Recommendations:</h4>
                  <ul>
                    {preflightResult.recommendations.map((rec, index) => (
                      <li key={index}>{rec}</li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      <div className="settings-section">
        <h2>Sandbox Configuration</h2>
        <div className="config-form">
          <p>Configuration form will be implemented in the next iteration.</p>
          <p>For now, this demonstrates the UI structure and state management.</p>

          {sandboxConfig && (
            <div className="current-config">
              <h4>Current Configuration:</h4>
              <div className="config-display">
                <div>Base URL: {sandboxConfig.baseUrl}</div>
                <div>Project ID: {sandboxConfig.projectId}</div>
                <div>API Key: {sandboxConfig.apiKey.substring(0, 12)}***</div>
                {sandboxConfig.defaultModel && (
                  <div>Default Model: {sandboxConfig.defaultModel}</div>
                )}
              </div>

              <button onClick={handleTestConnection} disabled={isLoading}>
                {isLoading ? 'Testing...' : 'Test Connection'}
              </button>
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

export default SettingsPage;