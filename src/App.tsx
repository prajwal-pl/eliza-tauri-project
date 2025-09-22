import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from './stores/appStore';
import { useConfigStore } from './stores/configStore';
import { initializeLogListeners } from './stores/runnerStore';
import SettingsPage from './components/Settings/SettingsPage';
import RunnerPage from './components/Runner/RunnerPage';
import './App.css';

function App() {
  const { currentView, isLoading, error, initialize, clearError } = useAppStore();
  const { loadConfig } = useConfigStore();

  useEffect(() => {
    // Initialize the application
    const initApp = async () => {
      try {
        await initialize();
        await loadConfig();
        await initializeLogListeners();
      } catch (error) {
        console.error('Failed to initialize app:', error);
      }
    };

    initApp();
  }, [initialize, loadConfig]);

  const handleTestCommand = async () => {
    try {
      // Test basic IPC - this should work even if other commands fail
      const result = await invoke('greet', { name: 'ElizaOS Desktop' });
      console.log('Test result:', result);
    } catch (error) {
      console.error('Test failed:', error);
    }
  };

  if (isLoading) {
    return (
      <div className="app-loading">
        <div className="loading-spinner"></div>
        <p>Loading ElizaOS CLI Desktop...</p>
      </div>
    );
  }

  return (
    <div className="app">
      <header className="app-header">
        <div className="header-content">
          <h1 className="app-title">ElizaOS CLI Desktop</h1>
          <div className="header-controls">
            <button
              onClick={handleTestCommand}
              className="test-button"
              title="Test IPC Communication"
            >
              Test IPC
            </button>
          </div>
        </div>
      </header>

      <nav className="app-nav">
        <div className="nav-tabs">
          <button
            className={`nav-tab ${currentView === 'settings' ? 'active' : ''}`}
            onClick={() => useAppStore.getState().setCurrentView('settings')}
          >
            Settings
          </button>
          <button
            className={`nav-tab ${currentView === 'runner' ? 'active' : ''}`}
            onClick={() => useAppStore.getState().setCurrentView('runner')}
          >
            Runner
          </button>
        </div>
      </nav>

      <main className="app-main">
        {error && (
          <div className="error-banner">
            <span className="error-message">{error}</span>
            <button onClick={clearError} className="error-dismiss">×</button>
          </div>
        )}

        <div className="page-container">
          {currentView === 'settings' ? <SettingsPage /> : <RunnerPage />}
        </div>
      </main>

      <footer className="app-footer">
        <div className="footer-content">
          <span className="version">v0.1.0</span>
          <span className="status-indicator">
            {isLoading ? 'Loading...' : 'Ready'}
          </span>
        </div>
      </footer>
    </div>
  );
}

export default App;