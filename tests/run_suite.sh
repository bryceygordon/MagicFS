#!/bin/bash

# ==================================================================================
# ⚠️  CRITICAL TERMINAL CONTROL SECTION - DO NOT REMOVE  ⚠️
# ==================================================================================
SAVED_TERM=$(stty -g)

restore_term() {
    stty "$SAVED_TERM" 2>/dev/null || stty sane
}

trap restore_term EXIT INT TERM HUP
# ==================================================================================

# Configuration
MOUNT_POINT="/tmp/magicfs-test-mount"
WATCH_DIR="/tmp/magicfs-test-data"
# --- UPDATED: MATCHING MAIN.RS ISOLATION PATH (Nomic v1.5) ---
DB_PATH="/tmp/.magicfs_nomic/index.db"
BINARY="./target/debug/magicfs"
LOG_FILE="tests/magicfs.log"
# NEW: System data directory for Phase 17
SYSTEM_DATA_DIR="/tmp/magicfs-test-system"

# FIX: Add 'tests' directory to PYTHONPATH so cases can import 'common'
export PYTHONPATH=$(pwd)/tests

# FIX: Export the log file path so common.py reads the CORRECT log on failure
export MAGICFS_LOG_FILE=$(pwd)/"$LOG_FILE"

# NEW: Export system data directory for Phase 17
export MAGICFS_DATA_DIR="$SYSTEM_DATA_DIR"
export RUST_LOG=debug

# Keep sudo alive
sudo -v
( while true; do sudo -v; sleep 60; done; ) > /dev/null 2>&1 &
SUDO_KEEPALIVE_PID=$!

cleanup_environment() {
    # 1. Kill Daemon
    sudo pkill -15 -x magicfs 2>/dev/null
    # sleep 0.5
    sudo pkill -9 -x magicfs 2>/dev/null

    # 2. Force Unmount (The Zombie Fix)
    if mount | grep -q "$MOUNT_POINT"; then
        sudo umount -l "$MOUNT_POINT" 2>/dev/null
    fi

    # 3. Wipe Data
    sudo rm -f "$DB_PATH" 2>/dev/null
    sudo rm -rf "$MOUNT_POINT" "$WATCH_DIR" "$SYSTEM_DATA_DIR" 2>/dev/null
    # Ensure parent dir exists
    mkdir -p "$(dirname "$DB_PATH")"

    # 4. Recreate Dirs
    mkdir -p "$MOUNT_POINT" "$WATCH_DIR" "$SYSTEM_DATA_DIR"
}

# 1. Build
echo "[Build] Compiling MagicFS (Nomic Edition)..."
cargo build --quiet || exit 1

# 2. Run Rust Unit Tests
echo "[Unit] Running Rust Unit Tests..."
cargo test --lib --quiet || exit 1
echo "✅ Unit Tests Passed"

# 3. Run Test Suite
echo "[Suite] Starting Isolation Runner..."

# Sort tests to run from newest to oldest, but ensure test_99 runs last
# Get all test files except test_99, sort them in reverse order
mapfile -t REGULAR_TESTS < <(ls tests/cases/test_*.py 2>/dev/null | grep -v test_99 | sort -r)
# Get test_99 files specifically
mapfile -t TEST_99_FILES < <(ls tests/cases/test_99_*.py 2>/dev/null | sort)
# Combine arrays
ALL_TESTS=("${REGULAR_TESTS[@]}" "${TEST_99_FILES[@]}")

for test_file in "${ALL_TESTS[@]}"; do
    restore_term
    TEST_NAME=$(basename "$test_file")
    echo -e "\n>>> 🧪 Running: $TEST_NAME"
    
    # A. Clean Slate
    cleanup_environment
    
    # B. Launch Daemon (Fresh Instance)
    # We truncate the log file for each test to make debugging easier
    sudo -E nohup $BINARY "$MOUNT_POINT" "$WATCH_DIR" > "$LOG_FILE" 2>&1 &
    
    # Wait for daemon to stabilize (HollowDrive ready)
    # Nomic might take a moment to download on first run
    sleep 3
    
    # C. Run Test
    # Run unbuffered (-u)
    python3 -u "$test_file" "$DB_PATH" "$MOUNT_POINT" "$WATCH_DIR"
    RESULT=$?
    
    # D. Check Result
    if [ $RESULT -ne 0 ]; then
        restore_term
        echo -e "\n❌ TEST FAILED: $TEST_NAME"
        echo "================================================================"
        echo "📜  MAGICFS LOG DUMP (Last 100 lines)"
        echo "================================================================"
        if [ -f "$LOG_FILE" ]; then
            tail -n 100 "$LOG_FILE"
        else
            echo "⚠️  Log file not found!"
        fi
        echo "================================================================"
        
        # Cleanup before exit
        cleanup_environment
        exit 1
    fi
    
    echo "✅ Passed: $TEST_NAME"
done

# Final Cleanup
cleanup_environment
if [ ! -z "$SUDO_KEEPALIVE_PID" ]; then
    kill "$SUDO_KEEPALIVE_PID" 2>/dev/null
fi

echo -e "\n🎉 ALL TESTS PASSED SUCCESSFULLY"
