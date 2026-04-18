#!/usr/bin/env python3
"""Test script for MCP server"""
import subprocess
import json
import sys

def test_mcp_server(command):
    """Test if an MCP server responds correctly"""
    print(f"Testing: {' '.join(command)}")
    print("-" * 50)
    
    proc = subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1  # Line buffered
    )
    
    # Send initialize request
    init_request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        }
    }
    
    print(f"Sending initialize request...")
    proc.stdin.write(json.dumps(init_request) + "\n")
    proc.stdin.flush()
    
    # Read response with timeout
    import select
    import time
    
    start = time.time()
    response_lines = []
    
    while time.time() - start < 5:  # 5 second timeout
        ready, _, _ = select.select([proc.stdout], [], [], 0.5)
        if ready:
            line = proc.stdout.readline()
            if line:
                response_lines.append(line.strip())
                print(f"Received: {line.strip()}")
                break
    
    if not response_lines:
        print("ERROR: No response received within 5 seconds")
        proc.terminate()
        proc.wait()
        return False
    
    # Parse response
    try:
        response = json.loads(response_lines[0])
        if response.get("id") == 1 and "result" in response:
            print("SUCCESS: MCP server responded correctly")
            
            # Try to get tools list
            tools_request = {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }
            proc.stdin.write(json.dumps(tools_request) + "\n")
            proc.stdin.flush()
            
            start = time.time()
            while time.time() - start < 5:
                ready, _, _ = select.select([proc.stdout], [], [], 0.5)
                if ready:
                    line = proc.stdout.readline()
                    if line:
                        print(f"Tools response: {line.strip()[:200]}...")
                        break
            
            proc.terminate()
            proc.wait()
            return True
        else:
            print(f"ERROR: Unexpected response: {response}")
            proc.terminate()
            proc.wait()
            return False
    except json.JSONDecodeError as e:
        print(f"ERROR: Invalid JSON response: {e}")
        proc.terminate()
        proc.wait()
        return False

if __name__ == "__main__":
    # Test Python source
    print("\n=== Testing Python Source ===")
    python_ok = test_mcp_server([sys.executable, "server/main.py"])
    
    # Test executable
    print("\n=== Testing PyInstaller Executable ===")
    exe_ok = test_mcp_server(["./dist/mt5-quant"])
    
    print("\n" + "=" * 50)
    print(f"Python source: {'OK' if python_ok else 'FAILED'}")
    print(f"PyInstaller exe: {'OK' if exe_ok else 'FAILED'}")
