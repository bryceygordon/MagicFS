#!/bin/bash
# FILE: dev.sh
set -e

# --- Config ---
MOUNT="$HOME/MagicFS"
WATCH_A="$HOME/me"
WATCH_B="$HOME/sync/vault"
# --- UPDATED: SNOWFLAKE M ISOLATION PATH ---
DB_DIR="/tmp/.magicfs_snowflake_m"

echo "🔑 Authorizing sudo..."
sudo -v

echo "☢️  Cleanup Sequence Initiated..."

# 1. Kill processes
sudo pkill -x magicfs || true

# 2. Unmount Loop (Wait for it to actually detach)
if mountpoint -q "$MOUNT" 2>/dev/null || grep -qs "$MOUNT" /proc/mounts; then
    echo "    🔻 Unmounting..."
    sudo umount -l "$MOUNT"
    
    # Wait until it is NO LONGER a mountpoint
    MAX_RETRIES=10
    COUNT=0
    while mountpoint -q "$MOUNT" 2>/dev/null; do
        sleep 0.2
        ((COUNT++))
        if [ $COUNT -ge $MAX_RETRIES ]; then
            echo "    ❌ Timeout waiting for unmount."
            exit 1
        fi
    done
fi

# 3. Delete mount directory
if [ -d "$MOUNT" ]; then
    echo "    🗑️  Removing old mount directory..."
    if ! sudo rm -rf "$MOUNT"; then
          echo "    ❌ FATAL: 'rm' failed. The mount is still stuck."
          ls -ld "$MOUNT"
          exit 1
    fi
fi

# 4. Delete Database (Fixes Permission Error from Tests)
if [ -d "$DB_DIR" ]; then
    echo "    🗄️  Wiping old database ($DB_DIR)..."
    sudo rm -rf "$DB_DIR"
fi

# 5. Recreate Dirs
echo "    ✨ Creating directories..."
# We create the mount point as the normal user so the folder belongs to you 
# before the mount overlays it.
mkdir -p "$MOUNT"
mkdir -p "$WATCH_A"
mkdir -p "$WATCH_B"
mkdir -p "$DB_DIR"

echo "🔨 Building (Snowflake Arctic Medium)..."
cd "$(dirname "$0")"
cargo build

echo "🚀 Launching with Multi-Root: $WATCH_A, $WATCH_B"

# FIX: Use sudo -E to preserve RUST_LOG and run as root.
# This ensures 'AllowOther' works correctly, allowing you to browse
# the filesystem without permission prompts.
sudo -E ./target/debug/magicfs "$MOUNT" "$WATCH_A,$WATCH_B"
