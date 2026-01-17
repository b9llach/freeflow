// FreeFlow Renderer Process

// DOM Elements
const statusIndicator = document.getElementById('status-indicator');
const statusText = document.getElementById('status-text');
const hotkeyText = document.getElementById('hotkey-text');
const settingsModal = document.getElementById('settings-modal');
const closeSettingsBtn = document.getElementById('close-settings');
const saveSettingsBtn = document.getElementById('save-settings');
const cancelSettingsBtn = document.getElementById('cancel-settings');
const hotkeyInput = document.getElementById('hotkey-input');
const currentHotkeySpan = document.getElementById('current-hotkey');
const audioDeviceSelect = document.getElementById('audio-device');

// State
let config = {
  hotkey: ['ctrl_l', 'shift_l', 'space'],
  activation_mode: 'push_to_talk',
  window_position: [100, 100],
  audio_device: null
};

let isRecordingHotkey = false;
let recordedKeys = new Set();
let tempHotkey = [];

// === Status Display ===

function formatHotkeyDisplay(hotkeyArray) {
  const keyMap = {
    'ctrl_l': 'LCtrl',
    'ctrl_r': 'RCtrl',
    'shift_l': 'LShift',
    'shift_r': 'RShift',
    'alt_l': 'LAlt',
    'alt_r': 'RAlt',
    'cmd': 'Cmd',
    'cmd_l': 'LCmd',
    'cmd_r': 'RCmd',
    'space': 'Space'
  };

  return hotkeyArray
    .map(key => keyMap[key.toLowerCase()] || key.charAt(0).toUpperCase() + key.slice(1))
    .join(' + ');
}

function getModeLabel(mode) {
  return mode === 'push_to_talk' ? 'hold' : 'toggle';
}

function updateStatusDisplay(status) {
  // Update indicator class
  statusIndicator.className = 'status-indicator ' + status;

  // Update status text
  const statusLabels = {
    'initializing': 'Initializing...',
    'loading': 'Loading Model...',
    'ready': 'Ready',
    'recording': 'Recording',
    'transcribing': 'Transcribing...',
    'error': 'Error'
  };

  statusText.textContent = statusLabels[status] || status;
}

function updateHotkeyDisplay() {
  const hotkeyStr = formatHotkeyDisplay(config.hotkey);
  const modeLabel = getModeLabel(config.activation_mode);
  hotkeyText.textContent = `${hotkeyStr} (${modeLabel})`;
}

// === Configuration ===

async function loadConfig() {
  try {
    const loadedConfig = await window.freeflow.getConfig();
    if (loadedConfig) {
      config = loadedConfig;
      updateHotkeyDisplay();
    }
  } catch (error) {
    console.error('Failed to load config:', error);
  }
}

async function loadAudioDevices() {
  try {
    const result = await window.freeflow.getAudioDevices();
    if (result && result.devices) {
      audioDeviceSelect.innerHTML = '<option value="">Default</option>';
      result.devices.forEach(device => {
        const option = document.createElement('option');
        option.value = device.index;
        option.textContent = device.name;
        audioDeviceSelect.appendChild(option);
      });

      // Set current selection
      if (config.audio_device !== null) {
        audioDeviceSelect.value = config.audio_device;
      }
    }
  } catch (error) {
    console.error('Failed to load audio devices:', error);
  }
}

// === Settings Modal ===

function openSettings() {
  // Resize window for settings
  // Note: This would need to be handled by the main process

  // Load current values
  currentHotkeySpan.textContent = formatHotkeyDisplay(config.hotkey);
  tempHotkey = [...config.hotkey];

  // Set activation mode
  const modeRadio = document.querySelector(`input[name="mode"][value="${config.activation_mode}"]`);
  if (modeRadio) {
    modeRadio.checked = true;
  }

  // Set audio device
  if (config.audio_device !== null) {
    audioDeviceSelect.value = config.audio_device;
  } else {
    audioDeviceSelect.value = '';
  }

  // Show modal
  settingsModal.classList.remove('hidden');

  // Load audio devices
  loadAudioDevices();
}

function closeSettings() {
  settingsModal.classList.add('hidden');
  isRecordingHotkey = false;
  hotkeyInput.classList.remove('recording');
}

async function saveSettings() {
  // Get values from form
  const selectedMode = document.querySelector('input[name="mode"]:checked').value;
  const selectedDevice = audioDeviceSelect.value;

  // Update config
  config.hotkey = tempHotkey;
  config.activation_mode = selectedMode;
  config.audio_device = selectedDevice === '' ? null : parseInt(selectedDevice);

  // Save to backend
  try {
    await window.freeflow.saveConfig(config);
    updateHotkeyDisplay();
    closeSettings();
  } catch (error) {
    console.error('Failed to save config:', error);
  }
}

// === Hotkey Recording ===

