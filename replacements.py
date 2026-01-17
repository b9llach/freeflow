"""Word and phrase replacement management for FreeFlow."""

import json
import re
import uuid
from pathlib import Path
from typing import Any, Dict, List, Optional
from config import get_config_dir


def get_replacements_path() -> Path:
    """Get the replacements file path."""
    return get_config_dir() / "replacements.json"


def load_replacements() -> List[Dict[str, Any]]:
    """Load replacement rules from file."""
    replacements_path = get_replacements_path()

    if replacements_path.exists():
        try:
            with open(replacements_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError) as e:
            print(f"Error loading replacements: {e}")
            return []
    return []


def save_replacements(replacements: List[Dict[str, Any]]) -> bool:
    """Save replacement rules to file."""
    replacements_path = get_replacements_path()

    try:
        with open(replacements_path, "w", encoding="utf-8") as f:
            json.dump(replacements, f, indent=2, ensure_ascii=False)
        return True
    except IOError as e:
        print(f"Error saving replacements: {e}")
        return False


def add_replacement(
    find: str,
    replace: str,
    case_sensitive: bool = False,
    whole_word: bool = True,
    enabled: bool = True
) -> Dict[str, Any]:
    """Add a new replacement rule.

    Args:
        find: The text to find
        replace: The text to replace with
        case_sensitive: Whether matching should be case-sensitive
        whole_word: Whether to match whole words only
        enabled: Whether this replacement is active

    Returns:
        The created replacement rule
    """
    replacements = load_replacements()

    rule = {
        "id": str(uuid.uuid4()),
        "find": find,
        "replace": replace,
        "case_sensitive": case_sensitive,
        "whole_word": whole_word,
        "enabled": enabled
    }

    replacements.append(rule)
    save_replacements(replacements)

    return rule


def update_replacement(rule_id: str, updates: Dict[str, Any]) -> Optional[Dict[str, Any]]:
    """Update an existing replacement rule.

    Args:
        rule_id: The ID of the rule to update
        updates: Dictionary of fields to update

    Returns:
        The updated rule, or None if not found
    """
    replacements = load_replacements()

    for rule in replacements:
        if rule.get("id") == rule_id:
            # Only allow updating specific fields
            allowed_fields = {"find", "replace", "case_sensitive", "whole_word", "enabled"}
            for key, value in updates.items():
                if key in allowed_fields:
                    rule[key] = value

            save_replacements(replacements)
            return rule

    return None


def delete_replacement(rule_id: str) -> bool:
    """Delete a replacement rule by ID."""
    replacements = load_replacements()

    original_len = len(replacements)
    replacements = [r for r in replacements if r.get("id") != rule_id]

    if len(replacements) < original_len:
        return save_replacements(replacements)
    return False


def get_replacements() -> List[Dict[str, Any]]:
    """Get all replacement rules."""
    return load_replacements()


def apply_replacements(text: str) -> str:
    """Apply all enabled replacement rules to text.

    Args:
        text: The input text to process

    Returns:
        Text with all replacements applied
    """
    replacements = load_replacements()

    for rule in replacements:
        if not rule.get("enabled", True):
            continue

        find = rule.get("find", "")
        replace = rule.get("replace", "")

        if not find:
            continue

        # Build regex pattern
        pattern = re.escape(find)

        if rule.get("whole_word", True):
            # Match whole words only
            pattern = r"\b" + pattern + r"\b"

        flags = 0 if rule.get("case_sensitive", False) else re.IGNORECASE

        try:
            text = re.sub(pattern, replace, text, flags=flags)
        except re.error as e:
            print(f"Regex error applying replacement '{find}': {e}")
            continue

    return text


def import_replacements(rules: List[Dict[str, Any]], merge: bool = True) -> int:
    """Import replacement rules from a list.

    Args:
        rules: List of replacement rules to import
        merge: If True, add to existing rules. If False, replace all.

    Returns:
        Number of rules imported
    """
    if merge:
        existing = load_replacements()
        existing_finds = {r.get("find", "").lower() for r in existing}

        imported = 0
        for rule in rules:
            find = rule.get("find", "")
            if find.lower() not in existing_finds:
                add_replacement(
                    find=find,
                    replace=rule.get("replace", ""),
                    case_sensitive=rule.get("case_sensitive", False),
                    whole_word=rule.get("whole_word", True),
                    enabled=rule.get("enabled", True)
                )
                imported += 1
        return imported
    else:
        save_replacements([])
        for rule in rules:
            add_replacement(
                find=rule.get("find", ""),
                replace=rule.get("replace", ""),
                case_sensitive=rule.get("case_sensitive", False),
                whole_word=rule.get("whole_word", True),
                enabled=rule.get("enabled", True)
            )
        return len(rules)


def export_replacements() -> List[Dict[str, Any]]:
    """Export all replacement rules for backup/sharing."""
    return load_replacements()
