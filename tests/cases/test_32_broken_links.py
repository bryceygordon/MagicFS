#!/usr/bin/env python3
"""
Test 32: Broken Link Detection
Phase 15 Final Task - Verifies orphaned record cleanup and real-time deletion detection

This test follows the existing MagicTest pattern and uses the compiled Rust daemon.
"""

from common import MagicTest
import os
import time
import shutil

# Test configuration - these will be passed by run_single.sh
# DB_PATH = "/tmp/.magicfs_nomic/index.db"
# MOUNT_POINT = "/tmp/magicfs-test-mount"
# WATCH_DIR = "/tmp/magicfs-test-data"

test = MagicTest()

def cleanup_directories():
    """Clean up any test files we create"""
    test_files = [
        os.path.join(test.watch_dir, "offline_test.txt"),
        os.path.join(test.watch_dir, "realtime_test.txt")
    ]
    for f in test_files:
        if os.path.exists(f):
            os.remove(f)

print("--- TEST 32: Broken Link Detection ---")

# =================================================================================================
# SCENARIO A: OFFLINE DELETION (Librarian::purge_orphaned_records)
# =================================================================================================
print("\n=== SCENARIO A: Offline Deletion Detection ===")

# 1. Create test file
offline_file = "offline_test.txt"
test.create_file(offline_file, "This file will be deleted while daemon is offline")

# 2. Wait for indexing
test.wait_for_indexing(offline_file)

# 3. Verify it's indexed
initial_count = test.get_db_count()
test.assert_file_indexed(offline_file)
print(f"✅ File indexed (DB count: {initial_count})")

# 4. Stop daemon (simulate offline scenario)
print("🛑 Stopping daemon (simulating offline)...")
os.system("sudo pkill -9 -x magicfs")
time.sleep(2)

# 5. Manually delete the physical file (offline deletion)
physical_path = os.path.join(test.watch_dir, offline_file)
if os.path.exists(physical_path):
    os.remove(physical_path)
    print(f"🗑️  Physically deleted: {physical_path}")
    assert not os.path.exists(physical_path), "File still exists after os.remove!"

# 6. Restart daemon
print("🚀 Restarting daemon...")
os.system("sudo pkill -9 -x magicfs 2>/dev/null")
time.sleep(1)
os.system("cd /home/bryceg/magicfs && cargo build --quiet")
os.system(f"cd /home/bryceg/magicfs && sudo nohup ./target/debug/magicfs {test.mount_point} {test.watch_dir} > tests/magicfs.log 2>&1 &")
time.sleep(3)

# 7. Wait for daemon ready and indexing to complete
test.wait_for_stable_db(stability_duration=3, max_wait=30)

# 8. Verify purge_orphaned_records cleaned up the orphan
final_count = test.get_db_count()
if test.check_file_in_db(offline_file):
    print(f"❌ FAILURE: Orphan record NOT cleaned up (count: {final_count})")
    exit(1)

print(f"✅ Scenario A PASSED: purge_orphaned_records detected offline deletion (before: {initial_count}, after: {final_count})")

# =================================================================================================
# SCENARIO B: REAL-TIME DELETION (inotify + handle_file_event -> Indexer::remove_file)
# =================================================================================================
print("\n=== SCENARIO B: Real-time Deletion Detection ===")

# 1. Create new test file
realtime_file = "realtime_test.txt"
test.create_file(realtime_file, "This file will be deleted in real-time")

# 2. Wait for indexing
test.wait_for_indexing(realtime_file)

# 3. Verify it's indexed
initial_count = test.get_db_count()
test.assert_file_indexed(realtime_file)
print(f"✅ File indexed (DB count: {initial_count})")

# 4. Delete file via backdoor (simulating external deletion)
physical_path = os.path.join(test.watch_dir, realtime_file)
if os.path.exists(physical_path):
    os.remove(physical_path)
    print(f"🗑️  Physically deleted (backdoor): {physical_path}")
    assert not os.path.exists(physical_path), "File still exists after os.remove!"

# 5. Wait for inotify to catch it and Oracle to process
print("👀 Waiting for real-time detection...")
timeout = 20
start = time.time()

while time.time() - start < timeout:
    if not test.check_file_in_db(realtime_file):
        print(f"✅ Real-time deletion detected after {time.time() - start:.1f}s")
        break
    time.sleep(0.5)
else:
    if test.check_file_in_db(realtime_file):
        print(f"❌ FAILURE: Real-time detection failed")
        exit(1)

# 6. Final verification
final_count = test.get_db_count()
if test.check_file_in_db(realtime_file):
    print(f"❌ FAILURE: Real-time cleanup incomplete (count: {final_count})")
    exit(1)

print(f"✅ Scenario B PASSED: Real-time deletion detected (before: {initial_count}, after: {final_count})")

# =================================================================================================
# CLEANUP
cleanup_directories()
print("\n🎉 ALL BROKEN LINK DETECTION TESTS PASSED!")
print("✅ Offline detection: purge_orphaned_records() works")
print("✅ Real-time detection: handle_file_event(Remove) -> Indexer::remove_file() works")
print("\n📊 Phase 15 Broken Link Detection: COMPLETE")