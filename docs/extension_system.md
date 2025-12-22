# Ratterm Extension System

Ratterm supports extensions via a REST API that allows external processes to control and modify the application. This document explains how the extension system works and how to create extensions.

## Overview

The extension system uses a simple architecture:
1. Extensions are external processes that run alongside Ratterm
2. Extensions communicate with Ratterm via a REST API on `http://127.0.0.1:7878`
3. Extensions must be approved by the user before they can run
4. Approvals are stored persistently and are version-specific

## Extension Manifest

Every extension needs a `ratterm.toml` manifest file:

```toml
[extension]
name = "my-extension"
version = "1.0.0"
description = "My awesome extension"
author = "Your Name"

[process]
command = "python"
args = ["{ext_dir}/main.py"]
cwd = "{ext_dir}"
restart_on_crash = true
max_restarts = 3
restart_delay_ms = 1000

[process.env]
MY_VAR = "value"
```

### Manifest Fields

| Field | Required | Description |
|-------|----------|-------------|
| `extension.name` | Yes | Unique extension identifier |
| `extension.version` | Yes | Semantic version (e.g., "1.0.0") |
| `extension.description` | No | Brief description |
| `extension.author` | No | Author name |
| `process.command` | Yes | Command to run (e.g., "python", "node") |
| `process.args` | No | Command arguments |
| `process.cwd` | No | Working directory |
| `process.env` | No | Environment variables |
| `process.restart_on_crash` | No | Auto-restart on crash (default: true) |
| `process.max_restarts` | No | Maximum restart attempts (default: 3) |
| `process.restart_delay_ms` | No | Delay between restarts (default: 1000) |

The placeholder `{ext_dir}` is replaced with the extension's installation directory.

## Installation

Extensions are installed from GitHub:

```bash
# Install from GitHub
rat ext install user/repo

# Install specific version
rat ext install user/repo@v1.0.0

# List installed extensions
rat ext list

# Update extensions
rat ext update [name]

# Remove extension
rat ext remove name
```

## User Approval

For security, extensions must be approved by the user:
- First-time installations show an approval popup
- Version updates require re-approval
- Approvals are stored in `~/.ratterm/approved_extensions.toml`

## API Reference

Extensions communicate via HTTP REST API at `http://127.0.0.1:7878`. All endpoints require authentication via Bearer token.

### Authentication

All requests must include the API token in the Authorization header:

```
Authorization: Bearer <token>
```

The token is provided to extensions via the `RATTERM_API_TOKEN` environment variable.

### Base URL

```
http://127.0.0.1:7878/api/v1
```

---

## Available Endpoints

### Terminal Operations

#### `GET /api/v1/terminal/buffer`
Reads the terminal buffer content.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `tab_index` | number | Optional tab index (default: active tab) |
| `lines` | number | Number of lines to read |
| `offset` | number | Line offset from top |

**Response:**
```json
{
  "lines": ["line1", "line2"],
  "cursor": {"col": 5, "row": 10},
  "size": {"cols": 80, "rows": 24}
}
```

#### `POST /api/v1/terminal/send_keys`
Sends keystrokes to the terminal.

**Request Body:**
```json
{
  "keys": "ls -la\n",
  "tab_index": 0
}
```

#### `GET /api/v1/terminal/size`
Gets terminal dimensions.

**Response:**
```json
{
  "cols": 80,
  "rows": 24
}
```

#### `GET /api/v1/terminal/cursor`
Gets terminal cursor position.

**Response:**
```json
{
  "col": 5,
  "row": 10
}
```

#### `GET /api/v1/terminal/title`
Gets terminal title.

#### `POST /api/v1/terminal/clear`
Clears the terminal screen.

#### `GET /api/v1/terminal/scrollback`
Gets terminal scrollback buffer.

#### `GET /api/v1/terminal/selection`
Gets current terminal selection.

#### `POST /api/v1/terminal/scroll`
Scrolls the terminal.

**Request Body:**
```json
{
  "lines": 10,
  "direction": "up"
}
```

---

### Editor Operations

#### `POST /api/v1/editor/open`
Opens a file in the editor.

**Request Body:**
```json
{
  "path": "/path/to/file.txt"
}
```

#### `GET /api/v1/editor/content`
Reads the current editor content.

**Response:**
```json
{
  "content": "file contents...",
  "path": "/path/to/file.txt",
  "modified": false,
  "cursor": {"col": 0, "row": 0}
}
```

#### `PUT /api/v1/editor/content`
Replaces the editor content.

**Request Body:**
```json
{
  "content": "new content..."
}
```

#### `POST /api/v1/editor/save`
Saves the current file.

**Request Body:**
```json
{
  "path": "/optional/new/path.txt"
}
```

#### `GET /api/v1/editor/file`
Gets current file info.

