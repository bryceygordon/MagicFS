#!/bin/bash

# ==================================================================================
# ⚠️  CRITICAL TERMINAL CONTROL SECTION - DO NOT REMOVE OR MODIFY  ⚠️
# ==================================================================================
# The following block prevents "laddering" (stair-step output) which renders
# logs unreadable. It saves the TTY state before tests run and FORCES restoration
# upon any exit signal.
#
# DO NOT DELETE 'SAVED_TERM' OR THE TRAP FUNCTION.
# DO NOT RELY ON 'reset' (It clears the scrollback, hiding debug info).
# ==================================================================================

# 1. Save the current sane terminal state
SAVED_TERM=$(stty -g)

# 2. Define the restoration function
restore_term() {
    # Silence errors in case TTY is already gone
    stty "$SAVED_TERM" 2>/dev/null
}

# 3. Trap EVERYTHING. If this script dies, the terminal MUST be restored.
trap restore_term EXIT INT TERM HUP

# ==================================================================================
# END CRITICAL SECTION
# ==================================================================================

# Configuration
MOUNT_POINT="/tmp/magicfs-test-mount"
WATCH_DIR="/tmp/magicfs-test-data"
DB_PATH="/tmp/.magicfs/index.db"
BINARY="./target/debug/magicfs"
LOG_FILE="tests/magicfs.log"

cleanup() {
    set +e
    echo -e "\n[Cleanup] Tearing down..."
    
    if [ ! -z "$SUDO_KEEPALIVE_PID" ]; then
        kill "$SUDO_KEEPALIVE_PID" 2>/dev/null
    fi

    # Kill daemon - use SIGTERM first, then SIGKILL
    sudo pkill -15 -x magicfs 2>/dev/null
    sleep 1
    sudo pkill -9 -x magicfs 2>/dev/null
    
    # Unmount
    if mount | grep -q "$MOUNT_POINT"; then
        sudo umount -l "$MOUNT_POINT" 2>/dev/null
    fi

    # Wipe
    sudo rm -f "$DB_PATH" 2>/dev/null
    sudo rm -rf "$MOUNT_POINT" "$WATCH_DIR" 2>/dev/null
    
    set -e
}

# 1. Setup
cleanup
mkdir -p "$MOUNT_POINT" "$WATCH_DIR"
sudo -v
( while true; do sudo -v; sleep 60; done; ) > /dev/null 2>&1 &
SUDO_KEEPALIVE_PID=$!

# 2. Build
echo "[Build] Compiling MagicFS..."
cargo build --quiet || exit 1

# 3. Launch
echo "[Launch] Starting Daemon..."
# Use 'nohup' and '&' to detach the process more cleanly from the TTY
sudo nohup $BINARY "$MOUNT_POINT" "$WATCH_DIR" > "$LOG_FILE" 2>&1 &
sleep 2

# 4. Run Tests
export PYTHONPATH=$PYTHONPATH:$(pwd)/tests

for test_file in tests/cases/*.py; do
    # Force restore before every test just in case the previous one leaked state
    restore_term
    
    echo -e "\n>>> Running: $(basename "$test_file")"
    
    # Run python unbuffered (-u) to ensure logs flow immediately
    python3 -u "$test_file" "$DB_PATH" "$MOUNT_POINT" "$WATCH_DIR"
    RESULT=$?
    
    if [ $RESULT -ne 0 ]; then
        restore_term
        echo -e "\n❌ TEST FAILED: $(basename "$test_file")"
        echo "--- LOG SUMMARY (Last 100 lines) ---"
        if [ -f "$LOG_FILE" ]; then
            tail -n 100 "$LOG_FILE"
        fi
        exit 1
    fi
done

echo -e "\n✅ ALL TESTS PASSED"
