#!/bin/bash

# ==================================================================================
# ⚠️  CRITICAL TERMINAL CONTROL SECTION ⚠️
# ==================================================================================
SAVED_TERM=$(stty -g)

restore_term() {
    # Try to restore exact state, fall back to sane defaults if that fails
    stty "$SAVED_TERM" 2>/dev/null || stty sane
}

trap restore_term EXIT INT TERM HUP
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

    sudo pkill -15 -x magicfs 2>/dev/null
    sleep 1
    sudo pkill -9 -x magicfs 2>/dev/null
    
    if mount | grep -q "$MOUNT_POINT"; then
        sudo umount -l "$MOUNT_POINT" 2>/dev/null
    fi

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
sudo nohup $BINARY "$MOUNT_POINT" "$WATCH_DIR" > "$LOG_FILE" 2>&1 &
sleep 2

# 4. Run Tests
export PYTHONPATH=$PYTHONPATH:$(pwd)/tests

for test_file in tests/cases/*.py; do
    restore_term
    echo -e "\n>>> Running: $(basename "$test_file")"
    
    # Run unbuffered
    python3 -u "$test_file" "$DB_PATH" "$MOUNT_POINT" "$WATCH_DIR"
    RESULT=$?
    
    if [ $RESULT -ne 0 ]; then
        restore_term
        echo -e "\n❌ TEST FAILED: $(basename "$test_file")"
        # Logs are now dumped by the Python script itself
        exit 1
    fi
done

echo -e "\n✅ ALL TESTS PASSED"