#### `GET /api/v1/editor/cursor`
Gets cursor position.

**Response:**
```json
{
  "line": 10,
  "col": 5
}
```

#### `PUT /api/v1/editor/cursor`
Sets cursor position.

**Request Body:**
```json
{
  "line": 10,
  "col": 5
}
```

#### `POST /api/v1/editor/insert`
Inserts text at position.

**Request Body:**
```json
{
  "text": "inserted text",
  "position": {"line": 5, "col": 0}
}
```

---

### Filesystem Operations

#### `GET /api/v1/fs/read`
Reads a file from disk.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | string | File path to read |

#### `POST /api/v1/fs/write`
Writes content to a file.

**Request Body:**
```json
{
  "path": "/path/to/file.txt",
  "content": "file content"
}
```

#### `GET /api/v1/fs/exists`
Checks if a path exists.

#### `GET /api/v1/fs/is_dir`
Checks if path is a directory.

#### `GET /api/v1/fs/is_file`
Checks if path is a file.

#### `GET /api/v1/fs/list_dir`
Lists directory contents.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | string | Directory path |

#### `POST /api/v1/fs/mkdir`
Creates a directory.

**Request Body:**
```json
{
  "path": "/path/to/new/dir"
}
```

#### `DELETE /api/v1/fs/remove`
Removes a file or directory.

#### `POST /api/v1/fs/rename`
Renames a file or directory.

**Request Body:**
```json
{
  "from": "/old/path",
  "to": "/new/path"
}
```

#### `POST /api/v1/fs/copy`
Copies a file or directory.

**Request Body:**
```json
{
  "from": "/source/path",
  "to": "/dest/path"
}
```

---

### Layout Operations

#### `GET /api/v1/layout/state`
Gets layout state.

**Response:**
```json
{
  "focused": "terminal",
  "ide_visible": true,
  "split_ratio": 0.5
}
```

#### `POST /api/v1/layout/focus`
Focuses a pane.

**Request Body:**
```json
{
  "pane": "terminal"
}
```

Pane values: `"terminal"` or `"editor"`

#### `POST /api/v1/layout/toggle_ide`
Toggles IDE visibility.

**Response:**
```json
{
  "visible": true
}
```

#### `PUT /api/v1/layout/split`
Resizes the split.

**Request Body:**
```json
{
  "ratio": 0.6
}
```

---

### Tab Operations

#### `GET /api/v1/tabs/terminal`
Lists terminal tabs.

**Response:**
```json
{
  "tabs": [
    {"index": 0, "name": "bash", "active": true},
    {"index": 1, "name": "zsh", "active": false}
  ]
}
```

#### `GET /api/v1/tabs/editor`
Lists editor tabs.

**Response:**
```json
{
  "tabs": [
    {"index": 0, "name": "main.py", "path": "/path/main.py", "modified": false, "active": true}
  ]
}
```

#### `POST /api/v1/tabs/terminal/new`
Creates a new terminal tab.

**Request Body:**
```json
{
  "shell": "bash"
}
```

**Response:**
```json
{
  "index": 1
}
```

#### `POST /api/v1/tabs/terminal/switch`
Switches to a terminal tab.

**Request Body:**
```json
{
  "index": 1
}
```

#### `DELETE /api/v1/tabs/terminal/close`
Closes a terminal tab.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `index` | number | Tab index (default: active) |

---

### System Operations

#### `GET /api/v1/system/cwd`
Gets current working directory.

**Response:**
```json
{
  "path": "/home/user/project"
}
```

#### `GET /api/v1/system/status`
Gets status bar message.

**Response:**
```json
{
  "message": "Ready"
}
```

#### `PUT /api/v1/system/status`
Sets status bar message.

**Request Body:**
```json
{
  "message": "Extension loaded!"
}
```

#### `GET /api/v1/system/version`
Gets Ratterm version.

**Response:**
```json
{
  "version": "0.1.3"
}
```

#### `GET /api/v1/system/theme`
Gets the current theme.

**Response:**
```json
{
  "name": "dracula"
}
```

#### `PUT /api/v1/system/theme`
Sets the theme.

**Request Body:**
```json
{
  "name": "matrix"
}
```

#### `GET /api/v1/system/themes`
Lists available themes.

**Response:**
```json
{
  "themes": ["dark", "light", "dracula", "gruvbox", "nord", "matrix"],
  "current": "dark"
}
```

#### `GET /api/v1/system/config`
Gets application configuration.

#### `POST /api/v1/system/notify`
Shows a notification.

**Request Body:**
```json
{
  "message": "Hello from extension!",
  "level": "info"
}
```

Level values: `"info"`, `"warning"`, `"error"`

---

### Command Operations

#### `POST /api/v1/commands/register`
Registers a custom command.

**Request Body:**
```json
{
  "name": "my-command",
  "description": "Does something cool"
}
```

