const { app, BrowserWindow, globalShortcut, ipcMain, Tray, Menu, clipboard, nativeImage } = require('electron');
const path = require('path');
const { spawn } = require('child_process');
const fetch = require('node-fetch');
const WebSocket = require('ws');

// Keep references to prevent garbage collection
let mainWindow = null;
let indicatorWindow = null;
let setupWindow = null;
let tray = null;
let pythonProcess = null;

// Configuration
const API_URL = 'http://127.0.0.1:5000';
const WS_URL = 'ws://127.0.0.1:5000/ws';
let config = {
  hotkey: ['ctrl_l', 'shift_l', 'space'],
  activation_mode: 'push_to_talk',
  window_position: [100, 100]
};

// State
let isRecording = false;
let hotkeyPressed = false;
let ws = null;
let wsReconnectTimer = null;

// === Python Process Management ===

function isDev() {
  return !app.isPackaged;
}

function getPythonPaths() {
  const pythonCmd = process.platform === 'win32' ? 'python' : 'python3';

  if (isDev()) {
    // In dev mode, Python files are in ../python folder
    const pythonDir = path.join(__dirname, '..', 'python');
    return {
      pythonDir: pythonDir,
      venvDir: path.join(__dirname, '..', '.venv'),
      apiScript: path.join(pythonDir, 'api.py'),
      requirements: path.join(pythonDir, 'requirements.txt'),
      systemPython: pythonCmd
    };
  } else {
    const pythonDir = path.join(process.resourcesPath, 'python');
    // Store venv in AppData so it persists across app updates/rebuilds
    const venvDir = path.join(app.getPath('userData'), 'python-venv');
    return {
      pythonDir: pythonDir,
      venvDir: venvDir,
      apiScript: path.join(pythonDir, 'api.py'),
      requirements: path.join(pythonDir, 'requirements.txt'),
      systemPython: pythonCmd
    };
  }
}

function getVenvPython(venvDir) {
  if (process.platform === 'win32') {
    return path.join(venvDir, 'Scripts', 'python.exe');
  }
  return path.join(venvDir, 'bin', 'python');
}

const fs = require('fs');

function sendSetupStatus(status, details = '', progress = undefined) {
  if (setupWindow && !setupWindow.isDestroyed()) {
    setupWindow.webContents.send('setup-status', { status, details, progress });
  }
}

function createSetupWindow() {
  setupWindow = new BrowserWindow({
    width: 450,
    height: 300,
    frame: false,
    transparent: false,
    resizable: false,
    center: true,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: false,
      nodeIntegration: true
    }
  });

  setupWindow.loadFile(path.join(__dirname, 'setup.html'));

  setupWindow.once('ready-to-show', () => {
    setupWindow.show();
  });

  return setupWindow;
}

function closeSetupWindow() {
  if (setupWindow && !setupWindow.isDestroyed()) {
    setupWindow.close();
    setupWindow = null;
  }
}

function getSetupConfigPath() {
  return path.join(app.getPath('userData'), 'setup.json');
}

function readSetupConfig() {
  try {
    const configPath = getSetupConfigPath();
    if (fs.existsSync(configPath)) {
      return JSON.parse(fs.readFileSync(configPath, 'utf8'));
    }
  } catch (e) {
    console.log('Could not read setup config:', e.message);
  }
  return {};
}

function writeSetupConfig(config) {
  try {
    const configPath = getSetupConfigPath();
    fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
    console.log('Setup config saved to:', configPath);
  } catch (e) {
    console.log('Could not write setup config:', e.message);
  }
}

async function checkPackageInstalled(pythonExe, packageName) {
  return new Promise((resolve) => {
    const proc = spawn(pythonExe, ['-m', 'pip', 'show', packageName], {
      stdio: ['pipe', 'pipe', 'pipe']
    });
    proc.on('close', (code) => {
      resolve(code === 0);
    });
    proc.on('error', () => {
      resolve(false);
    });
  });
}

