// FreeFlow Main Window Renderer

// DOM Elements - Tabs
const navTabs = document.querySelectorAll('.nav-tab');
const tabPanels = document.querySelectorAll('.tab-panel');

// DOM Elements - History
const historyList = document.getElementById('history-list');
const statsTotal = document.getElementById('stats-total');
const statsWords = document.getElementById('stats-words');
const statsChars = document.getElementById('stats-chars');
const btnClearHistory = document.getElementById('btn-clear-history');

// DOM Elements - Replacements
const replacementsList = document.getElementById('replacements-list');
const btnAddReplacement = document.getElementById('btn-add-replacement');
const replacementModal = document.getElementById('replacement-modal');
const modalTitle = document.getElementById('modal-title');
const closeModal = document.getElementById('close-modal');
const btnSaveReplacement = document.getElementById('btn-save-replacement');
const btnCancelReplacement = document.getElementById('btn-cancel-replacement');
const inputFind = document.getElementById('input-find');
const inputReplace = document.getElementById('input-replace');
const inputWholeWord = document.getElementById('input-whole-word');
const inputCaseSensitive = document.getElementById('input-case-sensitive');

// DOM Elements - Settings
const settingsHotkeyInput = document.getElementById('settings-hotkey-input');
const settingsCurrentHotkey = document.getElementById('settings-current-hotkey');
const settingsAudioDevice = document.getElementById('settings-audio-device');
const btnSaveSettings = document.getElementById('btn-save-settings');

// DOM Elements - Confirm Modal
const confirmModal = document.getElementById('confirm-modal');
const confirmMessage = document.getElementById('confirm-message');
const btnConfirmYes = document.getElementById('btn-confirm-yes');
const btnConfirmNo = document.getElementById('btn-confirm-no');

// DOM Elements - Titlebar
const btnMinimize = document.getElementById('btn-minimize');
const btnClose = document.getElementById('btn-close');

// State
let config = {};
let editingReplacementId = null;
let isRecordingHotkey = false;
let recordedKeys = new Set();
let tempHotkey = [];
let confirmCallback = null;

// === Tab Navigation ===

navTabs.forEach(tab => {
  tab.addEventListener('click', () => {
    const tabId = tab.dataset.tab;

    navTabs.forEach(t => t.classList.remove('active'));
    tabPanels.forEach(p => p.classList.remove('active'));

    tab.classList.add('active');
    document.getElementById(`tab-${tabId}`).classList.add('active');

    // Load data when switching tabs
    if (tabId === 'history') loadHistory();
    if (tabId === 'replacements') loadReplacements();
    if (tabId === 'settings') loadSettings();
  });
});

// === History Functions ===

async function loadHistory() {
  try {
    const data = await window.freeflow.getHistory();
    renderHistory(data.entries, data.stats);
  } catch (error) {
    console.error('Failed to load history:', error);
  }
}

function renderHistory(entries, stats) {
  // Update stats
  statsTotal.textContent = `${stats.total_entries} transcriptions`;
  statsWords.textContent = `${(stats.total_words || 0).toLocaleString()} words`;
  statsChars.textContent = `${stats.total_characters.toLocaleString()} characters`;

  if (entries.length === 0) {
    historyList.innerHTML = '<div class="empty-state">No transcriptions yet</div>';
    return;
  }

  historyList.innerHTML = entries.map(entry => {
    const date = new Date(entry.timestamp);
    const timeStr = date.toLocaleString();
    const durationStr = entry.duration_seconds
      ? `${entry.duration_seconds.toFixed(1)}s`
      : '';

    const showOriginal = entry.original_text !== entry.final_text;

    return `
      <div class="history-item" data-id="${entry.id}">
        <div class="history-item-header">
          <span class="history-timestamp">${timeStr}</span>
          ${durationStr ? `<span class="history-duration">${durationStr}</span>` : ''}
        </div>
        <div class="history-text">${escapeHtml(entry.final_text)}</div>
        ${showOriginal ? `
          <div class="history-original">
            <div class="history-original-label">Original:</div>
            ${escapeHtml(entry.original_text)}
          </div>
        ` : ''}
        <div class="history-actions">
          <button class="btn btn-secondary btn-sm btn-copy" data-text="${escapeHtml(entry.final_text).replace(/"/g, '&quot;')}">Copy</button>
          <button class="btn btn-icon btn-delete-history" data-id="${entry.id}">Delete</button>
        </div>
      </div>
    `;
  }).join('');

  // Attach event listeners
  historyList.querySelectorAll('.btn-copy').forEach(btn => {
    btn.addEventListener('click', () => copyToClipboard(btn.dataset.text));
  });
  historyList.querySelectorAll('.btn-delete-history').forEach(btn => {
    btn.addEventListener('click', () => deleteHistoryEntry(parseInt(btn.dataset.id)));
  });
}

