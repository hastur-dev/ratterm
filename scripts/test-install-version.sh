#!/bin/bash
# Version-resolution tests for install.sh.
#
# Runs the real installer end to end against a stub `curl` on PATH, so no
# network and no GitHub release are needed. Each case asserts which release
# tag the installer actually asked GitHub to download.
#
# Usage: bash scripts/test-install-version.sh

set -u

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_SH="$REPO_ROOT/install.sh"
FALLBACK_VERSION=$(grep -m1 '^VERSION="' "$INSTALL_SH" | sed -E 's/^VERSION="([^"]+)"/\1/')

PASS=0
FAIL=0

fail() { echo "  FAIL: $1"; FAIL=$((FAIL + 1)); }
pass() { echo "  ok: $1"; PASS=$((PASS + 1)); }

# Build a sandbox with a stub curl. $1 = tag_name the fake API returns, or the
# literal string "api-error" to make the API call fail.
make_sandbox() {
    local api_tag="$1"
    local sandbox
    sandbox=$(mktemp -d)

    mkdir -p "$sandbox/stub" "$sandbox/home"

    cat > "$sandbox/stub/curl" <<STUB
#!/bin/bash
# Stub curl. Records every invocation, serves a canned API response and a
# dummy binary payload.
echo "\$@" >> "$sandbox/curl-calls.log"

out=""
prev=""
for arg in "\$@"; do
    if [ "\$prev" = "-o" ]; then out="\$arg"; fi
    prev="\$arg"
done

case "\$*" in
    *api.github.com*)
        if [ "$api_tag" = "api-error" ]; then
            echo "404 Not Found" >&2
            exit 22
        fi
        echo '{"tag_name": "$api_tag", "name": "release"}'
        ;;
    *releases/download*)
        # Any payload over the installer's 1000-byte sanity floor.
        head -c 4096 /dev/zero | tr '\\0' 'x' > "\$out"
        echo "200"
        ;;
    *)
        echo "unexpected curl call: \$*" >&2
        exit 1
        ;;
esac
STUB
    chmod +x "$sandbox/stub/curl"
    echo "$sandbox"
}

# $1 = test name, $2 = api tag (or "api-error"), $3 = expected version
# downloaded, $4.. = extra env assignments passed to the installer.
run_case() {
    local name="$1" api_tag="$2" expected="$3"
    shift 3

    local sandbox
    sandbox=$(make_sandbox "$api_tag")

    local output
    output=$(env -u RATTERM_VERSION "$@" \
        HOME="$sandbox/home" \
        RATTERM_INSTALL_DIR="$sandbox/home/.local/bin" \
        SHELL=/bin/bash \
        PATH="$sandbox/stub:$PATH" \
        bash "$INSTALL_SH" --verbose 2>&1)
    local status=$?

    echo "$name"

    if [ $status -ne 0 ]; then
        fail "$name: installer exited $status"
        echo "$output" | tail -5 | sed 's/^/    /'
        rm -rf "$sandbox"
        return
    fi

    local download_line
    download_line=$(grep 'releases/download' "$sandbox/curl-calls.log" 2>/dev/null | head -1)

    if [ -z "$download_line" ]; then
        fail "$name: installer never requested a release asset"
        rm -rf "$sandbox"
        return
    fi

    if echo "$download_line" | grep -q "/download/v${expected}/"; then
        pass "$name: downloaded v$expected"
    else
        fail "$name: expected v$expected, got: $download_line"
    fi

    if [ -x "$sandbox/home/.local/bin/rat" ]; then
        pass "$name: binary installed and executable"
    else
        fail "$name: binary missing at \$RATTERM_INSTALL_DIR/rat"
    fi

    rm -rf "$sandbox"
}

echo "install.sh version resolution (fallback constant: v$FALLBACK_VERSION)"
echo

# Expected path: the API's latest tag wins over the hardcoded fallback. This is
# the regression that shipped v0.2.1 during the v0.2.2 release.
run_case "api latest is used" "v9.9.9" "9.9.9"

# Pinned installs skip the API entirely.
run_case "RATTERM_VERSION pins the install" "v9.9.9" "1.2.3" RATTERM_VERSION=1.2.3
run_case "RATTERM_VERSION accepts a v prefix" "v9.9.9" "1.2.3" RATTERM_VERSION=v1.2.3

# API unreachable: fall back to the constant the release workflow bumps.
run_case "api failure falls back to constant" "api-error" "$FALLBACK_VERSION"

echo
echo "passed: $PASS  failed: $FAIL"
[ "$FAIL" -eq 0 ]
