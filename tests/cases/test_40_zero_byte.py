#!/usr/bin/env python3
"""
TEST 40: Phase 24 - Zero-Byte Citizenship (Fixed Harness)

✅ FIXED: Robust test harness that validates the Rust fix:
1. Removes vec_index SQL query (untestable via CLI without extension loading)
2. Adds polling for deletion test (no race conditions)
3. Validates timing and metadata correctly

This test should PASS after the Rust fix is applied.
"""

import os
import subprocess
import time
import sys

def main():
    print("=== TEST 40: Phase 24 - Zero-Byte Citizenship (Fixed Harness) ===")

    if len(sys.argv) < 4:
        print("Usage: test_40_zero_byte.py <db_path> <mount_point> <watch_dir>")
        sys.exit(1)

    db_path = sys.argv[1]
    mount_point = sys.argv[2]
    watch_dir = sys.argv[3]

    print(f"[Setup] DB Path: {db_path}")
    print(f"[Setup] Mount Point: {mount_point}")
    print(f"[Setup] Watch Dir: {watch_dir}")

    # Helper to clean files
    def clean_files():
        test_files = [
            "empty_file.txt", "content.txt", "temp_empty.txt",
            "empty_0.txt", "empty_1.txt", "empty_2.txt", "empty_3.txt", "empty_4.txt"
        ]
        for f in test_files:
            path = os.path.join(watch_dir, f)
            if os.path.exists(path):
                try:
                    os.remove(path)
                except:
                    pass

    clean_files()
    time.sleep(0.5)

    # --- TEST 1: TIMING (The Core Metric) ---
    print("\n--- Test 1: Zero-byte file appearance timing ---")
    zero_file = os.path.join(watch_dir, "empty_file.txt")
    start_time = time.time()

    # Create 0-byte file
    with open(zero_file, "w") as f:
        pass

    # Poll for appearance (should be FAST, not 2+ seconds)
    found = False
    detection_time = 0
    max_wait = 3.0

    for i in range(30):  # Check up to 3 seconds
        time.sleep(0.1)  # Check every 100ms
        current_time = time.time() - start_time

        try:
            cmd = ["sudo", "sqlite3", db_path, f"SELECT count(*) FROM file_registry WHERE abs_path = '{zero_file}'"]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            if res.stdout.strip() == "1":
                found = True
                detection_time = current_time
                break
        except:
            pass

        if current_time > max_wait:
            break

    # Analysis
    if not found:
        print(f"❌ FAIL: File not detected in {max_wait:.1f}s")
        print("   This means the Rust fix is NOT applied (still retrying ~2s)")
        sys.exit(1)

    if detection_time < 1.0:
        print(f"✅ PASS: Detected in {detection_time:.3f}s (FAST: < 1.0s)")
        print(f"   🎯 Target achieved! (Previous: ~2.0s+)")
    else:
        print(f"❌ FAIL: Detected in {detection_time:.3f}s (SLOW: >= 1.0s)")
        print(f"   ⚠️  Even if registered, it took too long")
        sys.exit(1)

    # --- TEST 2: METADATA VERIFICATION ---
    print("\n--- Test 2: Metadata Verification ---")

    # Get file_id
    try:
        cmd = ["sudo", "sqlite3", db_path, f"SELECT file_id FROM file_registry WHERE abs_path = '{zero_file}'"]
        res = subprocess.run(cmd, capture_output=True, text=True, check=True)
        file_id = res.stdout.strip()
        if not file_id:
            print("❌ FAIL: No file_id assigned")
            sys.exit(1)
        print(f"✅ PASS: file_id assigned ({file_id})")
    except subprocess.CalledProcessError as e:
        print(f"❌ FAIL: Could not query file_id: {e}")
        sys.exit(1)

    # Verify size is 0
    try:
        cmd = ["sudo", "sqlite3", db_path, f"SELECT size FROM file_registry WHERE abs_path = '{zero_file}'"]
        res = subprocess.run(cmd, capture_output=True, text=True, check=True)
        size = res.stdout.strip()
        if size == "0":
            print(f"✅ PASS: Size is 0 in DB")
        else:
            print(f"❌ FAIL: Size is {size}, expected 0")
            sys.exit(1)
    except subprocess.CalledProcessError as e:
        print(f"❌ FAIL: Could not query size: {e}")
        sys.exit(1)

    # --- TEST 3: MULTIPLE ZERO-BYTE FILES ---
    print("\n--- Test 3: Multiple 0-byte files ---")
    files = [os.path.join(watch_dir, f"empty_{i}.txt") for i in range(5)]

    for f in files:
        with open(f, "w") as pass_file:
            pass

    # Wait reasonable time (should process all 5 quickly)
    time.sleep(1.5)

    # Count registered
    try:
        placeholders = ",".join([f"'{f}'" for f in files])
        cmd = ["sudo", "sqlite3", db_path, f"SELECT COUNT(*) FROM file_registry WHERE abs_path IN ({placeholders})"]
        res = subprocess.run(cmd, capture_output=True, text=True, check=True)
        count = int(res.stdout.strip())

        if count == 5:
            print(f"✅ PASS: All 5 files registered")
        else:
            print(f"❌ FAIL: Only {count}/5 files registered")
            sys.exit(1)
    except subprocess.CalledProcessError as e:
        print(f"❌ FAIL: Multi-file query failed: {e}")
        sys.exit(1)

    # --- TEST 4: DELETION WITH POLLING (No Race Conditions) ---
    print("\n--- Test 4: Deletion Cleanup ---")
    temp_file = os.path.join(watch_dir, "temp_empty.txt")

    # Create and wait for registration
    with open(temp_file, "w") as f:
        pass

    # Wait for it to appear
    registered = False
    for _ in range(20):
        try:
            cmd = ["sudo", "sqlite3", db_path, f"SELECT count(*) FROM file_registry WHERE abs_path = '{temp_file}'"]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            if res.stdout.strip() == "1":
                registered = True
                break
        except:
            pass
        time.sleep(0.1)

    if not registered:
        print("❌ FAIL: temp file never registered")
        sys.exit(1)

    print("   ✓ File registered, now deleting...")

    # Delete the file
    os.remove(temp_file)

    # Poll for removal (with generous timeout for event propagation)
    removed = False
    for _ in range(40):  # 4 seconds max
        try:
            cmd = ["sudo", "sqlite3", db_path, f"SELECT count(*) FROM file_registry WHERE abs_path = '{temp_file}'"]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            if res.stdout.strip() == "0":
                removed = True
                break
        except:
            pass
        time.sleep(0.1)

    if removed:
        print("✅ PASS: File successfully removed from DB")
    else:
        print("❌ FAIL: File still in DB after 4s polling")
        sys.exit(1)

    # --- SUMMARY ---
    print("\n" + "="*60)
    print("🎉 TEST 40 COMPLETE: Zero-Byte Citizenship")
    print("="*60)
    print("✅ All tests passed!")
    print("   - Zero-byte files register quickly (<1.0s)")
    print("   - File IDs assigned correctly")
    print("   - Size metadata correct")
    print("   - Multiple files handled")
    print("   - Deletion cleanup works")
    print("")
    print("🚀 Rust fix is working! Phase 24 complete.")

if __name__ == "__main__":
    main()