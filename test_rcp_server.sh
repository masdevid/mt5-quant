#!/bin/bash

echo "Testing Rust MCP Server..."

# Create a temporary file for test
TEMP_FILE=$(mktemp)
cat > "$TEMP_FILE" << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"verify_setup","arguments":{}}}
EOF

# Send all requests in one session
cat "$TEMP_FILE" | /opt/homebrew/bin/mt5-quant

# Clean up
rm "$TEMP_FILE"