async function ensureVenv(showSetup = false) {
  const paths = getPythonPaths();
  const venvPython = getVenvPython(paths.venvDir);
  const setupConfig = readSetupConfig();

  // Check if venv python exists
  if (fs.existsSync(venvPython)) {
    // Check config file first (fast path)
    if (setupConfig.depsInstalled) {
      console.log('Dependencies already installed (config flag set)');
      return { python: venvPython, needsSetup: false };
    }

    // Config flag missing - verify by checking if a key package is installed
    console.log('Checking if dependencies are already installed...');
    const fastApiInstalled = await checkPackageInstalled(venvPython, 'fastapi');

    if (fastApiInstalled) {
      console.log('Dependencies already installed (fastapi found)');
      // Save to config so we skip this check next time
      writeSetupConfig({ ...setupConfig, depsInstalled: true, installedAt: new Date().toISOString() });
      return { python: venvPython, needsSetup: false };
    }

    console.log('Dependencies not found, will install...');
  }

  // Need to setup - show setup window if requested
  if (showSetup) {
    createSetupWindow();
    await new Promise(resolve => setTimeout(resolve, 500)); // Let window render
  }

  // Create venv if it doesn't exist
  if (!fs.existsSync(venvPython)) {
    console.log('Creating virtual environment...');
    sendSetupStatus('Creating Python environment...', 'Setting up virtual environment');

    await new Promise((resolve, reject) => {
      const proc = spawn(paths.systemPython, ['-m', 'venv', paths.venvDir], {
        stdio: ['pipe', 'pipe', 'pipe']
      });
      proc.stdout.on('data', (data) => {
        console.log(`[venv] ${data.toString().trim()}`);
      });
      proc.stderr.on('data', (data) => {
        console.error(`[venv] ${data.toString().trim()}`);
      });
      proc.on('close', (code) => {
        if (code === 0) resolve();
        else reject(new Error(`venv creation failed with code ${code}`));
      });
      proc.on('error', reject);
    });
  }

  console.log('Installing dependencies (this may take a while on first run)...');
  sendSetupStatus('Installing dependencies...', 'This may take a few minutes');

  // Install requirements
  await new Promise((resolve, reject) => {
    const proc = spawn(venvPython, ['-m', 'pip', 'install', '-r', paths.requirements], {
      stdio: ['pipe', 'pipe', 'pipe'],
      cwd: paths.pythonDir
    });

    proc.stdout.on('data', (data) => {
      const line = data.toString().trim();
      console.log(`[pip] ${line}`);
      // Extract package name from pip output
      const match = line.match(/(?:Collecting|Installing|Downloading)\s+(\S+)/i);
      if (match) {
        sendSetupStatus('Installing dependencies...', match[1]);
      }
    });

    proc.stderr.on('data', (data) => {
      const line = data.toString().trim();
      console.error(`[pip] ${line}`);
      // Also check stderr for progress (pip sometimes outputs there)
      if (line.includes('Collecting') || line.includes('Installing')) {
        const match = line.match(/(?:Collecting|Installing)\s+(\S+)/i);
        if (match) {
          sendSetupStatus('Installing dependencies...', match[1]);
        }
      }
    });

    proc.on('close', (code) => {
      if (code === 0) resolve();
      else reject(new Error(`pip install failed with code ${code}`));
    });
    proc.on('error', reject);
  });

  // Save to config to indicate successful installation
  writeSetupConfig({ depsInstalled: true, installedAt: new Date().toISOString() });

  console.log('Dependencies installed');
  sendSetupStatus('Dependencies installed', 'Starting application...');

  return { python: venvPython, needsSetup: true };
}

async function startPythonAPI() {
  const paths = getPythonPaths();

  console.log('Starting Python API server...');
  console.log('Mode:', isDev() ? 'development' : 'production');

  let pythonExe;

  // In dev mode, use system python directly (assume deps are installed)
  // In production, ensure venv exists
  if (isDev()) {
    pythonExe = paths.systemPython;
  } else {
    try {
      const result = await ensureVenv(true); // Show setup window in production
      pythonExe = result.python;

      // Close setup window after a brief delay to show "Starting application..."
      if (result.needsSetup) {
        await new Promise(resolve => setTimeout(resolve, 1000));
      }
      closeSetupWindow();
    } catch (err) {
      console.error('Failed to setup Python environment:', err);
      sendSetupStatus('Setup failed', err.message);
      return;
    }
  }

  console.log('Python:', pythonExe);
  console.log('Script:', paths.apiScript);

  pythonProcess = spawn(pythonExe, [paths.apiScript], {
    stdio: ['pipe', 'pipe', 'pipe'],
    cwd: paths.pythonDir
  });

  pythonProcess.stdout.on('data', (data) => {
    console.log(`[Python] ${data.toString().trim()}`);
  });

  pythonProcess.stderr.on('data', (data) => {
    console.error(`[Python Error] ${data.toString().trim()}`);
  });

  pythonProcess.on('close', (code) => {
    console.log(`Python process exited with code ${code}`);
    pythonProcess = null;
  });

  pythonProcess.on('error', (err) => {
    console.error('Failed to start Python process:', err);
  });
}

