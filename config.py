"""Configuration management for FreeFlow."""

import json
import os
from pathlib import Path
from typing import Any, Dict, List, Optional


# Activation modes
MODE_PUSH_TO_TALK = "push_to_talk"
MODE_TOGGLE = "toggle"

DEFAULT_CONFIG = {
    "hotkey": ["ctrl_l", "shift_l", "space"],
    "activation_mode": MODE_PUSH_TO_TALK,  # "push_to_talk" or "toggle"
    "window_position": [100, 100],
    "audio_device": None,
    "show_timestamps": False,
}


def get_config_dir() -> Path:
    """Get the configuration directory path."""
    if os.name == "nt":
        base = Path(os.environ.get("APPDATA", Path.home()))
    else:
        base = Path.home() / ".config"

    config_dir = base / "freeflow"
    config_dir.mkdir(parents=True, exist_ok=True)
    return config_dir


def get_config_path() -> Path:
    """Get the configuration file path."""
    return get_config_dir() / "config.json"


def load_config() -> Dict[str, Any]:
    """Load configuration from file, creating default if it doesn't exist."""
    config_path = get_config_path()

    if config_path.exists():
        try:
            with open(config_path, "r", encoding="utf-8") as f:
                config = json.load(f)
            # Merge with defaults to handle missing keys
            merged = DEFAULT_CONFIG.copy()
            merged.update(config)
            return merged
        except (json.JSONDecodeError, IOError) as e:
            print(f"Error loading config: {e}. Using defaults.")
            return DEFAULT_CONFIG.copy()
    else:
        save_config(DEFAULT_CONFIG)
        return DEFAULT_CONFIG.copy()


def save_config(config: Dict[str, Any]) -> bool:
    """Save configuration to file."""
    config_path = get_config_path()

    try:
        with open(config_path, "w", encoding="utf-8") as f:
            json.dump(config, f, indent=2)
        return True
    except IOError as e:
        print(f"Error saving config: {e}")
        return False


def get_hotkey() -> List[str]:
    """Get the configured hotkey combination."""
    config = load_config()
    return config.get("hotkey", DEFAULT_CONFIG["hotkey"])


def set_hotkey(hotkey: List[str]) -> bool:
    """Set and save the hotkey combination."""
    config = load_config()
    config["hotkey"] = hotkey
    return save_config(config)


def get_window_position() -> List[int]:
    """Get the window position."""
    config = load_config()
    return config.get("window_position", DEFAULT_CONFIG["window_position"])


def set_window_position(x: int, y: int) -> bool:
    """Set and save the window position."""
    config = load_config()
    config["window_position"] = [x, y]
    return save_config(config)


def get_audio_device() -> Optional[int]:
    """Get the configured audio device index."""
    config = load_config()
    return config.get("audio_device", DEFAULT_CONFIG["audio_device"])


def set_audio_device(device_index: Optional[int]) -> bool:
    """Set and save the audio device index."""
    config = load_config()
    config["audio_device"] = device_index
    return save_config(config)


def get_activation_mode() -> str:
    """Get the activation mode (push_to_talk or toggle)."""
    config = load_config()
    mode = config.get("activation_mode", DEFAULT_CONFIG["activation_mode"])
    # Validate mode
    if mode not in (MODE_PUSH_TO_TALK, MODE_TOGGLE):
        return MODE_PUSH_TO_TALK
    return mode


def set_activation_mode(mode: str) -> bool:
    """Set and save the activation mode.

    Args:
        mode: Either MODE_PUSH_TO_TALK or MODE_TOGGLE.
    """
    if mode not in (MODE_PUSH_TO_TALK, MODE_TOGGLE):
        print(f"Invalid activation mode: {mode}")
        return False
    config = load_config()
    config["activation_mode"] = mode
    return save_config(config)
