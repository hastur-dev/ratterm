#!/usr/bin/env python3
"""
Bright Blue Theme Extension for Ratterm.

This extension creates and applies a bright blue theme that makes
everything in the terminal bright blue including all text.

Requirements:
    pip install requests
"""

import os
import sys

import requests


# Get API URL and token from environment variables
API_URL = os.environ.get("RATTERM_API_URL", "http://127.0.0.1:7878")
API_TOKEN = os.environ.get("RATTERM_API_TOKEN", "")


# The bright blue theme TOML content
BRIGHT_BLUE_THEME = '''[theme]
name = "bright-blue"
description = "Everything is bright blue"
base = "dark"

[palette]
bright_blue = "#00BFFF"
deep_blue = "#0066FF"
light_blue = "#87CEEB"
dark_blue = "#000033"

[colors]
# Terminal
"terminal.foreground" = "$bright_blue"
"terminal.background" = "$dark_blue"
"terminal.cursor" = "$light_blue"
"terminal.selection" = "$deep_blue"
"terminal.border" = "$bright_blue"
"terminal.border_focused" = "$light_blue"

# Editor
"editor.foreground" = "$bright_blue"
"editor.background" = "$dark_blue"
"editor.cursor" = "$light_blue"
"editor.selection" = "$deep_blue"
"editor.line_numbers_fg" = "$bright_blue"
"editor.line_numbers_bg" = "$dark_blue"
"editor.current_line" = "#001a4d"
"editor.border" = "$bright_blue"
"editor.border_focused" = "$light_blue"

# Status bar
"statusbar.foreground" = "$bright_blue"
"statusbar.background" = "#001133"
"statusbar.mode_normal" = "$bright_blue"
"statusbar.mode_insert" = "$light_blue"
"statusbar.mode_visual" = "$deep_blue"
"statusbar.mode_command" = "#00FFFF"

# Tabs
"tabs.active_bg" = "$deep_blue"
"tabs.active_fg" = "$light_blue"
"tabs.inactive_bg" = "$dark_blue"
"tabs.inactive_fg" = "$bright_blue"

# Popup
"popup.foreground" = "$bright_blue"
"popup.background" = "$dark_blue"
"popup.border" = "$bright_blue"
"popup.selected_bg" = "$deep_blue"
"popup.selected_fg" = "$light_blue"
"popup.input_bg" = "#001a4d"

# File browser
"filebrowser.foreground" = "$bright_blue"
"filebrowser.background" = "$dark_blue"
"filebrowser.directory" = "$light_blue"
"filebrowser.file" = "$bright_blue"
"filebrowser.selected_bg" = "$deep_blue"
"filebrowser.selected_fg" = "$light_blue"
"filebrowser.border" = "$bright_blue"
'''


def get_themes_dir() -> str:
    """Get the themes directory path."""
    if os.name == 'nt':
        home = os.environ.get('USERPROFILE', os.path.expanduser('~'))
    else:
        home = os.path.expanduser('~')
    result = os.path.join(home, '.ratterm', 'themes')
    assert result, "Failed to determine themes directory"
    return result


def ensure_theme_file() -> str:
    """Create the bright-blue theme file if it doesn't exist."""
    themes_dir = get_themes_dir()
    os.makedirs(themes_dir, exist_ok=True)
    assert os.path.isdir(themes_dir), f"Failed to create themes directory: {themes_dir}"

    theme_path = os.path.join(themes_dir, 'bright-blue.toml')

    # Always write the theme file to ensure it's up to date
    with open(theme_path, 'w', encoding='utf-8') as f:
        f.write(BRIGHT_BLUE_THEME)

    assert os.path.isfile(theme_path), f"Failed to create theme file: {theme_path}"
    print(f"Created theme file: {theme_path}")
    return theme_path


def get_headers() -> dict[str, str]:
    """Get request headers with authentication."""
    assert API_TOKEN, "RATTERM_API_TOKEN environment variable must be set"
    return {
        "Authorization": f"Bearer {API_TOKEN}",
        "Content-Type": "application/json"
    }


def get_theme() -> dict:
    """Get the current theme."""
    resp = requests.get(
        f"{API_URL}/api/v1/system/theme",
        headers=get_headers(),
        timeout=10
    )
    resp.raise_for_status()
    result = resp.json()
    assert isinstance(result, dict), "Expected dict response"
    return result


def list_themes() -> dict:
    """List available themes."""
    resp = requests.get(
        f"{API_URL}/api/v1/system/themes",
        headers=get_headers(),
        timeout=10
    )
    resp.raise_for_status()
    result = resp.json()
    assert isinstance(result, dict), "Expected dict response"
    return result


def set_theme(name: str) -> dict:
    """Set the theme."""
    assert name, "Theme name must not be empty"
    resp = requests.put(
        f"{API_URL}/api/v1/system/theme",
        headers=get_headers(),
        json={"name": name},
        timeout=10
    )
    resp.raise_for_status()
    result = resp.json()
    assert isinstance(result, dict), "Expected dict response"
    return result


def set_status(message: str) -> dict:
    """Set the status bar message."""
    assert message, "Message must not be empty"
    resp = requests.put(
        f"{API_URL}/api/v1/system/status",
        headers=get_headers(),
        json={"message": message},
        timeout=10
    )
    resp.raise_for_status()
    result = resp.json()
    assert isinstance(result, dict), "Expected dict response"
    return result


def main() -> None:
    """Main entry point."""
    # Validate environment
    if not API_TOKEN:
        print("Error: RATTERM_API_TOKEN environment variable not set")
        print("Make sure Ratterm is running and has started this extension")
        sys.exit(1)

    # First, ensure the theme file exists
    ensure_theme_file()

    # Connect to API
    try:
        current_data = get_theme()
    except requests.RequestException as e:
        print(f"Failed to connect to Ratterm API: {e}")
        print("Make sure Ratterm is running.")
        sys.exit(1)

    current_theme = current_data.get("name", "unknown")
    print(f"Current theme: {current_theme}")

    # List available themes to verify our theme is there
    themes_data = list_themes()
    themes = themes_data.get("themes", [])
    assert isinstance(themes, list), "Expected themes to be a list"
    print(f"Available themes: {', '.join(themes)}")

    # Check if bright-blue is in the list
    if 'bright-blue' not in themes:
        print("Warning: bright-blue theme not found in list.")
        print("The theme file was created, but you may need to restart Ratterm.")
        print("Attempting to set it anyway...")

    # Set the bright blue theme
    print("Setting theme to: bright-blue")
    try:
        result = set_theme("bright-blue")
        if result.get("success"):
            print("Theme changed to bright-blue!")
            print("Everything should now be bright blue!")
            # Update status bar
            set_status("Bright blue theme activated!")
        else:
            print("Theme change may have failed.")
            print(f"Response: {result}")
            print("Try restarting Ratterm to load the new theme file.")
    except requests.RequestException as e:
        print(f"Error setting theme: {e}")
        print("The theme file was created. Try restarting Ratterm and running again.")


if __name__ == "__main__":
    main()