async function deleteHistoryEntry(id) {
  showConfirm('Are you sure you want to delete this transcription?', async () => {
    try {
      await window.freeflow.deleteHistoryEntry(id);
      loadHistory();
    } catch (error) {
      console.error('Failed to delete history entry:', error);
    }
  });
}

async function clearAllHistory() {
  showConfirm('Are you sure you want to delete all transcription history?', async () => {
    try {
      await window.freeflow.clearHistory();
      loadHistory();
    } catch (error) {
      console.error('Failed to clear history:', error);
    }
  });
}

btnClearHistory.addEventListener('click', clearAllHistory);

// === Replacements Functions ===

async function loadReplacements() {
  try {
    const data = await window.freeflow.getReplacements();
    renderReplacements(data.replacements);
  } catch (error) {
    console.error('Failed to load replacements:', error);
  }
}

function renderReplacements(replacements) {
  if (replacements.length === 0) {
    replacementsList.innerHTML = '<div class="empty-state">No replacement rules yet</div>';
    return;
  }

  replacementsList.innerHTML = replacements.map(rule => {
    const options = [];
    if (rule.whole_word) options.push('Whole word');
    if (rule.case_sensitive) options.push('Case sensitive');

    return `
      <div class="replacement-item ${rule.enabled ? '' : 'disabled'}" data-id="${rule.id}">
        <div class="replacement-toggle">
          <label class="toggle-switch">
            <input type="checkbox" class="replacement-toggle-input" ${rule.enabled ? 'checked' : ''} data-id="${rule.id}">
            <span class="toggle-slider"></span>
          </label>
        </div>
        <div class="replacement-content">
          <div class="replacement-rule">
            <span class="replacement-find">${escapeHtml(rule.find)}</span>
            <span class="replacement-arrow">-></span>
            <span class="replacement-replace">${escapeHtml(rule.replace)}</span>
          </div>
          ${options.length ? `
            <div class="replacement-options">
              ${options.map(opt => `<span class="replacement-option">${opt}</span>`).join('')}
            </div>
          ` : ''}
        </div>
        <div class="replacement-actions">
          <button class="btn btn-icon btn-edit" data-id="${rule.id}">Edit</button>
          <button class="btn btn-icon btn-delete" data-id="${rule.id}">Delete</button>
        </div>
      </div>
    `;
  }).join('');

  // Attach event listeners using delegation
  const editBtns = replacementsList.querySelectorAll('.btn-edit');
  const deleteBtns = replacementsList.querySelectorAll('.btn-delete');
  const toggleInputs = replacementsList.querySelectorAll('.replacement-toggle-input');

  console.log('Attaching listeners:', editBtns.length, 'edit,', deleteBtns.length, 'delete,', toggleInputs.length, 'toggle');

  editBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      console.log('Edit clicked:', btn.dataset.id);
      editReplacement(btn.dataset.id);
    });
  });
  deleteBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      console.log('Delete clicked:', btn.dataset.id);
      deleteReplacement(btn.dataset.id);
    });
  });
  toggleInputs.forEach(input => {
    input.addEventListener('change', () => {
      console.log('Toggle changed:', input.dataset.id, input.checked);
      toggleReplacement(input.dataset.id, input.checked);
    });
  });
}

