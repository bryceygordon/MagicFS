#!/bin/bash
set -e # Exit on error

# Configuration
MOUNT_POINT="/tmp/magicfs-test-mount"
WATCH_DIR="/tmp/magicfs-test-data"
DB_PATH="/tmp/.magicfs/index.db"
BINARY="./target/debug/magicfs"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== MagicFS Test Suite ===${NC}"

# 0. Sudo Refresh (THE FIX)
# This prompts for password properly in the foreground, caching it for later.
echo "Acquiring sudo privileges..."
if ! sudo -v; then
    echo -e "${RED}Sudo authentication failed.${NC}"
    exit 1
fi

# Keep sudo alive in the background while script runs (optional but safer)
( while true; do sudo -v; sleep 60; done; ) &
SUDO_KEEPALIVE_PID=$!

# 1. Cleanup Function
cleanup() {
    echo "Cleaning up..."
    # Kill the sudo keepalive
    kill $SUDO_KEEPALIVE_PID 2>/dev/null || true
    
    # Kill any running magicfs instances
    sudo pkill -f "target/debug/magicfs" || true
    
    # Unmount if mounted
    if mount | grep -q "$MOUNT_POINT"; then
        sudo fusermount3 -u "$MOUNT_POINT"
    fi
    
    # Clean DB and directories
    # Note: We remove the database to ensure we aren't reading old index data
    if [ -f "$DB_PATH" ]; then
        sudo rm -f "$DB_PATH"
        echo "Deleted old database."
    fi
    
    sudo rm -rf "$MOUNT_POINT"
    rm -rf "$WATCH_DIR"
}

# Trap cleanup to run on exit or interrupt
trap cleanup EXIT

# 2. Build the project
echo "Building MagicFS..."
cargo build
if [ $? -ne 0 ]; then
    echo -e "${RED}Build failed.${NC}"
    exit 1
fi

# 3. Setup Test Data
# (We wipe the directory first to ensure no old files exist)
rm -rf "$WATCH_DIR"
mkdir -p "$MOUNT_POINT"
mkdir -p "$WATCH_DIR"
mkdir -p "$WATCH_DIR/.git"
mkdir -p "$WATCH_DIR/.obsidian"
mkdir -p "$WATCH_DIR/Projects"

# -- Create Visible Files --
echo "This is a python script about snake games." > "$WATCH_DIR/game.py"
echo "Rust is a systems programming language." > "$WATCH_DIR/Projects/main.rs"

# -- Create Hidden/Ignored Files --
echo "Secret git config" > "$WATCH_DIR/.git/config"
echo "Obsidian workspace settings" > "$WATCH_DIR/.obsidian/workspace.json"

# 4. Run MagicFS
echo -e "${GREEN}Launching MagicFS...${NC}"
# Sudo is already cached, so this background command won't prompt
sudo RUST_LOG=info $BINARY "$MOUNT_POINT" "$WATCH_DIR" &
MAGIC_PID=$!

# 5. Wait for initialization
echo "Waiting for system to mount and index (5 seconds)..."
sleep 5

# Check if process is still running
if ! ps -p $MAGIC_PID > /dev/null; then
    echo -e "${RED}MagicFS process died unexpectedly! Check logs.${NC}"
    exit 1
fi

# 6. Run Verification Script
echo -e "${GREEN}Running Verification Logic...${NC}"
python3 tests/verify.py "$DB_PATH" "$MOUNT_POINT"

TEST_EXIT_CODE=$?

if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✅ ALL TESTS PASSED${NC}"
else
    echo -e "${RED}❌ TESTS FAILED${NC}"
fi

exit $TEST_EXIT_CODE
