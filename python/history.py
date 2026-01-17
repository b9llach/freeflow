"""Transcription history management for FreeFlow."""

import json
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional
from config import get_config_dir


def get_history_path() -> Path:
    """Get the history file path."""
    return get_config_dir() / "history.json"


def load_history() -> List[Dict[str, Any]]:
    """Load transcription history from file."""
    history_path = get_history_path()

    if history_path.exists():
        try:
            with open(history_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError) as e:
            print(f"Error loading history: {e}")
            return []
    return []


def save_history(history: List[Dict[str, Any]]) -> bool:
    """Save transcription history to file."""
    history_path = get_history_path()

    try:
        with open(history_path, "w", encoding="utf-8") as f:
            json.dump(history, f, indent=2, ensure_ascii=False)
        return True
    except IOError as e:
        print(f"Error saving history: {e}")
        return False


def add_to_history(
    original_text: str,
    final_text: str,
    duration_seconds: Optional[float] = None
) -> Dict[str, Any]:
    """Add a new transcription to history.

    Args:
        original_text: The raw transcribed text before replacements
        final_text: The text after applying replacements
        duration_seconds: Optional recording duration

    Returns:
        The created history entry
    """
    history = load_history()

    entry = {
        "id": len(history) + 1,
        "timestamp": datetime.now().isoformat(),
        "original_text": original_text,
        "final_text": final_text,
        "duration_seconds": duration_seconds
    }

    history.append(entry)
    save_history(history)

    return entry


def get_history(limit: Optional[int] = None, offset: int = 0) -> List[Dict[str, Any]]:
    """Get transcription history with optional pagination.

    Args:
        limit: Maximum number of entries to return (None for all)
        offset: Number of entries to skip

    Returns:
        List of history entries, newest first
    """
    history = load_history()

    # Sort by timestamp, newest first
    history.sort(key=lambda x: x.get("timestamp", ""), reverse=True)

    # Apply pagination
    if offset:
        history = history[offset:]
    if limit:
        history = history[:limit]

    return history


def clear_history() -> bool:
    """Clear all transcription history."""
    return save_history([])


def delete_history_entry(entry_id: int) -> bool:
    """Delete a specific history entry by ID."""
    history = load_history()

    original_len = len(history)
    history = [entry for entry in history if entry.get("id") != entry_id]

    if len(history) < original_len:
        return save_history(history)
    return False


def get_history_stats() -> Dict[str, Any]:
    """Get statistics about transcription history."""
    history = load_history()

    total_entries = len(history)
    total_duration = sum(
        entry.get("duration_seconds", 0) or 0
        for entry in history
    )
    total_characters = sum(
        len(entry.get("final_text", ""))
        for entry in history
    )
    total_words = sum(
        len(entry.get("final_text", "").split())
        for entry in history
    )

    return {
        "total_entries": total_entries,
        "total_duration_seconds": total_duration,
        "total_characters": total_characters,
        "total_words": total_words
    }