function openReplacementModal(rule = null) {
  console.log('openReplacementModal called with rule:', rule);
  console.log('replacementModal element:', replacementModal);
  if (rule) {
    modalTitle.textContent = 'Edit Replacement Rule';
    editingReplacementId = rule.id;
    inputFind.value = rule.find;
    inputReplace.value = rule.replace;
    inputWholeWord.checked = rule.whole_word;
    inputCaseSensitive.checked = rule.case_sensitive;
  } else {
    modalTitle.textContent = 'Add Replacement Rule';
    editingReplacementId = null;
    inputFind.value = '';
    inputReplace.value = '';
    inputWholeWord.checked = true;
    inputCaseSensitive.checked = false;
  }
  console.log('Removing hidden class from modal');
  replacementModal.classList.remove('hidden');
  console.log('Modal classList after:', replacementModal.classList);
  inputFind.focus();
}

function closeReplacementModal() {
  replacementModal.classList.add('hidden');
  editingReplacementId = null;
}

async function saveReplacement() {
  const find = inputFind.value.trim();
  const replace = inputReplace.value.trim();

  if (!find) {
    inputFind.focus();
    return;
  }

  const data = {
    find,
    replace,
    whole_word: inputWholeWord.checked,
    case_sensitive: inputCaseSensitive.checked,
    enabled: true
  };

  try {
    if (editingReplacementId) {
      await window.freeflow.updateReplacement(editingReplacementId, data);
    } else {
      await window.freeflow.addReplacement(data);
    }
    closeReplacementModal();
    loadReplacements();
  } catch (error) {
    console.error('Failed to save replacement:', error);
  }
}

async function editReplacement(id) {
  console.log('editReplacement called with id:', id);
  try {
    const data = await window.freeflow.getReplacements();
    console.log('Got replacements data:', data);
    const rule = data.replacements.find(r => r.id === id);
    console.log('Found rule:', rule);
    if (rule) {
      openReplacementModal(rule);
    }
  } catch (error) {
    console.error('Failed to get replacement:', error);
  }
}

async function toggleReplacement(id, enabled) {
  try {
    await window.freeflow.updateReplacement(id, { enabled });
  } catch (error) {
    console.error('Failed to toggle replacement:', error);
    loadReplacements(); // Refresh to show actual state
  }
}

async function deleteReplacement(id) {
  showConfirm('Are you sure you want to delete this replacement rule?', async () => {
    try {
      await window.freeflow.deleteReplacement(id);
      loadReplacements();
    } catch (error) {
      console.error('Failed to delete replacement:', error);
    }
  });
}

btnAddReplacement.addEventListener('click', () => openReplacementModal());
closeModal.addEventListener('click', closeReplacementModal);
btnCancelReplacement.addEventListener('click', closeReplacementModal);
btnSaveReplacement.addEventListener('click', saveReplacement);

// Close modal on escape key
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    if (!replacementModal.classList.contains('hidden')) {
      closeReplacementModal();
    }
    if (!confirmModal.classList.contains('hidden')) {
      closeConfirm();
    }
  }
});

// === Settings Functions ===

async function loadSettings() {
  try {
    config = await window.freeflow.getConfig();

    // Update hotkey display
    settingsCurrentHotkey.textContent = formatHotkeyDisplay(config.hotkey);
    tempHotkey = [...config.hotkey];

    // Update activation mode
    const modeRadio = document.querySelector(`input[name="settings-mode"][value="${config.activation_mode}"]`);
    if (modeRadio) modeRadio.checked = true;

    // Load and update audio devices
    const devices = await window.freeflow.getAudioDevices();
    settingsAudioDevice.innerHTML = '<option value="">Default</option>';
    if (devices && devices.devices) {
      devices.devices.forEach(device => {
        const option = document.createElement('option');
        option.value = device.index;
        option.textContent = device.name;
        settingsAudioDevice.appendChild(option);
      });
    }

    if (config.audio_device !== null) {
      settingsAudioDevice.value = config.audio_device;
    }
  } catch (error) {
    console.error('Failed to load settings:', error);
  }
}

async function saveSettings() {
  const mode = document.querySelector('input[name="settings-mode"]:checked').value;
  const audioDevice = settingsAudioDevice.value;

  if (tempHotkey.length === 0) {
    alert('Please set a hotkey');
    return;
  }

  config.hotkey = tempHotkey;
  config.activation_mode = mode;
  config.audio_device = audioDevice === '' ? null : parseInt(audioDevice);

  try {
    await window.freeflow.saveConfig(config);
    settingsCurrentHotkey.textContent = formatHotkeyDisplay(config.hotkey);
    console.log('Settings saved successfully');
  } catch (error) {
    console.error('Failed to save settings:', error);
  }
}

