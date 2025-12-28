#!/bin/bash
set -e

# Configuration
MOUNT_POINT="/tmp/magicfs-test-mount"
WATCH_DIR="/tmp/magicfs-test-data"
DB_PATH="/tmp/.magicfs/index.db"
BINARY="./target/debug/magicfs"
LOG_FILE="tests/magicfs.log"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

# FORCE SANE TERMINAL AT START
stty sane

echo -e "${GREEN}=== MagicFS Test Suite ===${NC}"

# 0. Sudo Refresh
echo "Acquiring sudo privileges..."
if ! sudo -v; then
    echo -e "${RED}Sudo authentication failed.${NC}"
    exit 1
fi
# Keep sudo alive in background (silenced stdout/stderr)
( while true; do sudo -v; sleep 60; done; ) > /dev/null 2>&1 &
SUDO_KEEPALIVE_PID=$!

# 1. Cleanup Function
cleanup() {
    # CRITICAL: Fix terminal state immediately upon exit/interrupt
    stty sane 

    echo "" # Force a newline
    echo "Cleaning up..."
    
    kill $SUDO_KEEPALIVE_PID 2>/dev/null || true
    
    # Kill magicfs aggressively
    sudo pkill -9 -f "magicfs" || true
    
    # Lazy unmount
    if mount | grep -q "$MOUNT_POINT"; then
        sudo umount -l "$MOUNT_POINT" || true
    fi
    
    # Remove test artifacts
    if [ -f "$DB_PATH" ]; then
        sudo rm -f "$DB_PATH"
    fi
    sudo rm -rf "$MOUNT_POINT"
    rm -rf "$WATCH_DIR"
}

trap cleanup EXIT

# Run pre-cleanup to kill zombies
cleanup

# 2. Build
echo -e "Building MagicFS..."
# Check if build succeeds
if ! cargo build --quiet; then
    echo -e "${RED}Build failed.${NC}"
    exit 1
fi

# 3. Setup Data
mkdir -p "$MOUNT_POINT"
mkdir -p "$WATCH_DIR"
mkdir -p "$WATCH_DIR/.git"
mkdir -p "$WATCH_DIR/.obsidian"
mkdir -p "$WATCH_DIR/Projects"

echo "This is a python script about snake games." > "$WATCH_DIR/game.py"
echo "Rust is a systems programming language." > "$WATCH_DIR/Projects/main.rs"
echo "Secret git config" > "$WATCH_DIR/.git/config"
echo "Obsidian workspace settings" > "$WATCH_DIR/.obsidian/workspace.json"

# 4. Run MagicFS
echo -e "${GREEN}Launching MagicFS... (Logs -> $LOG_FILE)${NC}"
# Redirect both stdout and stderr to file to keep terminal clean
sudo RUST_LOG=info $BINARY "$MOUNT_POINT" "$WATCH_DIR" > "$LOG_FILE" 2>&1 &
MAGIC_PID=$!

echo "Waiting for system to mount and index (5 seconds)..."
sleep 5

# Check if alive
if ! ps -p $MAGIC_PID > /dev/null; then
    echo -e "${RED}MagicFS died! Dumping log:${NC}"
    cat "$LOG_FILE"
    exit 1
fi

# 5. Verify
# CRITICAL FIX: Force terminal sanity before python takes over stdout
stty sane

echo -e "${GREEN}Running Verification Logic...${NC}"

set +e # Turn off exit-on-error for python step
python3 tests/verify.py "$DB_PATH" "$MOUNT_POINT"
TEST_EXIT_CODE=$?
set -e

if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✅ ALL TESTS PASSED${NC}"
else
    echo -e "${RED}❌ TESTS FAILED. Dumping MagicFS Log:${NC}"
    echo "---------------------------------------------------"
    cat "$LOG_FILE"
    echo "---------------------------------------------------"
fi

exit $TEST_EXIT_CODE
