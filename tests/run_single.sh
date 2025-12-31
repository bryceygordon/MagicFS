#!/bin/bash
TEST_FILE=$1
if [ -z "$TEST_FILE" ]; then
    echo "Usage: ./tests/run_single.sh tests/cases/test_09_chatter.py"
    exit 1
fi

# Cleanup
sudo pkill -9 -x magicfs 2>/dev/null
sudo umount -l "/tmp/magicfs-test-mount" 2>/dev/null
sudo rm -rf "/tmp/magicfs-test-mount" "/tmp/magicfs-test-data" "/tmp/.magicfs"
mkdir -p "/tmp/magicfs-test-mount" "/tmp/magicfs-test-data"

# Build
cargo build --quiet

# Start Daemon
export RUST_LOG=debug 
# Note: Using debug logs might help see the "Suppressing chatter" messages
sudo nohup ./target/debug/magicfs "/tmp/magicfs-test-mount" "/tmp/magicfs-test-data" > tests/magicfs.log 2>&1 &
sleep 2

# Run Test
export PYTHONPATH=$(pwd)/tests
python3 -u "$TEST_FILE" "/tmp/.magicfs/index.db" "/tmp/magicfs-test-mount" "/tmp/magicfs-test-data"
