import React, { useState } from 'react';
import { useConfigStore } from '../../stores/configStore';
import { SandboxConfig, DEFAULT_SANDBOX_CONFIG } from '../../types';

const SettingsPage: React.FC = () => {
  const { sandboxConfig, preflightResult, isLoading, error, testConnection, runPreflightCheck, saveConfig, resetConfig, clearError } = useConfigStore();

  // Form state
  const [formData, setFormData] = useState<SandboxConfig>({
    baseUrl: sandboxConfig?.baseUrl || DEFAULT_SANDBOX_CONFIG.baseUrl || '',
    apiKey: sandboxConfig?.apiKey || '',
    defaultModel: sandboxConfig?.defaultModel || DEFAULT_SANDBOX_CONFIG.defaultModel || ''
  });

  const [validationErrors, setValidationErrors] = useState<Record<string, string>>({});
  const [isTestingConnection, setIsTestingConnection] = useState(false);

  const handleTestConnection = async () => {
    try {
      setIsTestingConnection(true);
      await testConnection();
    } catch (error) {
      console.error('Connection test failed:', error);
    } finally {
      setIsTestingConnection(false);
    }
  };

  const handleRunPreflight = async () => {
    try {
      await runPreflightCheck();
    } catch (error) {
      console.error('Preflight check failed:', error);
    }
  };

  const validateForm = (): boolean => {
    const errors: Record<string, string> = {};

    // Base URL validation
    if (!formData.baseUrl.trim()) {
      errors.baseUrl = 'Base URL is required';
    } else {
      try {
        new URL(formData.baseUrl);
      } catch {
        errors.baseUrl = 'Please enter a valid URL';
      }
    }

    // API Key validation
    if (!formData.apiKey.trim()) {
      errors.apiKey = 'API Key is required';
    } else if (!/^eliza_[a-f0-9]{64}$/.test(formData.apiKey) || formData.apiKey.length !== 70) {
      errors.apiKey = 'API Key must start with "eliza_" followed by 64 hexadecimal characters (70 characters total)';
    }

    setValidationErrors(errors);
    return Object.keys(errors).length === 0;
  };

  const handleInputChange = (field: keyof SandboxConfig, value: string) => {
    setFormData(prev => ({ ...prev, [field]: value }));

    // Clear validation error for this field
    if (validationErrors[field]) {
      setValidationErrors(prev => ({ ...prev, [field]: '' }));
    }

    // Clear general error
    if (error) {
      clearError();
    }
  };

  const handleSaveConfig = async () => {
    if (!validateForm()) {
      return;
    }

    try {
      await saveConfig(formData);
    } catch (error) {
      console.error('Failed to save configuration:', error);
    }
  };

  const handleTestFormConnection = async () => {
    if (!validateForm()) {
      return;
    }

    try {
      setIsTestingConnection(true);
      // First save the config, then test connection
      await saveConfig(formData);
      await testConnection();
    } catch (error) {
      console.error('Connection test failed:', error);
    } finally {
      setIsTestingConnection(false);
    }
  };

  const handleResetConfig = async () => {
    try {
      await resetConfig();
      setFormData({
        baseUrl: DEFAULT_SANDBOX_CONFIG.baseUrl || '',
        apiKey: '',
        defaultModel: DEFAULT_SANDBOX_CONFIG.defaultModel || ''
      });
      setValidationErrors({});
    } catch (error) {
      console.error('Failed to reset configuration:', error);
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
              <div className={`status-badge ${preflightResult.overallStatus}`}>
                Status: {preflightResult.overallStatus?.replace('_', ' ').toUpperCase() || 'UNKNOWN'}
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
          <div className="form-group">
            <label htmlFor="baseUrl">Base URL *</label>
            <input
              type="url"
              id="baseUrl"
              placeholder="https://eliza-cloud-private-production.up.railway.app/api/v1"
              value={formData.baseUrl}
              onChange={(e) => handleInputChange('baseUrl', e.target.value)}
              className={validationErrors.baseUrl ? 'error' : ''}
            />
            {validationErrors.baseUrl && (
              <span className="field-error">{validationErrors.baseUrl}</span>
            )}
          </div>

          <div className="form-group">
            <label htmlFor="apiKey">API Key *</label>
            <input
              type="password"
              id="apiKey"
              placeholder="eliza_[64 character hex string] (70 chars total)"
              value={formData.apiKey}
              onChange={(e) => handleInputChange('apiKey', e.target.value)}
              className={validationErrors.apiKey ? 'error' : ''}
            />
            {validationErrors.apiKey && (
              <span className="field-error">{validationErrors.apiKey}</span>
            )}
            <small className="field-help">
              Your Sandbox API key. Must start with "eliza_" followed by 64 hexadecimal characters (70 characters total).
            </small>
          </div>


          <div className="form-group">
            <label htmlFor="defaultModel">Default Model</label>
            <select
              id="defaultModel"
              value={formData.defaultModel}
              onChange={(e) => handleInputChange('defaultModel', e.target.value)}
            >
              <option value="">Select a model</option>
              <option value="gpt-4o-mini">GPT-4o Mini</option>
              <option value="gpt-4o">GPT-4o</option>
              <option value="gpt-4-turbo">GPT-4 Turbo</option>
              <option value="gpt-3.5-turbo">GPT-3.5 Turbo</option>
            </select>
            <small className="field-help">
              Default model to use for ElizaOS CLI operations.
            </small>
          </div>

          <div className="form-actions">
            <button
              type="button"
              onClick={handleSaveConfig}
              disabled={isLoading}
              className="btn-primary"
            >
              {isLoading ? 'Saving...' : 'Save Configuration'}
            </button>

            <button
              type="button"
              onClick={handleTestFormConnection}
              disabled={isLoading || isTestingConnection}
              className="btn-secondary"
            >
              {isTestingConnection ? 'Testing...' : 'Test & Save'}
            </button>

            <button
              type="button"
              onClick={handleResetConfig}
              disabled={isLoading}
              className="btn-tertiary"
            >
              Reset to Defaults
            </button>
          </div>

          {sandboxConfig && (
            <div className="current-config">
              <h4>Current Configuration:</h4>
              <div className="config-display">
                <div><strong>Base URL:</strong> {sandboxConfig.baseUrl}</div>
                <div><strong>API Key:</strong> {sandboxConfig.apiKey.substring(0, 12)}***</div>
                {sandboxConfig.defaultModel && (
                  <div><strong>Default Model:</strong> {sandboxConfig.defaultModel}</div>
                )}
              </div>

              <button
                onClick={handleTestConnection}
                disabled={isLoading || isTestingConnection}
                className="btn-test"
              >
                {isTestingConnection ? 'Testing...' : 'Test Current Configuration'}
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