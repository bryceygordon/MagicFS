#!/bin/bash
set -e

# --- Config ---
MOUNT="$HOME/MagicFS"
WATCH="$HOME/me"

echo "🔑 Authorizing sudo..."
sudo -v

echo "☢️  Cleanup Sequence Initiated..."

# 1. Kill processes
sudo fuser -k -m "$MOUNT" 2>/dev/null || true

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

# 3. Delete directory (Only if it exists)
if [ -d "$MOUNT" ]; then
    echo "   🗑️  Removing old directory..."
    # If this fails, it means the mount is still ghosting us
    if ! sudo rm -rf "$MOUNT"; then
         echo "   ❌ FATAL: 'rm' failed. The mount is still stuck."
         ls -ld "$MOUNT"
         exit 1
    fi
fi

# 4. Recreate
echo "   ✨ Creating fresh mountpoint..."
mkdir -p "$MOUNT"

echo "🔨 Building..."
cd "$(dirname "$0")"
cargo build

echo "🚀 Launching..."
RUST_BACKTRACE=1 ./target/debug/magicfs "$MOUNT" "$WATCH"
