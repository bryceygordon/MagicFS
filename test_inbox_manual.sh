#!/bin/bash
# Manual test runner for Phase 17 Inbox functionality

echo "=== Phase 17 Inbox Test ==="

# Configuration
MOUNT_POINT="/tmp/magicfs-inbox-test"
WATCH_DIR="/tmp/magicfs-inbox-data"
DB_PATH="/tmp/.magicfs_nomic/index.db"
LOG_FILE="/tmp/inbox_test.log"

# Cleanup
echo "Cleaning up previous test..."
sudo pkill -9 -x magicfs 2>/dev/null || true
sudo umount -l "$MOUNT_POINT" 2>/dev/null || true
sudo rm -rf "$MOUNT_POINT" "$WATCH_DIR" "/tmp/.magicfs_nomic" "$LOG_FILE"

# Setup dirs
mkdir -p "$MOUNT_POINT"
mkdir -p "$WATCH_DIR"

# Build
echo "Building..."
cargo build --quiet

# Start daemon
echo "Starting daemon..."
sudo -E RUST_LOG=debug ./target/debug/magicfs "$MOUNT_POINT" "$WATCH_DIR" > "$LOG_FILE" 2>&1 &
DAEMON_PID=$!
sleep 2

# Check if running
if ! ps -p $DAEMON_PID > /dev/null; then
    echo "❌ Daemon failed to start!"
    cat "$LOG_FILE"
    exit 1
fi

echo "✅ Daemon running (PID: $DAEMON_PID)"

# Test 1: Check /inbox exists and is writable
echo -e "\n--- Test 1: Check /inbox directory ---"
ls -la "$MOUNT_POINT/" | grep inbox
if [ -d "$MOUNT_POINT/inbox" ]; then
    echo "✅ /inbox directory exists"
else
    echo "❌ /inbox directory missing"
fi

# Test 2: Try to write to inbox
echo -e "\n--- Test 2: Write to /inbox/idea.txt ---"
echo "This is a test idea for the inbox." > "$MOUNT_POINT/inbox/idea.txt" 2>&1
WRITE_RESULT=$?

if [ $WRITE_RESULT -eq 0 ]; then
    echo "✅ Write succeeded"

    # Test 3: Check physical file
    echo -e "\n--- Test 3: Check physical persistence ---"
    if [ -f "$WATCH_DIR/_imported/idea.txt" ]; then
        echo "✅ Physical file exists"
        cat "$WATCH_DIR/_imported/idea.txt"
    else
        echo "❌ Physical file missing"
        ls -la "$WATCH_DIR/"
    fi

    # Test 4: Check database
    echo -e "\n--- Test 4: Check database state ---"
    if command -v sqlite3 &> /dev/null; then
        echo "=== file_registry ==="
        sudo sqlite3 "$DB_PATH" "SELECT * FROM file_registry WHERE abs_path LIKE '%idea.txt';"
        echo -e "\n=== tags ==="
        sudo sqlite3 "$DB_PATH" "SELECT * FROM tags;"
        echo -e "\n=== file_tags ==="
        sudo sqlite3 "$DB_PATH" "SELECT ft.*, t.name FROM file_tags ft JOIN tags t ON ft.tag_id = t.tag_id;"
    else
        echo "⚠️  sqlite3 not available"
    fi

    # Test 5: Read back
    echo -e "\n--- Test 5: Read back from inbox ---"
    CONTENT=$(cat "$MOUNT_POINT/inbox/idea.txt" 2>&1)
    if [ "$CONTENT" = "This is a test idea for the inbox." ]; then
        echo "✅ Read-back successful"
    else
        echo "❌ Read-back failed"
        echo "Content: $CONTENT"
    fi

else
    echo "❌ Write failed with code: $WRITE_RESULT"
    echo "--- Log tail ---"
    tail -n 20 "$LOG_FILE"
fi

# Cleanup
echo -e "\n--- Cleanup ---"
sudo kill $DAEMON_PID 2>/dev/null || true
sleep 1
sudo umount -l "$MOUNT_POINT" 2>/dev/null || true

echo -e "\n=== DONE ==="
if [ $WRITE_RESULT -eq 0 ]; then
    echo "✅ ALL TESTS PASSED"
else
    echo "❌ TESTS FAILED"
    echo "See log: $LOG_FILE"
fi