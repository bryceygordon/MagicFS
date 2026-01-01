#!/bin/bash
set -e

# --- Config ---
MOUNT="$HOME/MagicFS"
WATCH_A="$HOME/me"
WATCH_B="$HOME/sync/vault"
DB_DIR="/tmp/.magicfs"

echo "🔑 Authorizing sudo..."
sudo -v

echo "☢️  Cleanup Sequence Initiated..."

# 1. Kill processes
sudo pkill -x magicfs || true

# 2. Unmount Loop (Wait for it to actually detach)
if mountpoint -q "$MOUNT" 2>/dev/null || grep -qs "$MOUNT" /proc/mounts; then
    echo "   🔻 Unmounting..."
    sudo umount -l "$MOUNT"
    
    # Wait until it is NO LONGER a mountpoint
    MAX_RETRIES=10
    COUNT=0
    while mountpoint -q "$MOUNT" 2>/dev/null; do
        sleep 0.2
        ((COUNT++))
        if [ $COUNT -ge $MAX_RETRIES ]; then
            echo "   ❌ Timeout waiting for unmount."
            exit 1
        fi
    done
fi

# 3. Delete mount directory
if [ -d "$MOUNT" ]; then
    echo "   🗑️  Removing old mount directory..."
    if ! sudo rm -rf "$MOUNT"; then
         echo "   ❌ FATAL: 'rm' failed. The mount is still stuck."
         ls -ld "$MOUNT"
         exit 1
    fi
fi

# 4. Delete Database (Fixes Permission Error from Tests)
if [ -d "$DB_DIR" ]; then
    echo "   🗄️  Wiping old database..."
    sudo rm -rf "$DB_DIR"
fi

# 5. Recreate Dirs
echo "   ✨ Creating directories..."
mkdir -p "$MOUNT"
# We assume WATCH_A and WATCH_B exist since they are your real data.
# mkdir -p is safe to run on existing dirs (it does nothing).
mkdir -p "$WATCH_A"
mkdir -p "$WATCH_B"

echo "🔨 Building..."
cd "$(dirname "$0")"
cargo build

echo "🚀 Launching with Multi-Root: $WATCH_A, $WATCH_B"
# Pass both paths separated by a comma
RUST_LOG=info ./target/debug/magicfs "$MOUNT" "$WATCH_A,$WATCH_B"
