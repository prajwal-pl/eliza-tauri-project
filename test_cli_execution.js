// Simple test to verify real CLI execution via IPC
const { invoke } = require('@tauri-apps/api');

async function testRealCliExecution() {
  try {
    console.log('Testing real ElizaOS CLI execution...');

    // Create a test configuration
    const testConfig = {
      baseUrl: 'https://api.sandbox.test',
      apiKey: 'eliza_test_key_12345678901234567890123456789012345678901234567890123456789012345',
      projectId: 'test-project-123',
      defaultModel: 'gpt-4'
    };

    // Create a test run specification
    const testSpec = {
      id: 'test-run-1',
      mode: 'Doctor', // This should trigger `elizaos start --mode diagnostic`
      args: ['--help'],
      working_dir: null
    };

    console.log('Invoking start_eliza_run with real CLI...');

    const result = await invoke('start_eliza_run', {
      spec: testSpec,
      config: testConfig
    });

    console.log('CLI execution result:');
    console.log(JSON.stringify(result, null, 2));

    if (result.success && result.data) {
      console.log('\n✅ REAL CLI EXECUTION SUCCESS!');
      console.log('Status:', result.data.status);
      console.log('Exit Code:', result.data.exit_code);
      console.log('Duration:', result.data.duration_ms + 'ms');
      console.log('Stdout lines:', result.data.stdout.length);
      console.log('Stderr lines:', result.data.stderr.length);

      if (result.data.stdout.length > 0) {
        console.log('\nFirst few stdout lines:');
        result.data.stdout.slice(0, 5).forEach((line, i) => {
          console.log(`  ${i + 1}: ${line}`);
        });
      }
    } else {
      console.log('\n❌ CLI execution failed:');
      console.log(result.error || 'Unknown error');
    }

  } catch (error) {
    console.error('❌ Test failed with error:', error);
  }
}

// Run the test
testRealCliExecution();