function stopPythonAPI() {
  if (pythonProcess) {
    console.log('Stopping Python API server...');
    pythonProcess.kill();
    pythonProcess = null;
  }
}

// === API Communication ===

async function apiCall(endpoint, method = 'GET', body = null) {
  try {
    const options = {
      method,
      headers: { 'Content-Type': 'application/json' }
    };

    if (body) {
      options.body = JSON.stringify(body);
    }

    const response = await fetch(`${API_URL}${endpoint}`, options);
    return await response.json();
  } catch (error) {
    console.error(`API call failed (${endpoint}):`, error.message);
    return null;
  }
}

async function waitForAPI(maxAttempts = 30) {
  for (let i = 0; i < maxAttempts; i++) {
    try {
      const response = await fetch(`${API_URL}/health`);
      if (response.ok) {
        console.log('API server is ready');
        return true;
      }
    } catch (e) {
      // Server not ready yet
    }
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  console.error('API server failed to start');
  return false;
}

// === WebSocket Connection ===

function connectWebSocket() {
  if (ws) {
    ws.close();
  }

  console.log('Connecting to WebSocket...');
  ws = new WebSocket(WS_URL);

  ws.on('open', () => {
    console.log('WebSocket connected');
    if (wsReconnectTimer) {
      clearTimeout(wsReconnectTimer);
      wsReconnectTimer = null;
    }
  });

  ws.on('message', (data) => {
    try {
      const message = JSON.parse(data.toString());

      if (message.type === 'status') {
        // Update indicator window with new status
        indicatorWindow?.webContents.send('status-update', message.status);

        // Resize indicator window based on recording state
        if (indicatorWindow && !indicatorWindow.isDestroyed()) {
          if (message.status === 'recording') {
            // Expand window to show transcript
            indicatorWindow.setSize(320, 180);
          } else if (message.status === 'ready' || message.status === 'error') {
            // Shrink back to normal size
            indicatorWindow.setSize(200, 56);
          }
        }

        // Update local recording state
        if (message.is_recording !== undefined) {
          isRecording = message.is_recording;
        }

        // If transcription completed, notify main window to refresh history
        if (message.transcription) {
          mainWindow?.webContents.send('history-update');
          indicatorWindow?.webContents.send('paste-text', message.transcription);
        }
      } else if (message.type === 'partial_transcript') {
        // Send partial transcript to indicator window for live display
        indicatorWindow?.webContents.send('partial-transcript', message.text);
      }
    } catch (e) {
      console.error('Failed to parse WebSocket message:', e);
    }
  });

  ws.on('close', () => {
    console.log('WebSocket disconnected');
    ws = null;
    // Reconnect after a short delay
    if (!app.isQuitting) {
      wsReconnectTimer = setTimeout(connectWebSocket, 2000);
    }
  });

  ws.on('error', (error) => {
    console.error('WebSocket error:', error.message);
  });
}

// === Recording Control ===

async function startRecording() {
  if (isRecording) return;

  const result = await apiCall('/recording/start', 'POST');
  if (result && result.recording) {
    isRecording = true;
    indicatorWindow?.webContents.send('status-update', 'recording');
    console.log('Recording started');
  }
}

async function stopRecording() {
  if (!isRecording) return;

  indicatorWindow?.webContents.send('status-update', 'transcribing');

  const result = await apiCall('/recording/stop', 'POST');
  isRecording = false;

  if (result && result.success && result.text) {
    console.log('Transcribed:', result.text);

    // Copy to clipboard first
    clipboard.writeText(result.text);

    // Notify renderer
    indicatorWindow?.webContents.send('paste-text', result.text);

    // Notify main window to refresh history
    mainWindow?.webContents.send('history-update');

    // Call Python to simulate Ctrl+V paste
    setTimeout(async () => {
      await apiCall('/paste', 'POST');
    }, 100);
  }

  indicatorWindow?.webContents.send('status-update', 'ready');
}

async function cancelRecording() {
  if (!isRecording) return;

  await apiCall('/recording/cancel', 'POST');
  isRecording = false;
  indicatorWindow?.webContents.send('status-update', 'ready');
}

// === Hotkey Management ===

function isModifierOnly(hotkeyArray) {
  if (!Array.isArray(hotkeyArray) || hotkeyArray.length === 0) return false;

  const modifiers = ['ctrl', 'ctrl_l', 'ctrl_r', 'shift', 'shift_l', 'shift_r',
                     'alt', 'alt_l', 'alt_r', 'cmd', 'cmd_l', 'cmd_r'];

  return hotkeyArray.every(key => modifiers.includes(key.toLowerCase()));
}

function formatHotkey(hotkeyArray) {
  // Convert our format to Electron accelerator format
  const keyMap = {
    'ctrl': 'Ctrl',
    'ctrl_l': 'Ctrl',
    'ctrl_r': 'Ctrl',
    'shift': 'Shift',
    'shift_l': 'Shift',
    'shift_r': 'Shift',
    'alt': 'Alt',
    'alt_l': 'Alt',
    'alt_r': 'Alt',
    'cmd': 'Super',
    'cmd_l': 'Super',
    'cmd_r': 'Super',
    'space': 'Space',
    'enter': 'Enter',
    'tab': 'Tab',
    'escape': 'Escape',
    'esc': 'Escape'
  };

  if (!Array.isArray(hotkeyArray) || hotkeyArray.length === 0) {
    return 'Ctrl+Shift+Space';
  }

  const mapped = hotkeyArray.map(key => {
    const lowerKey = key.toLowerCase();
    return keyMap[lowerKey] || key.charAt(0).toUpperCase() + key.slice(1);
  });

  return mapped.join('+');
}

let usePythonHotkey = false;

function registerHotkeys() {
  // Unregister all first
  globalShortcut.unregisterAll();
  usePythonHotkey = false;

  console.log('Config hotkey value:', config.hotkey);

  // Check if hotkey is modifier-only (Electron can't handle these)
  if (isModifierOnly(config.hotkey)) {
    console.log('Modifier-only hotkey detected, using Python for detection');
    usePythonHotkey = true;
    // Tell Python to handle hotkey detection
    apiCall('/hotkey/enable', 'POST', {
      hotkey: config.hotkey,
      mode: config.activation_mode
    });
    return;
  }

  // Disable Python hotkey detection
  apiCall('/hotkey/disable', 'POST');

  const accelerator = formatHotkey(config.hotkey);
  console.log('Registering hotkey accelerator:', accelerator);

  try {
    let success = false;

    if (config.activation_mode === 'toggle') {
      success = globalShortcut.register(accelerator, () => {
        console.log('Hotkey pressed! (toggle mode)');
        if (isRecording) {
          stopRecording();
        } else {
          startRecording();
        }
      });
    } else {
      success = globalShortcut.register(accelerator, () => {
        console.log('Hotkey pressed! (push-to-talk mode)');
        if (!hotkeyPressed) {
          hotkeyPressed = true;
          startRecording();
        }
      });
    }

    if (success) {
      console.log('Hotkey registered successfully:', accelerator);
    } else {
      console.error('Failed to register hotkey:', accelerator);
      // Fall back to Python
      console.log('Falling back to Python hotkey detection');
      usePythonHotkey = true;
      apiCall('/hotkey/enable', 'POST', {
        hotkey: config.hotkey,
        mode: config.activation_mode
      });
    }

  } catch (error) {
    console.error('Error registering hotkey:', error.message);
    // Fall back to Python
    usePythonHotkey = true;
    apiCall('/hotkey/enable', 'POST', {
      hotkey: config.hotkey,
      mode: config.activation_mode
    });
  }
}

// === Window Management ===

function createIndicatorWindow() {
  indicatorWindow = new BrowserWindow({
    width: 200,
    height: 56,
    x: config.window_position[0],
    y: config.window_position[1],
    frame: false,
    transparent: true,
    backgroundColor: '#00000000',
    alwaysOnTop: true,
    skipTaskbar: true,
    resizable: true,  // Allow programmatic resize
    hasShadow: false,
    focusable: false,  // Prevents stealing focus from other apps
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false
    }
  });

  // Set to screen-saver level (highest, truly always on top)
  indicatorWindow.setAlwaysOnTop(true, 'screen-saver');

  indicatorWindow.loadFile(path.join(__dirname, 'index.html'));

  // Save position when moved
  indicatorWindow.on('moved', () => {
    const [x, y] = indicatorWindow.getPosition();
    config.window_position = [x, y];
    apiCall('/config/position', 'POST', { x, y });
  });

  indicatorWindow.on('closed', () => {
    indicatorWindow = null;
  });

  // Periodically re-assert always on top (Windows sometimes loses it)
  setInterval(() => {
    if (indicatorWindow && !indicatorWindow.isDestroyed()) {
      indicatorWindow.setAlwaysOnTop(true, 'screen-saver');
    }
  }, 5000);
}

