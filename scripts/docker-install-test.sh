#!/bin/bash
# Docker-based install script test runner
set -e

echo ""
echo "==========================================="
echo "     Install Script Test Suite"
echo "==========================================="
echo ""

PASS=0
FAIL=0

pass() {
    echo "[PASS] $1"
    PASS=$((PASS + 1))
}

fail() {
    echo "[FAIL] $1"
    FAIL=$((FAIL + 1))
}

# Test 1: Syntax check
echo "[TEST] Bash syntax validation..."
if bash -n /test/install.sh 2>/dev/null; then
    pass "Syntax is valid"
else
    fail "Syntax errors found"
fi

# Test 2: Required functions exist
echo ""
echo "[TEST] Required functions..."
required_functions="detect_platform get_latest_version download install_binary setup_path verify uninstall main"
for func in $required_functions; do
    # Check for function definition (name followed by () with optional spaces)
    if grep -qE "^${func}[[:space:]]*\(\)" /test/install.sh; then
        pass "Function '$func' exists"
    else
        fail "Function '$func' missing"
    fi
done

# Test 3: Required variables
echo ""
echo "[TEST] Required variables..."
required_vars="VERSION REPO BINARY_NAME INSTALL_DIR"
for var in $required_vars; do
    if grep -q "^${var}=" /test/install.sh; then
        pass "Variable '$var' defined"
    else
        fail "Variable '$var' missing"
    fi
done

# Test 4: Error handling
echo ""
echo "[TEST] Error handling..."
if grep -q "set -e" /test/install.sh; then
    pass "Error exit enabled (set -e)"
else
    fail "Missing 'set -e'"
fi

# Test 5: Platform detection logic
echo ""
echo "[TEST] Platform detection..."
if grep -q 'uname -s' /test/install.sh && grep -q 'uname -m' /test/install.sh; then
    pass "Platform detection uses uname"
else
    fail "Platform detection missing"
fi

# Test 6: Supported architectures
echo ""
echo "[TEST] Architecture support..."
if grep -q "x86_64\|amd64" /test/install.sh; then
    pass "x86_64 architecture supported"
else
    fail "x86_64 not supported"
fi

# Test 7: Supported operating systems
echo ""
echo "[TEST] OS support..."
if grep -q "Linux" /test/install.sh; then
    pass "Linux supported"
else
    fail "Linux not supported"
fi

if grep -q "Darwin" /test/install.sh; then
    pass "macOS supported"
else
    fail "macOS not supported"
fi

# Test 8: Download tools
echo ""
echo "[TEST] Download tool support..."
if grep -q "curl" /test/install.sh; then
    pass "curl download supported"
else
    fail "curl not supported"
fi

if grep -q "wget" /test/install.sh; then
    pass "wget fallback supported"
else
    fail "wget fallback not supported"
fi

# Test 9: Uninstall support
echo ""
echo "[TEST] Uninstall support..."
if grep -q "\-\-uninstall\|\-u" /test/install.sh; then
    pass "Uninstall flag supported"
else
    fail "Uninstall flag missing"
fi

# Test 10: Verbose mode
echo ""
echo "[TEST] Verbose mode..."
if grep -q "\-\-verbose\|\-v" /test/install.sh; then
    pass "Verbose flag supported"
else
    fail "Verbose flag missing"
fi

# Test 11: PATH setup
echo ""
echo "[TEST] PATH configuration..."
if grep -q "\.bashrc\|\.zshrc" /test/install.sh; then
    pass "Shell RC file modification supported"
else
    fail "Shell RC file modification missing"
fi

# Summary
echo ""
echo "==========================================="
echo "              TEST SUMMARY"
echo "==========================================="
echo ""
echo "Passed: $PASS"
echo "Failed: $FAIL"
echo ""

if [ $FAIL -eq 0 ]; then
    echo "[SUCCESS] All tests passed!"
    exit 0
else
    echo "[ERROR] $FAIL test(s) failed"
    exit 1
fi
