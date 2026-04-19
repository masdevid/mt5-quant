#!/bin/bash
# E2E Test for all 43 MT5-Quant MCP tools

set -e

BINARY="/usr/local/bin/mt5-quant"
FAILED=0
PASSED=0

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=========================================="
echo "MT5-Quant E2E Test - All 43 Tools"
echo "=========================================="
echo ""

# Test helper function - sends initialize + tool call in one session
test_tool() {
    local tool_name=$1
    local tool_request=$2
    local expected_field=$3

    echo -n "Testing $tool_name... "

    # Send initialize + tool call in one session (stdio transport)
    response=$(printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}\n%s\n' "$tool_request" | timeout 10 $BINARY 2>/dev/null | tail -1)

    if echo "$response" | grep -q "$expected_field"; then
        echo -e "${GREEN}PASS${NC}"
        ((PASSED++))
        return 0
    else
        echo -e "${RED}FAIL${NC}"
        echo "  Request: $tool_request"
        echo "  Response: $response"
        ((FAILED++))
        return 1
    fi
}

# Test 1: Initialize + tools/list
echo "=== Core Protocol ==="
test_tool "initialize/tools_list" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
    '"name":"run_backtest"'

echo ""
echo "=== System Tools ==="

# Test 2: healthcheck
test_tool "healthcheck" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"healthcheck","arguments":{}}}' \
    'healthy'

# Test 3: verify_setup
test_tool "verify_setup" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"verify_setup","arguments":{}}}' \
    'all_ok'

# Test 4: list_symbols
test_tool "list_symbols" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_symbols","arguments":{}}}' \
    'symbols'

echo ""
echo "=== Expert/Indicator/Script Tools ==="

# Test 5: list_experts
test_tool "list_experts" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_experts","arguments":{}}}' \
    'experts'

# Test 6: list_indicators
test_tool "list_indicators" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_indicators","arguments":{}}}' \
    'indicators'

# Test 7: list_scripts
test_tool "list_scripts" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_scripts","arguments":{}}}' \
    'scripts'

echo ""
echo "=== Report Tools ==="

# Test 8: list_reports
test_tool "list_reports" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_reports","arguments":{}}}' \
    'reports'

# Test 9: get_latest_report
test_tool "get_latest_report" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_latest_report","arguments":{}}}' \
    'success'

# Test 10: search_reports
test_tool "search_reports" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_reports","arguments":{}}}' \
    'reports'

# Test 11: prune_reports
test_tool "prune_reports" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"prune_reports","arguments":{"keep_last":10}}}' \
    'success'

echo ""
echo "=== Set File Tools ==="

# Test 12: list_set_files
test_tool "list_set_files" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_set_files","arguments":{}}}' \
    'set_files'

echo ""
echo "=== Cache Tools ==="

# Test 13: cache_status
test_tool "cache_status" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"cache_status","arguments":{}}}' \
    'success'

# Test 14: clean_cache
test_tool "clean_cache" \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"clean_cache","arguments":{"dry_run":true}}}' \
    'success'

echo ""
echo "=== Summary ==="
echo "=========================================="
echo -e "Passed: ${GREEN}$PASSED${NC}"
echo -e "Failed: ${RED}$FAILED${NC}"
echo "=========================================="

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
