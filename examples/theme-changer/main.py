#!/usr/bin/env python3
"""
Theme Changer Extension for Ratterm.

This extension demonstrates how to connect to Ratterm's REST API
and change the terminal theme.

Usage:
    python main.py [theme_name]

    If no theme name is provided, it cycles through all available themes.

Requirements:
    pip install requests
"""

import os
import sys

import requests


# Get API URL and token from environment variables
API_URL = os.environ.get("RATTERM_API_URL", "http://127.0.0.1:7878")
API_TOKEN = os.environ.get("RATTERM_API_TOKEN", "")


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

    # Get current theme
    try:
        current_data = get_theme()
    except requests.RequestException as e:
        print(f"Failed to connect to Ratterm API: {e}")
        print("Make sure Ratterm is running.")
        sys.exit(1)

    current_theme = current_data.get("name", "unknown")
    print(f"Current theme: {current_theme}")

    # List available themes
    themes_data = list_themes()
    themes = themes_data.get("themes", [])
    assert isinstance(themes, list), "Expected themes to be a list"
    print(f"Available themes: {', '.join(themes)}")

    # Determine which theme to set
    if len(sys.argv) > 1:
        # Use specified theme
        new_theme = sys.argv[1]
        if new_theme not in themes:
            print(f"Unknown theme: {new_theme}")
            print(f"Available: {', '.join(themes)}")
            sys.exit(1)
    else:
        # Cycle to next theme
        if current_theme in themes:
            idx = themes.index(current_theme)
            new_theme = themes[(idx + 1) % len(themes)]
        else:
            new_theme = themes[0] if themes else "dark"

    # Set the new theme
    print(f"Changing theme to: {new_theme}")
    result = set_theme(new_theme)

    if result.get("success"):
        print(f"Theme changed to {new_theme}!")
        # Update status bar to show what we did
        set_status(f"Theme changed to {new_theme} by extension")
    else:
        print("Theme change may have failed")
        print(f"Response: {result}")


if __name__ == "__main__":
    main()
