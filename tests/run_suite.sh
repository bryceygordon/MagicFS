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
BLUE='\033[0;34m'
NC='\033[0m'

# Disable job control messages
set +m
stty sane

echo -e "${GREEN}=== MagicFS Modular Test Suite ===${NC}"

# 0. Sudo Refresh
echo "Acquiring sudo privileges..."
if ! sudo -v; then
    echo -e "${RED}Sudo authentication failed.${NC}"
    exit 1
fi
( while true; do sudo -v; sleep 60; done; ) > /dev/null 2>&1 &
SUDO_KEEPALIVE_PID=$!
disown $SUDO_KEEPALIVE_PID

# 1. Cleanup Function
cleanup() {
    set +m 
    echo ""
    echo "Cleaning up..."
    kill $SUDO_KEEPALIVE_PID 2>/dev/null || true
    sudo pkill -9 -f "magicfs" > /dev/null 2>&1 || true
    if mount | grep -q "$MOUNT_POINT"; then
        sudo umount -l "$MOUNT_POINT" > /dev/null 2>&1 || true
    fi
    if [ -f "$DB_PATH" ]; then
        sudo rm -f "$DB_PATH"
    fi
    sudo rm -rf "$MOUNT_POINT"
    rm -rf "$WATCH_DIR"
    stty sane
}
trap cleanup EXIT

# Clean zombies first
cleanup

# 2. Build
echo "Building MagicFS..."
if ! cargo build --quiet; then
    echo -e "${RED}Build failed.${NC}"
    exit 1
fi

# 3. Setup Test Environment (The "Super Set" of data)
mkdir -p "$MOUNT_POINT"
mkdir -p "$WATCH_DIR"
mkdir -p "$WATCH_DIR/.git"
mkdir -p "$WATCH_DIR/.obsidian"
mkdir -p "$WATCH_DIR/Projects"

# -- Data for Indexing Test --
echo "This is a python script about snake games." > "$WATCH_DIR/game.py"
echo "Rust is a systems programming language." > "$WATCH_DIR/Projects/main.rs"

# -- Data for Ignore Test --
echo "Secret git config" > "$WATCH_DIR/.git/config"
echo "Obsidian workspace settings" > "$WATCH_DIR/.obsidian/workspace.json"

# -- Data for Ignore Test (Custom Ignore Rule) --
# This directory DOES NOT start with a dot. Standard logic would index it.
mkdir -p "$WATCH_DIR/secrets"
echo "Super sensitive password" > "$WATCH_DIR/secrets/passwords.txt"

# -- Create .magicfsignore --
echo ".git" > "$WATCH_DIR/.magicfsignore"
echo ".obsidian" >> "$WATCH_DIR/.magicfsignore"
echo "secrets" >> "$WATCH_DIR/.magicfsignore" # <--- The new rule

# -- Create .magicfsignore --
echo ".git" > "$WATCH_DIR/.magicfsignore"
echo ".obsidian" >> "$WATCH_DIR/.magicfsignore"

# 4. Launch MagicFS
echo -e "${GREEN}Launching MagicFS... (Logs -> $LOG_FILE)${NC}"
sudo RUST_LOG=info $BINARY "$MOUNT_POINT" "$WATCH_DIR" > "$LOG_FILE" 2>&1 &
MAGIC_PID=$!
disown $MAGIC_PID

echo "Waiting for system to mount and index (5 seconds)..."
sleep 5

# Check survival
if ! pgrep -f "$BINARY" > /dev/null; then
    echo -e "${RED}MagicFS died on startup! Dumping log:${NC}"
    cat "$LOG_FILE"
    exit 1
fi

# 5. Run Test Cases
echo -e "${BLUE}Starting Test Cases...${NC}"

# We need to export PYTHONPATH so tests can find common.py
export PYTHONPATH=$PYTHONPATH:$(pwd)/tests

# Loop through all python files in tests/cases/
for test_file in tests/cases/*.py; do
    echo -e "\n${BLUE}>>> Running: $(basename "$test_file")${NC}"
    
    # Run the test, passing DB path and Mount point
    # We turn off set -e temporarily to capture the failure
    set +e
    python3 "$test_file" "$DB_PATH" "$MOUNT_POINT"
    RESULT=$?
    set -e
    
    if [ $RESULT -ne 0 ]; then
        echo -e "${RED}❌ TEST FAILED: $(basename "$test_file")${NC}"
        echo -e "${RED}Dumping MagicFS Log:${NC}"
        echo "---------------------------------------------------"
        cat "$LOG_FILE"
        echo "---------------------------------------------------"
        exit 1
    fi
done

echo -e "\n${GREEN}✅ ALL TEST CASES PASSED${NC}"
exit 0
