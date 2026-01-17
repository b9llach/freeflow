const { contextBridge, ipcRenderer } = require('electron');

// Expose protected methods to the renderer process
contextBridge.exposeInMainWorld('freeflow', {
  // Status
  getStatus: () => ipcRenderer.invoke('get-status'),

  // Config
  getConfig: () => ipcRenderer.invoke('get-config'),
  saveConfig: (config) => ipcRenderer.invoke('save-config', config),

  // Audio devices
  getAudioDevices: () => ipcRenderer.invoke('get-audio-devices'),

  // Recording
  startRecording: () => ipcRenderer.invoke('start-recording'),
  stopRecording: () => ipcRenderer.invoke('stop-recording'),

  // History
  getHistory: (limit, offset) => ipcRenderer.invoke('get-history', limit, offset),
  clearHistory: () => ipcRenderer.invoke('clear-history'),
  deleteHistoryEntry: (id) => ipcRenderer.invoke('delete-history-entry', id),

  // Replacements
  getReplacements: () => ipcRenderer.invoke('get-replacements'),
  addReplacement: (data) => ipcRenderer.invoke('add-replacement', data),
  updateReplacement: (id, data) => ipcRenderer.invoke('update-replacement', id, data),
  deleteReplacement: (id) => ipcRenderer.invoke('delete-replacement', id),

  // Window controls
  minimizeMainWindow: () => ipcRenderer.send('minimize-main-window'),
  hideMainWindow: () => ipcRenderer.send('hide-main-window'),
  showMainWindow: () => ipcRenderer.send('show-main-window'),
  closeApp: () => ipcRenderer.send('close-app'),

  // Hotkey release (for push-to-talk)
  hotkeyReleased: () => ipcRenderer.send('hotkey-released'),

  // Event listeners
  onStatusUpdate: (callback) => {
    ipcRenderer.on('status-update', (event, status) => callback(status));
  },

  onPasteText: (callback) => {
    ipcRenderer.on('paste-text', (event, text) => callback(text));
  },

  onOpenSettings: (callback) => {
    ipcRenderer.on('open-settings', () => callback());
  },

  onHistoryUpdate: (callback) => {
    ipcRenderer.on('history-update', () => callback());
  },

  onConfigUpdate: (callback) => {
    ipcRenderer.on('config-update', (event, config) => callback(config));
  }
});
