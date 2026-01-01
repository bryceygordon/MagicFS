#!/bin/bash
TEST_FILE=$1
if [ -z "$TEST_FILE" ]; then
    echo "Usage: ./tests/run_single.sh tests/cases/test_11_passthrough.py"
    exit 1
fi

# --- NEW LOGGING CONFIGURATION ---
LOG_FILE="/tmp/magicfs_debug.log"

# Create file and force permissions so both sudo (daemon) and user (python) can access it
rm -f "$LOG_FILE"
touch "$LOG_FILE"
chmod 666 "$LOG_FILE"

# Cleanup
sudo pkill -9 -x magicfs 2>/dev/null
sudo umount -l "/tmp/magicfs-test-mount" 2>/dev/null
sudo rm -rf "/tmp/magicfs-test-mount" "/tmp/magicfs-test-data" "/tmp/.magicfs"
mkdir -p "/tmp/magicfs-test-mount" "/tmp/magicfs-test-data"

# Build
cargo build --quiet

# Start Daemon
# -E preserves RUST_LOG
export RUST_LOG=debug 
sudo -E nohup ./target/debug/magicfs "/tmp/magicfs-test-mount" "/tmp/magicfs-test-data" > "$LOG_FILE" 2>&1 &
DAEMON_PID=$!
sleep 2

# Check if daemon died immediately
if ! ps -p $DAEMON_PID > /dev/null; then
    echo "❌ CRITICAL: Daemon died immediately on startup!"
    echo "--- LOG START ---"
    cat "$LOG_FILE"
    echo "--- LOG END ---"
    exit 1
fi

# Run Test
export PYTHONPATH=$(pwd)/tests
export MAGICFS_LOG_FILE="$LOG_FILE" # Tell Python where to look
python3 -u "$TEST_FILE" "/tmp/.magicfs/index.db" "/tmp/magicfs-test-mount" "/tmp/magicfs-test-data"