function createMainWindow() {
  mainWindow = new BrowserWindow({
    width: 800,
    height: 600,
    minWidth: 600,
    minHeight: 400,
    frame: false,
    show: true,  // Show on startup
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false
    }
  });

  mainWindow.loadFile(path.join(__dirname, 'main-window.html'));

  mainWindow.on('closed', () => {
    mainWindow = null;
    // Quit the entire app when main window is closed
    app.quit();
  });
}

function createTray() {
  // Skip tray for now - no icon available
  // Users can use the main window directly
  console.log('Tray icon skipped (no icon file)');
}

// === IPC Handlers ===

function setupIPC() {
  // Status
  ipcMain.handle('get-status', async () => {
    return await apiCall('/status');
  });

  // Config
  ipcMain.handle('get-config', async () => {
    const result = await apiCall('/config');
    if (result) {
      config = result;
    }
    return config;
  });

  ipcMain.handle('save-config', async (event, newConfig) => {
    config = { ...config, ...newConfig };
    await apiCall('/config', 'POST', config);
    registerHotkeys();
    // Notify indicator window to update its display
    indicatorWindow?.webContents.send('config-update', config);
    return config;
  });

  // Audio devices
  ipcMain.handle('get-audio-devices', async () => {
    return await apiCall('/audio-devices');
  });

  // Recording
  ipcMain.handle('start-recording', async () => {
    await startRecording();
  });

  ipcMain.handle('stop-recording', async () => {
    await stopRecording();
  });

  // History
  ipcMain.handle('get-history', async (event, limit, offset) => {
    let endpoint = '/history';
    const params = [];
    if (limit) params.push(`limit=${limit}`);
    if (offset) params.push(`offset=${offset}`);
    if (params.length) endpoint += '?' + params.join('&');
    return await apiCall(endpoint);
  });

  ipcMain.handle('clear-history', async () => {
    return await apiCall('/history', 'DELETE');
  });

  ipcMain.handle('delete-history-entry', async (event, id) => {
    return await apiCall(`/history/${id}`, 'DELETE');
  });

  // Replacements
  ipcMain.handle('get-replacements', async () => {
    return await apiCall('/replacements');
  });

  ipcMain.handle('add-replacement', async (event, data) => {
    return await apiCall('/replacements', 'POST', data);
  });

  ipcMain.handle('update-replacement', async (event, id, data) => {
    return await apiCall(`/replacements/${id}`, 'PUT', data);
  });

  ipcMain.handle('delete-replacement', async (event, id) => {
    return await apiCall(`/replacements/${id}`, 'DELETE');
  });

  // Window controls
  ipcMain.on('minimize-main-window', () => {
    mainWindow?.minimize();
  });

  ipcMain.on('hide-main-window', () => {
    mainWindow?.hide();
  });

  ipcMain.on('show-main-window', () => {
    mainWindow?.show();
    mainWindow?.focus();
  });

  ipcMain.on('close-app', () => {
    app.quit();
  });

  // For push-to-talk key release detection
  ipcMain.on('hotkey-released', () => {
    if (hotkeyPressed && config.activation_mode === 'push_to_talk') {
      hotkeyPressed = false;
      stopRecording();
    }
  });
}

