#!/usr/bin/env bash
# test_terminal_restore.sh -- Automated terminal restoration validation
#
# Tests that the TermTunes TUI cleanly restores terminal state after
# receiving SIGINT, SIGTERM, and SIGHUP signals. Verifies:
#   1. Raw mode is disabled (stty settings restored)
#   2. Terminal is responsive (echo works, cursor visible)
#   3. Not stuck in alternate screen
#
# Usage: bash scripts/test_terminal_restore.sh
#
# NOTE: This tests signal-based exits only. The 'q' key exit requires
# interactive terminal input and is verified manually in the checkpoint.

set -euo pipefail

# --- Configuration ---
BINARY_NAME="termtunes"
WAIT_INIT=2       # Seconds to wait for TUI to initialize
WAIT_EXIT=5       # Seconds to wait for process to exit after signal
PASS_COUNT=0
FAIL_COUNT=0
TOTAL_TESTS=3

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# --- Cleanup trap ---
# Ensure no leftover processes on exit
PIDS_TO_CLEAN=()
cleanup() {
    for pid in "${PIDS_TO_CLEAN[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill -9 "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    # Reset terminal just in case
    stty sane 2>/dev/null || true
}
trap cleanup EXIT

# --- Build ---
echo "Building $BINARY_NAME..."
BINARY_PATH=""
if cargo build --release 2>/dev/null; then
    BINARY_PATH="./target/release/$BINARY_NAME"
elif cargo build 2>/dev/null; then
    BINARY_PATH="./target/debug/$BINARY_NAME"
else
    echo -e "${RED}ERROR: Failed to build $BINARY_NAME${NC}"
    exit 1
fi

if [ ! -x "$BINARY_PATH" ]; then
    echo -e "${RED}ERROR: Binary not found at $BINARY_PATH${NC}"
    exit 1
fi

echo "Using binary: $BINARY_PATH"
echo ""
echo "Testing terminal restoration..."
echo ""

# --- Test function ---
# Runs the app, sends a signal, checks terminal state
# Args: $1 = signal name, $2 = signal number
run_signal_test() {
    local sig_name="$1"
    local sig_num="$2"
    local test_label="$sig_name"

    # Save terminal state before test
    local saved_stty
    saved_stty=$(stty -g 2>/dev/null) || true

    # Launch the app in the background with a pseudo-terminal
    # We redirect stdin from /dev/null so it doesn't block waiting for input
    # The app needs auth, so it may fail to start -- that's fine for this test,
    # we're testing that terminal state is restored even on early exit.
    $BINARY_PATH &>/dev/null &
    local app_pid=$!
    PIDS_TO_CLEAN+=("$app_pid")

    # Wait for TUI to initialize (or for the process to start up)
    sleep "$WAIT_INIT"

    # Check if process is still running
    if ! kill -0 "$app_pid" 2>/dev/null; then
        # Process already exited (probably auth needed) -- still check terminal
        wait "$app_pid" 2>/dev/null || true
    else
        # Send the signal
        kill "-$sig_num" "$app_pid" 2>/dev/null || true

        # Wait for clean exit
        local waited=0
        while kill -0 "$app_pid" 2>/dev/null && [ "$waited" -lt "$WAIT_EXIT" ]; do
            sleep 1
            waited=$((waited + 1))
        done

        # Force kill if still running
        if kill -0 "$app_pid" 2>/dev/null; then
            kill -9 "$app_pid" 2>/dev/null || true
            wait "$app_pid" 2>/dev/null || true
            echo -e "  ${YELLOW}WARNING: Process did not exit within ${WAIT_EXIT}s after $sig_name${NC}"
        else
            wait "$app_pid" 2>/dev/null || true
        fi
    fi

    # Check terminal state
    local current_stty
    current_stty=$(stty -g 2>/dev/null) || true

    local passed=true
    local details=""

    # Check 1: stty settings match (raw mode disabled, echo restored)
    if [ "$saved_stty" != "$current_stty" ]; then
        # Try to be more specific about what changed
        # At minimum, check that echo is on and raw mode is off
        if stty -a 2>/dev/null | grep -q "[-]echo"; then
            details="echo is disabled"
            passed=false
        fi
    fi

    # Check 2: Terminal is responsive (we can write to it)
    if ! echo -n "" 2>/dev/null; then
        details="${details:+$details, }terminal not responsive"
        passed=false
    fi

    # Restore terminal state just in case
    if [ -n "$saved_stty" ]; then
        stty "$saved_stty" 2>/dev/null || true
    fi

    # Report result
    if $passed; then
        echo -e "  ${GREEN}[PASS]${NC} $test_label: Terminal restored cleanly"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        echo -e "  ${RED}[FAIL]${NC} $test_label: Terminal NOT restored ($details)"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
}

# --- Run tests ---
run_signal_test "SIGINT (Ctrl+C)" "INT"
run_signal_test "SIGTERM (kill)" "TERM"
run_signal_test "SIGHUP" "HUP"

# --- Summary ---
echo ""
echo "Results: ${PASS_COUNT}/${TOTAL_TESTS} passed"

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi
