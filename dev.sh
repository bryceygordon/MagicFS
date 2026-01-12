#!/bin/bash
# FILE: dev.sh
set -e

# --- Config ---
MOUNT="$HOME/MagicFS"
WATCH_A="$HOME/me"
WATCH_B="$HOME/sync/vault"
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
# We check /proc/mounts because 'mountpoint' can lie during lazy unmounts
if grep -qs "$MOUNT" /proc/mounts; then
    echo "    🔻 Unmounting..."
    sudo umount -l "$MOUNT"
    
    MAX_RETRIES=20
    COUNT=0
    # Wait until it is GONE from the kernel table
    while grep -qs "$MOUNT" /proc/mounts; do
        sleep 0.2
        ((COUNT++))
        if [ $COUNT -ge $MAX_RETRIES ]; then
            echo "    ❌ CRITICAL: Kernel refuses to release mount. Trying forced kill..."
            sudo fuser -km "$MOUNT" 2>/dev/null || true
            sudo umount -f "$MOUNT" 2>/dev/null || true
            break
        fi
    done
fi

# Double check to prevent stacking
if grep -qs "$MOUNT" /proc/mounts; then
    echo "❌ FATAL: Mount point is still busy. Please reboot or check 'lsof $MOUNT'."
    exit 1
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
mkdir -p "$WATCH_A"
mkdir -p "$WATCH_B"
mkdir -p "$DB_DIR"

echo "🔨 Building (Nomic Embed v1.5)..."
cd "$(dirname "$0")"
cargo build

echo "🚀 Launching with Multi-Root: $WATCH_A, $WATCH_B"

# Get current user ID/GID before elevating
CURRENT_UID=$(id -u)
CURRENT_GID=$(id -g)

echo "🏹 Robin Hood Mode Configuration:"
echo "   Daemon User : Root (via sudo)"
echo "   Target User : UID $CURRENT_UID / GID $CURRENT_GID"

# Launch with explicit identity variables
# We use 'exec' to replace the shell process, ensuring signals propagate correctly
exec sudo SUDO_UID=$CURRENT_UID SUDO_GID=$CURRENT_GID RUST_LOG=debug ./target/debug/magicfs "$MOUNT" "$WATCH_A,$WATCH_B"