// === App Lifecycle ===

app.whenReady().then(async () => {
  // Start Python API server
  await startPythonAPI();

  // Wait for API to be ready
  const apiReady = await waitForAPI();

  if (!apiReady) {
    console.error('Failed to start API server');
    app.quit();
    return;
  }

  // Load config from API
  const loadedConfig = await apiCall('/config');
  console.log('Loaded config from API:', JSON.stringify(loadedConfig, null, 2));
  if (loadedConfig) {
    config = loadedConfig;
  }
  console.log('Using config:', JSON.stringify(config, null, 2));
  console.log('Hotkey array:', config.hotkey);

  // Setup IPC
  setupIPC();

  // Create UI
  createMainWindow();
  createIndicatorWindow();
  createTray();

  // Startup complete - now window-all-closed can quit the app
  isStartingUp = false;

  // Register hotkeys
  registerHotkeys();

  // Connect WebSocket for real-time status updates
  connectWebSocket();
});

// Track if we're still in startup phase
let isStartingUp = true;

app.on('window-all-closed', () => {
  // Don't quit during startup (setup window closing before main window opens)
  if (!isStartingUp) {
    app.quit();
  }
});

app.on('before-quit', () => {
  app.isQuitting = true;
});

app.on('will-quit', () => {
  globalShortcut.unregisterAll();
  if (wsReconnectTimer) {
    clearTimeout(wsReconnectTimer);
  }
  if (ws) {
    ws.close();
  }
  stopPythonAPI();
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createMainWindow();
    createIndicatorWindow();
  }
});
