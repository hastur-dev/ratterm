#!/usr/bin/env python3
"""Test client for Ratterm Background Process API.

This script connects to the Ratterm API via named pipe and tests
background process management commands.

Usage:
    1. Start ratterm: cargo run --release
    2. In another terminal: python test_background_api.py
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
    print("Ratterm Background Process API Test Client")
    print("=" * 50)

    # Connect to pipe
    handle = connect_pipe()
    if not handle:
        return 1

    print("Connected to ratterm API!")
    print()

    # Test 1: List background processes (should be empty)
    print("Test 1: Listing background processes (should be empty)...")
    response = send_request(handle, "background.list")
    print(f"  Response: {json.dumps(response, indent=2)}")
    print()

    # Test 2: Start a short-running background process
    print("Test 2: Starting a short background process (echo test)...")
    response = send_request(handle, "background.start", {"command": "echo Hello from background process"})
    print(f"  Response: {json.dumps(response, indent=2)}")

    if "result" in response and "id" in response["result"]:
        process_id = response["result"]["id"]
        print(f"  Started process ID: {process_id}")
    else:
        print("  Failed to start process!")
        process_id = None
    print()

    # Wait a bit for the process to complete
    time.sleep(1)

    # Test 3: Check process status
    if process_id:
        print(f"Test 3: Getting status of process {process_id}...")
        response = send_request(handle, "background.status", {"id": process_id})
        print(f"  Response: {json.dumps(response, indent=2)}")
        print()

    # Test 4: Get process output
    if process_id:
        print(f"Test 4: Getting output of process {process_id}...")
        response = send_request(handle, "background.output", {"id": process_id})
        print(f"  Response: {json.dumps(response, indent=2)}")
        print()

    # Test 5: Start a longer-running process
    print("Test 5: Starting a longer background process (ping)...")
    if sys.platform == 'win32':
        command = "ping -n 3 127.0.0.1"
    else:
        command = "ping -c 3 127.0.0.1"

    response = send_request(handle, "background.start", {"command": command})
    print(f"  Response: {json.dumps(response, indent=2)}")

    if "result" in response and "id" in response["result"]:
        long_process_id = response["result"]["id"]
        print(f"  Started process ID: {long_process_id}")
    else:
        long_process_id = None
    print()

    # Test 6: List all processes
    print("Test 6: Listing all background processes...")
    response = send_request(handle, "background.list")
    print(f"  Response: {json.dumps(response, indent=2)}")
    print()

    # Test 7: Check status while running
    if long_process_id:
        print(f"Test 7: Checking status of running process {long_process_id}...")
        response = send_request(handle, "background.status", {"id": long_process_id})
        print(f"  Response: {json.dumps(response, indent=2)}")
        print()

    # Wait for the ping to complete
    print("Waiting for ping to complete...")
    time.sleep(5)

    # Test 8: Get final status and output
    if long_process_id:
        print(f"Test 8: Getting final status and output of process {long_process_id}...")
        response = send_request(handle, "background.status", {"id": long_process_id})
        print(f"  Status: {json.dumps(response, indent=2)}")

        response = send_request(handle, "background.output", {"id": long_process_id})
        print(f"  Output: {json.dumps(response, indent=2)}")
        print()

    # Test 9: Start a process and kill it
    print("Test 9: Starting a process to kill...")
    if sys.platform == 'win32':
        command = "ping -n 10 127.0.0.1"
    else:
        command = "sleep 10"

    response = send_request(handle, "background.start", {"command": command})
    if "result" in response and "id" in response["result"]:
        kill_process_id = response["result"]["id"]
        print(f"  Started process ID: {kill_process_id}")

        time.sleep(1)

        print(f"  Killing process {kill_process_id}...")
        response = send_request(handle, "background.kill", {"id": kill_process_id})
        print(f"  Kill response: {json.dumps(response, indent=2)}")

        response = send_request(handle, "background.status", {"id": kill_process_id})
        print(f"  Status after kill: {json.dumps(response, indent=2)}")
    print()

    # Test 10: Clear finished processes
    print("Test 10: Clearing finished processes...")
    response = send_request(handle, "background.clear")
    print(f"  Response: {json.dumps(response, indent=2)}")

    response = send_request(handle, "background.list")
    print(f"  List after clear: {json.dumps(response, indent=2)}")
    print()

    print("=" * 50)
    print("All tests complete!")

    # Close handle
    win32file.CloseHandle(handle)
    return 0

if __name__ == "__main__":
    sys.exit(main())
