#!/bin/bash

# Configuration
MOUNT_POINT="/tmp/magicfs-test-mount"
WATCH_DIR="/tmp/magicfs-test-data"
DB_PATH="/tmp/.magicfs/index.db"
BINARY="./target/debug/magicfs"
LOG_FILE="tests/magicfs.log"

# Violent reset to kill laddering
reset_term() {
    # 'reset' is the nuclear option for terminal garbling
    reset -Q 2>/dev/null || stty sane
}
trap reset_term EXIT

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
    
    reset_term
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
    # Before every test, ensure terminal is sane
    reset_term
    echo -e "\n>>> Running: $(basename "$test_file")"
    python3 "$test_file" "$DB_PATH" "$MOUNT_POINT" "$WATCH_DIR"
    RESULT=$?
    
    if [ $RESULT -ne 0 ]; then
        reset_term
        echo -e "\n❌ TEST FAILED: $(basename "$test_file")"
        echo "--- LOG SUMMARY ---"
        if [ -f "$LOG_FILE" ]; then
            tail -n 50 "$LOG_FILE" | uniq -c
        fi
        exit 1
    fi
done

reset_term
echo -e "\n✅ ALL TESTS PASSED"
