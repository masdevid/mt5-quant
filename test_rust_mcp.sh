#!/bin/bash

echo "Testing Rust MCP Server..."

# Start the server in background
./target/debug/mt5-quant &
SERVER_PID=$!

# Give it time to start
sleep 1

# Test initialization
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | ./target/debug/mt5-quant

# Wait a bit
sleep 1

# Test tools list
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | ./target/debug/mt5-quant

# Wait a bit
sleep 1

# Test tool call
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"verify_setup","arguments":{}}}' | ./target/debug/mt5-quant

# Clean up
kill $SERVER_PID 2>/dev/null