function startHotkeyRecording() {
  isRecordingHotkey = true;
  recordedKeys.clear();
  tempHotkey = [];
  hotkeyInput.classList.add('recording');
  currentHotkeySpan.textContent = 'Press keys...';
}

function stopHotkeyRecording() {
  isRecordingHotkey = false;
  hotkeyInput.classList.remove('recording');

  if (tempHotkey.length > 0) {
    currentHotkeySpan.textContent = formatHotkeyDisplay(tempHotkey);
  } else {
    currentHotkeySpan.textContent = formatHotkeyDisplay(config.hotkey);
    tempHotkey = [...config.hotkey];
  }
}

function keyToString(event) {
  // Map key events to our format
  const key = event.key.toLowerCase();
  const code = event.code;

  // Handle modifiers with left/right distinction
  if (code === 'ControlLeft') return 'ctrl_l';
  if (code === 'ControlRight') return 'ctrl_r';
  if (code === 'ShiftLeft') return 'shift_l';
  if (code === 'ShiftRight') return 'shift_r';
  if (code === 'AltLeft') return 'alt_l';
  if (code === 'AltRight') return 'alt_r';
  if (code === 'MetaLeft') return 'cmd_l';
  if (code === 'MetaRight') return 'cmd_r';
  if (code === 'Space') return 'space';

  // For other keys, use the key value
  return key;
}

// === Event Listeners ===

// Settings button events
closeSettingsBtn.addEventListener('click', closeSettings);
cancelSettingsBtn.addEventListener('click', closeSettings);
saveSettingsBtn.addEventListener('click', saveSettings);

// Hotkey input events
hotkeyInput.addEventListener('click', () => {
  if (!isRecordingHotkey) {
    startHotkeyRecording();
  }
});

hotkeyInput.addEventListener('keydown', (event) => {
  if (!isRecordingHotkey) return;

  event.preventDefault();
  event.stopPropagation();

  const keyStr = keyToString(event);
  if (!recordedKeys.has(keyStr)) {
    recordedKeys.add(keyStr);
    tempHotkey.push(keyStr);
    currentHotkeySpan.textContent = formatHotkeyDisplay(tempHotkey);
  }
});

hotkeyInput.addEventListener('keyup', (event) => {
  if (!isRecordingHotkey) return;

  event.preventDefault();
  event.stopPropagation();

  // Stop recording when all keys are released
  setTimeout(() => {
    if (recordedKeys.size > 0) {
      stopHotkeyRecording();
    }
  }, 100);
});

hotkeyInput.addEventListener('blur', () => {
  if (isRecordingHotkey) {
    stopHotkeyRecording();
  }
});

// Context menu for settings
document.addEventListener('contextmenu', (event) => {
  event.preventDefault();
  openSettings();
});

// === IPC Listeners ===

// Status updates from main process
window.freeflow.onStatusUpdate((status) => {
  updateStatusDisplay(status);
});

// Open settings from tray menu
window.freeflow.onOpenSettings(() => {
  openSettings();
});

// Config updated from main window
window.freeflow.onConfigUpdate((newConfig) => {
  config = newConfig;
  updateHotkeyDisplay();
});

// Paste text notification (for potential UI feedback)
window.freeflow.onPasteText((text) => {
  console.log('Transcribed text:', text);
  // Could add a toast notification here
});

// === Push-to-Talk Key Release Detection ===
// Note: This is a workaround since Electron's globalShortcut doesn't detect key release
// The renderer monitors keyboard events and notifies main when hotkey is released

let hotkeyKeysDown = new Set();

document.addEventListener('keydown', (event) => {
  if (settingsModal.classList.contains('hidden')) {
    const keyStr = keyToString(event);
    hotkeyKeysDown.add(keyStr);
  }
});

document.addEventListener('keyup', (event) => {
  if (settingsModal.classList.contains('hidden')) {
    const keyStr = keyToString(event);
    hotkeyKeysDown.delete(keyStr);

    // Check if any hotkey key was released
    if (config.activation_mode === 'push_to_talk') {
      const hotkeySet = new Set(config.hotkey.map(k => k.toLowerCase()));
      if (hotkeySet.has(keyStr.toLowerCase())) {
        window.freeflow.hotkeyReleased();
      }
    }
  }
});

// Also listen for window blur to handle key release when window loses focus
window.addEventListener('blur', () => {
  if (config.activation_mode === 'push_to_talk' && hotkeyKeysDown.size > 0) {
    window.freeflow.hotkeyReleased();
    hotkeyKeysDown.clear();
  }
});

// === Initialization ===

async function init() {
  updateStatusDisplay('initializing');

  // Load configuration
  await loadConfig();

  // Get initial status
  try {
    const status = await window.freeflow.getStatus();
    if (status) {
      updateStatusDisplay(status.status);
    }
  } catch (error) {
    console.error('Failed to get status:', error);
  }
}

// Start the app
init();
