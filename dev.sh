#!/bin/bash
set -e

# --- Config ---
MOUNT="$HOME/MagicFS"
WATCH_A="$HOME/me"
WATCH_B="$HOME/sync/vault"
# Keep your Arctic path
DB_DIR="/tmp/.magicfs_arctic"

# --- LOGGING CONFIGURATION ---
# We force this to prevent silence or "trace" floods from dependencies
export RUST_LOG="info,magicfs=debug"

echo "🔑 Authorizing sudo..."
sudo -v

echo "☢️  Cleanup Sequence Initiated..."

# 1. Kill processes
sudo pkill -x magicfs || true

# 2. Unmount Loop
if mountpoint -q "$MOUNT" 2>/dev/null || grep -qs "$MOUNT" /proc/mounts; then
    echo "    🔻 Unmounting..."
    sudo umount -l "$MOUNT"
    
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

# 3. Clean Mount Point
if [ -d "$MOUNT" ]; then
    echo "    🗑️  Removing old mount directory..."
    if ! sudo rm -rf "$MOUNT"; then
         echo "    ❌ FATAL: 'rm' failed. The mount is still stuck."
         ls -ld "$MOUNT"
         exit 1
    fi
fi

# 4. Clean DB
if [ -d "$DB_DIR" ]; then
    echo "    🗄️  Wiping old database ($DB_DIR)..."
    sudo rm -rf "$DB_DIR"
fi

# 5. Recreate Dirs
echo "    ✨ Creating directories..."
mkdir -p "$MOUNT"
mkdir -p "$WATCH_A"
mkdir -p "$WATCH_B"
mkdir -p "$DB_DIR"

echo "🔨 Building (Snowflake Arctic XS)..."
cd "$(dirname "$0")"
cargo build

echo "🚀 Launching with Multi-Root: $WATCH_A, $WATCH_B"
echo "📝 Log Level: $RUST_LOG"

# Pass the env var explicitly to sudo
sudo -E ./target/debug/magicfs "$MOUNT" "$WATCH_A,$WATCH_B"
