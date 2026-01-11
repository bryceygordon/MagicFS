#!/usr/bin/env python3
import os
import sys

mount_point = "/tmp/magicfs-test-mount"
system_data_dir = "/tmp/magicfs-test-system"

# Quick test for zero.txt only
try:
    # Create zero-byte file
    zero_file = os.path.join(mount_point, "inbox", "zero.txt")
    print(f"Attempting to create: {zero_file}")

    with open(zero_file, "w") as f:
        pass

    print("SUCCESS: zero.txt created")
except Exception as e:
    print(f"FAILURE: {e}")
    sys.exit(1)