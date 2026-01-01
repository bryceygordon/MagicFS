#!/bin/bash
set -e # Stop on error

# --- Configuration ---
MOUNT_POINT="$HOME/MagicFS"
WATCH_DIR="$HOME/me"
DB_DIR="/tmp/.magicfs_dev"

# --- Disassemble (Cleanup) ---
echo "🧹 Cleaning up..."
pkill -f "target/debug/magicfs" || true
fusermount -u "$MOUNT_POINT" 2>/dev/null || true
mkdir -p "$MOUNT_POINT"

# --- Build ---
echo "🔨 Building..."
cd ~/magicfs
cargo build

# --- Assemble (Run) ---
echo "🚀 Starting MagicFS (Dev Mode)..."
echo "   Mount: $MOUNT_POINT"
echo "   Watch: $WATCH_DIR"
echo "   Press Ctrl+C to stop."

# Run with backtrace for debugging
RUST_BACKTRACE=1 ./target/debug/magicfs "$MOUNT_POINT" "$WATCH_DIR"
