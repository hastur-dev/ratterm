#!/usr/bin/env python3
"""Test client for Ratterm API.

This script connects to the Ratterm API via named pipe and tests
terminal manipulation commands.

Usage:
    1. Start ratterm: cargo run --release
    2. In another terminal: python test_api.py
"""

import json
import time
import sys

# Windows named pipe support
if sys.platform == 'win32':
    import win32file
    import win32pipe

PIPE_NAME = r'\\.\pipe\ratterm-api'

def connect_pipe():
    """Connect to the named pipe."""
    try:
        handle = win32file.CreateFile(
            PIPE_NAME,
            win32file.GENERIC_READ | win32file.GENERIC_WRITE,
            0,
            None,
            win32file.OPEN_EXISTING,
            0,
            None
        )
        return handle
    except Exception as e:
        print(f"Failed to connect to pipe: {e}")
        print("Make sure ratterm is running.")
        return None

def send_request(handle, method, params=None):
    """Send a request and get response."""
    if params is None:
        params = {}

    request = {
        "id": str(int(time.time() * 1000)),
        "method": method,
        "params": params
    }

    # Send request with newline delimiter
    message = json.dumps(request) + "\n"
    win32file.WriteFile(handle, message.encode('utf-8'))

    # Read response (read until newline)
    response_data = b""
    while True:
        _, data = win32file.ReadFile(handle, 4096)
        response_data += data
        if b'\n' in response_data:
            break

    response_str = response_data.decode('utf-8').strip()
    return json.loads(response_str)

def main():
    print("Ratterm API Test Client")
    print("=" * 40)

    # Connect to pipe
    handle = connect_pipe()
    if not handle:
        return 1

    print("Connected to ratterm API!")
    print()

    # Test 1: Get system version
    print("Test 1: Getting system version...")
    response = send_request(handle, "system.get_version")
    print(f"  Response: {response}")
    print()

    # Test 2: Get terminal size
    print("Test 2: Getting terminal size...")
    response = send_request(handle, "terminal.get_size")
    print(f"  Response: {response}")
    print()

    # Test 3: Read terminal buffer
    print("Test 3: Reading terminal buffer (first 5 lines)...")
    response = send_request(handle, "terminal.read_buffer", {"lines": 5})
    if "result" in response and "lines" in response["result"]:
        for i, line in enumerate(response["result"]["lines"]):
            print(f"  Line {i}: {repr(line)}")
    else:
        print(f"  Response: {response}")
    print()

    # Test 4: Send keystrokes
    print("Test 4: Sending 'echo Hello from API' to terminal...")
    response = send_request(handle, "terminal.send_keys", {"keys": "echo Hello from API\n"})
    print(f"  Response: {response}")
    print()

    # Wait for command to execute
    time.sleep(0.5)

    # Test 5: Read buffer again to see output
    print("Test 5: Reading terminal buffer after command...")
    response = send_request(handle, "terminal.read_buffer", {"lines": 10})
    if "result" in response and "lines" in response["result"]:
        for i, line in enumerate(response["result"]["lines"]):
            if line.strip():
                print(f"  Line {i}: {repr(line)}")
    else:
        print(f"  Response: {response}")
    print()

    # Test 6: Get layout state
    print("Test 6: Getting layout state...")
    response = send_request(handle, "layout.get_state")
    print(f"  Response: {response}")
    print()

    # Test 7: List terminal tabs
    print("Test 7: Listing terminal tabs...")
    response = send_request(handle, "tabs.list_terminal")
    print(f"  Response: {response}")
    print()

    # Test 8: Set status message
    print("Test 8: Setting status message...")
    response = send_request(handle, "system.set_status", {"message": "API test complete!"})
    print(f"  Response: {response}")
    print()

    print("=" * 40)
    print("All tests complete!")

    # Close handle
    win32file.CloseHandle(handle)
    return 0

if __name__ == "__main__":
    sys.exit(main())
