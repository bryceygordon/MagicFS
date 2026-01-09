#!/bin/bash
TEST_FILE=$1
if [ -z "$TEST_FILE" ]; then
    echo "Usage: ./tests/run_single.sh tests/cases/test_11_passthrough.py"
    exit 1
fi

# ==================================================================================
# ⚠️  CRITICAL TERMINAL CONTROL SECTION
# ==================================================================================
SAVED_TERM=$(stty -g)

restore_term() {
    stty "$SAVED_TERM" 2>/dev/null || stty sane
}

trap restore_term EXIT INT TERM HUP
# ==================================================================================

# --- NEW LOGGING CONFIGURATION ---
LOG_FILE="/tmp/magicfs_debug.log"

# Create file and force permissions so both sudo (daemon) and user (python) can access it
rm -f "$LOG_FILE"
touch "$LOG_FILE"
chmod 666 "$LOG_FILE"

# --- CONFIGURATION (MATCHING MAIN.RS) ---
MOUNT_POINT="/tmp/magicfs-test-mount"
WATCH_DIR="/tmp/magicfs-test-data"
# FIX: Use the correct Nomic DB path
DB_PATH="/tmp/.magicfs_nomic/index.db"
# NEW: System data directory for Phase 17 (system-managed inbox)
SYSTEM_DATA_DIR="/tmp/magicfs-test-system"

# Cleanup
sudo pkill -9 -x magicfs 2>/dev/null
sudo umount -l "$MOUNT_POINT" 2>/dev/null
sudo rm -rf "$MOUNT_POINT" "$WATCH_DIR" "/tmp/.magicfs" "/tmp/.magicfs_nomic" "$SYSTEM_DATA_DIR"
mkdir -p "$MOUNT_POINT" "$WATCH_DIR" "$SYSTEM_DATA_DIR"

# Build
cargo build --quiet

# Start Daemon
# -E preserves RUST_LOG and other environment variables
export RUST_LOG=debug
# NEW: Export system data directory for Phase 17
export MAGICFS_DATA_DIR="$SYSTEM_DATA_DIR"
sudo -E nohup ./target/debug/magicfs "$MOUNT_POINT" "$WATCH_DIR" > "$LOG_FILE" 2>&1 &
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
# FIX: Pass the correct DB_PATH
python3 -u "$TEST_FILE" "$DB_PATH" "$MOUNT_POINT" "$WATCH_DIR"