btnSaveSettings.addEventListener('click', saveSettings);

// === Hotkey Recording ===

function formatHotkeyDisplay(hotkeyArray) {
  const keyMap = {
    'ctrl_l': 'LCtrl',
    'ctrl_r': 'RCtrl',
    'ctrl': 'Ctrl',
    'shift_l': 'LShift',
    'shift_r': 'RShift',
    'shift': 'Shift',
    'alt_l': 'LAlt',
    'alt_r': 'RAlt',
    'alt': 'Alt',
    'cmd': 'Cmd',
    'cmd_l': 'LCmd',
    'cmd_r': 'RCmd',
    'space': 'Space'
  };

  return hotkeyArray
    .map(key => keyMap[key.toLowerCase()] || key.charAt(0).toUpperCase() + key.slice(1))
    .join(' + ');
}

function keyToString(event) {
  const code = event.code;

  if (code === 'ControlLeft') return 'ctrl_l';
  if (code === 'ControlRight') return 'ctrl_r';
  if (code === 'ShiftLeft') return 'shift_l';
  if (code === 'ShiftRight') return 'shift_r';
  if (code === 'AltLeft') return 'alt_l';
  if (code === 'AltRight') return 'alt_r';
  if (code === 'MetaLeft') return 'cmd_l';
  if (code === 'MetaRight') return 'cmd_r';
  if (code === 'Space') return 'space';

  return event.key.toLowerCase();
}

settingsHotkeyInput.addEventListener('click', () => {
  if (!isRecordingHotkey) {
    isRecordingHotkey = true;
    recordedKeys.clear();
    tempHotkey = [];
    settingsHotkeyInput.classList.add('recording');
    settingsCurrentHotkey.textContent = 'Press keys...';
  }
});

settingsHotkeyInput.addEventListener('keydown', (event) => {
  if (!isRecordingHotkey) return;

  event.preventDefault();
  event.stopPropagation();

  const keyStr = keyToString(event);
  if (!recordedKeys.has(keyStr)) {
    recordedKeys.add(keyStr);
    tempHotkey.push(keyStr);
    settingsCurrentHotkey.textContent = formatHotkeyDisplay(tempHotkey);
  }
});

settingsHotkeyInput.addEventListener('keyup', (event) => {
  if (!isRecordingHotkey) return;

  event.preventDefault();

  setTimeout(() => {
    if (recordedKeys.size > 0) {
      isRecordingHotkey = false;
      settingsHotkeyInput.classList.remove('recording');
    }
  }, 100);
});

settingsHotkeyInput.addEventListener('blur', () => {
  if (isRecordingHotkey) {
    isRecordingHotkey = false;
    settingsHotkeyInput.classList.remove('recording');

    if (tempHotkey.length === 0) {
      tempHotkey = [...config.hotkey];
      settingsCurrentHotkey.textContent = formatHotkeyDisplay(config.hotkey);
    }
  }
});

// === Confirm Modal ===

function showConfirm(message, callback) {
  confirmMessage.textContent = message;
  confirmCallback = callback;
  confirmModal.classList.remove('hidden');
}

function closeConfirm() {
  confirmModal.classList.add('hidden');
  confirmCallback = null;
}

btnConfirmYes.addEventListener('click', () => {
  if (confirmCallback) {
    confirmCallback();
  }
  closeConfirm();
});

btnConfirmNo.addEventListener('click', closeConfirm);

// === Window Controls ===

btnMinimize.addEventListener('click', () => {
  window.freeflow.minimizeMainWindow();
});

btnClose.addEventListener('click', () => {
  window.freeflow.closeApp();
});

// === Utilities ===

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function copyToClipboard(text) {
  navigator.clipboard.writeText(text).catch(err => {
    console.error('Failed to copy:', err);
  });
}

// Note: Event listeners are now attached directly in render functions
// No need for global window exports since we don't use inline onclick handlers

// === Initialization ===

async function init() {
  console.log('Main window renderer initializing...');
  console.log('replacementModal element:', replacementModal);
  console.log('confirmModal element:', confirmModal);
  await loadHistory();
  console.log('Main window renderer initialized');
}

init();
