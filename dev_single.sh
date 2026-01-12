#!/bin/bash
# FILE: dev_single.sh
set -e

# --- Config ---
MOUNT="$HOME/MagicFS"
WATCH_DIR="$HOME/me"
# --- UPDATED: NOMIC ISOLATION PATH ---
DB_DIR="/tmp/.magicfs_nomic"

# Check for arguments
KEEP_DB=false
for arg in "$@"; do
    if [ "$arg" == "--keep-db" ]; then
        KEEP_DB=true
        echo "💾 Database persistence enabled."
    fi
done

echo "🔑 Authorizing sudo..."
sudo -v

echo "☢️  Cleanup Sequence Initiated..."

# 1. Kill processes
sudo pkill -x magicfs || true

# 2. Unmount Loop (ROBUST VERIFICATION)
if grep -qs "$MOUNT" /proc/mounts; then
    echo "    🔻 Unmounting..."
    sudo umount -l "$MOUNT"
    
    MAX_RETRIES=20
    COUNT=0
    while grep -qs "$MOUNT" /proc/mounts; do
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
    sudo rm -rf "$MOUNT"
fi

# 4. Delete Database (Conditional)
if [ "$KEEP_DB" = false ]; then
    if [ -d "$DB_DIR" ]; then
        echo "    🗄️  Wiping old database ($DB_DIR)..."
        sudo rm -rf "$DB_DIR"
    fi
else
    echo "    ⏩ Skipping database wipe (--keep-db)."
fi

# 5. Recreate Dirs
echo "    ✨ Creating directories..."
mkdir -p "$MOUNT"
mkdir -p "$WATCH_DIR"
mkdir -p "$DB_DIR"

echo "🔨 Building (Nomic Embed v1.5)..."
cd "$(dirname "$0")"
cargo build

echo "🚀 Launching with Single Root: $WATCH_DIR"

# Get current user ID/GID before elevating
CURRENT_UID=$(id -u)
CURRENT_GID=$(id -g)

# Launch with explicit identity variables
exec sudo SUDO_UID=$CURRENT_UID SUDO_GID=$CURRENT_GID RUST_LOG=debug ./target/debug/magicfs "$MOUNT" "$WATCH_DIR"