#### `DELETE /api/v1/commands/unregister`
Unregisters a custom command.

#### `GET /api/v1/commands/list`
Lists all registered commands.

#### `POST /api/v1/commands/execute`
Executes a command.

**Request Body:**
```json
{
  "name": "my-command",
  "args": []
}
```

---

### Event Operations

#### `GET /api/v1/events/stream`
Server-Sent Events (SSE) stream for real-time events.

**Example Events:**
```
event: file_opened
data: {"path": "/path/to/file.txt"}

event: file_saved
data: {"path": "/path/to/file.txt"}

event: theme_changed
data: {"name": "dracula"}
```

---

### Extension Operations

#### `GET /api/v1/extensions/list`
Lists installed extensions.

#### `GET /api/v1/extensions/health`
Health check endpoint.

**Response:**
```json
{
  "status": "ok"
}
```

#### `POST /api/v1/extensions/reload`
Reloads an extension.

---

## Example Extension (Python)

```python
#!/usr/bin/env python3
"""Example Ratterm extension that changes the theme."""

import os
import requests

# Get API URL and token from environment
API_URL = os.environ.get("RATTERM_API_URL", "http://127.0.0.1:7878")
API_TOKEN = os.environ.get("RATTERM_API_TOKEN", "")

def get_headers():
    """Get request headers with authentication."""
    return {
        "Authorization": f"Bearer {API_TOKEN}",
        "Content-Type": "application/json"
    }

def get_theme():
    """Get the current theme."""
    resp = requests.get(
        f"{API_URL}/api/v1/system/theme",
        headers=get_headers()
    )
    resp.raise_for_status()
    return resp.json()

def list_themes():
    """List available themes."""
    resp = requests.get(
        f"{API_URL}/api/v1/system/themes",
        headers=get_headers()
    )
    resp.raise_for_status()
    return resp.json()

def set_theme(name: str):
    """Set the theme."""
    resp = requests.put(
        f"{API_URL}/api/v1/system/theme",
        headers=get_headers(),
        json={"name": name}
    )
    resp.raise_for_status()
    return resp.json()

def set_status(message: str):
    """Set the status bar message."""
    resp = requests.put(
        f"{API_URL}/api/v1/system/status",
        headers=get_headers(),
        json={"message": message}
    )
    resp.raise_for_status()
    return resp.json()

def main():
    # Get current theme
    current = get_theme()
    print(f"Current theme: {current.get('name')}")

    # List available themes
    themes_data = list_themes()
    themes = themes_data.get("themes", [])
    print(f"Available themes: {', '.join(themes)}")

    # Change theme to matrix
    result = set_theme("matrix")
    if result.get("success"):
        print("Theme changed to matrix!")

    # Set status message
    set_status("Theme changed by extension!")

if __name__ == "__main__":
    main()
```

---

## Example: Listening to Events (SSE)

```python
#!/usr/bin/env python3
"""Example: Listen to Ratterm events via SSE."""

import os
import requests

API_URL = os.environ.get("RATTERM_API_URL", "http://127.0.0.1:7878")
API_TOKEN = os.environ.get("RATTERM_API_TOKEN", "")

def listen_events():
    """Listen to the event stream."""
    headers = {
        "Authorization": f"Bearer {API_TOKEN}",
        "Accept": "text/event-stream"
    }

    with requests.get(
        f"{API_URL}/api/v1/events/stream",
        headers=headers,
        stream=True
    ) as resp:
        resp.raise_for_status()
        for line in resp.iter_lines():
            if line:
                print(line.decode())

if __name__ == "__main__":
    listen_events()
```

---

## Environment Variables

When Ratterm starts an extension process, these environment variables are set:

| Variable | Description |
|----------|-------------|
| `RATTERM_API_URL` | REST API URL (e.g., `http://127.0.0.1:7878`) |
| `RATTERM_API_TOKEN` | Authentication token for REST API |
| `RATTERM_EXTENSION_NAME` | Extension name from manifest |
| `RATTERM_EXTENSION_DIR` | Extension installation directory |

## Security

- Extensions run as separate processes with their own permissions
- User must approve each extension before it can run
- Version changes require re-approval
- All API requests require authentication via Bearer token
- The API only listens on localhost (127.0.0.1)
- Never install extensions from untrusted sources

## Troubleshooting

### Extension not starting
1. Check the manifest file format
2. Verify the command exists and is executable
3. Check logs for error messages

### API connection failed
1. Ensure Ratterm is running
2. Check if the API is enabled in settings
3. Verify the API token is correct

### Authentication errors
1. Ensure `RATTERM_API_TOKEN` environment variable is set
2. Include the token in the Authorization header
3. Use `Bearer <token>` format

### Permission denied
1. Ensure the extension has been approved
2. Check file permissions on the extension directory
