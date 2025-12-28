#!/bin/bash
set -e

# Configuration
MOUNT_POINT="/tmp/magicfs-test-mount"
WATCH_DIR="/tmp/magicfs-test-data"
DB_PATH="/tmp/.magicfs/index.db"
BINARY="./target/debug/magicfs"
LOG_FILE="tests/magicfs.log"

# Force terminal sanity immediately
stty sane

# Setup/Cleanup Logic
cleanup() {
    set +e
    echo ""
    echo "Cleaning up..."
    
    # Kill keepalive if it exists
    if [ ! -z "$SUDO_KEEPALIVE_PID" ]; then
        kill $SUDO_KEEPALIVE_PID 2>/dev/null
    fi

    # Kill magicfs
    sudo pkill -9 -f "magicfs" > /dev/null 2>&1
    
    # Unmount
    if mount | grep -q "$MOUNT_POINT"; then
        sudo umount -l "$MOUNT_POINT" > /dev/null 2>&1
    fi

    # Remove data
    if [ -f "$DB_PATH" ]; then
        sudo rm -f "$DB_PATH"
    fi
    sudo rm -rf "$MOUNT_POINT"
    rm -rf "$WATCH_DIR"
    
    # FIX: Force terminal to behave correctly after cleanup
    stty sane
}
trap cleanup EXIT

# 0. Sudo Refresh (Keep alive in background)
echo "Acquiring sudo privileges..."
sudo -v
( while true; do sudo -v; sleep 60; done; ) > /dev/null 2>&1 &
SUDO_KEEPALIVE_PID=$!

# 1. Clean & Prepare
cleanup # Run cleanup once to ensure clean slate
mkdir -p "$MOUNT_POINT"
mkdir -p "$WATCH_DIR"

# 2. Build
echo "Building..."
if ! cargo build --quiet; then
    exit 1
fi

# 3. Launch MagicFS
# FIX: Use RUST_LOG=debug so we can see Librarian events in the logs
echo "Launching MagicFS (Debug Mode)..."
sudo RUST_LOG=debug $BINARY "$MOUNT_POINT" "$WATCH_DIR" > "$LOG_FILE" 2>&1 &
MAGIC_PID=$!

# Wait for startup
sleep 3

if ! pgrep -f "$BINARY" > /dev/null; then
    echo "MagicFS died on startup. Check logs."
    cat "$LOG_FILE"
    exit 1
fi

# 4. Run Tests
export PYTHONPATH=$PYTHONPATH:$(pwd)/tests

# Fix terminal again before tests just in case
stty sane

for test_file in tests/cases/*.py; do
    echo -e "\n>>> Running: $(basename "$test_file")"
    
    # Run test
    set +e
    python3 "$test_file" "$DB_PATH" "$MOUNT_POINT" "$WATCH_DIR"
    RESULT=$?
    set -e
    
    if [ $RESULT -ne 0 ]; then
        echo "❌ TEST FAILED"
        echo "--- LOG DUMP (Last 50 lines) ---"
        tail -n 50 "$LOG_FILE"
        exit 1
    fi
done

echo -e "\n✅ ALL TESTS PASSED"
