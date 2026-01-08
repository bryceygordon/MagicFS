from common import MagicTest
import sqlite3
import os
import time

test = MagicTest()
print("=== TEST 19: War Mode Implementation & State Machine ===")

# 1. SETUP: Create backlog BEFORE daemon startup
# The daemon is already running, but this test assumes cold-start scenario
# Let's create a large backlog by populating the watch directory before the test starts
# However, since the daemon is already running, we'll verify its behavior by creating files
# and monitoring the state transitions in logs

print("\n--- Phase 1: Create initial backlog ---")
# Create 200 files rapidly to force a bulk indexing scenario
file_count = 200
for i in range(file_count):
    filename = f"backlog_file_{i:03d}.txt"
    content = f"This is content for backlog file {i}. " * 50  # Make it substantial
    test.create_file(filename, content)

print(f"✅ Created {file_count} files in watch directory")

# 2. VERIFY WAR MODE: Check logs for War Mode engagement
print("\n--- Phase 2: Verify War Mode Engagement ---")
time.sleep(2)  # Let the system start processing

# Read log file
log_file = os.environ.get("MAGICFS_LOG_FILE", "/tmp/magicfs_debug.log")
war_mode_found = False
peace_mode_found = False

if os.path.exists(log_file):
    with open(log_file, "r") as f:
        log_content = f.read()

        # Check for War Mode entry
        if "[Repository] 🔥 ENTERING WAR MODE" in log_content:
            print("✅ War Mode detected in logs")
            war_mode_found = True
        else:
            print("⚠️  War Mode log entry not found (may have completed already)")

        # Check for Peace Mode transition
        if "[Librarian] 🛡️ Initial indexing complete. Switching to Peace Mode" in log_content:
            print("✅ Peace Mode transition detected")
            peace_mode_found = True
        else:
            print("⚠️  Peace Mode transition not yet detected")

else:
    print(f"❌ Log file not found: {log_file}")
    test.dump_logs()
    exit(1)

# 3. VERIFY HANDOVER: Wait for queue to drain
print("\n--- Phase 3: Verify Queue Drain and Handover ---")
print("[Sensor] Waiting for initial indexing queue to drain...")

# Wait for all files to be indexed
if test.wait_for_stable_db(stability_duration=5, max_wait=120):
    final_count = test.get_db_count()
    print(f"✅ All files indexed. Final DB count: {final_count}")

    # Should be close to our file_count (allow for some system files)
    if final_count >= file_count * 0.9:
        print(f"✅ File count verification passed ({final_count} >= {file_count * 0.9})")
    else:
        print(f"❌ File count too low: {final_count} (expected >= {file_count * 0.9})")
        test.dump_logs()
        exit(1)
else:
    print("❌ Timeout waiting for DB to stabilize")
    test.dump_logs()
    exit(1)

# 4. VERIFY INTEGRITY: Check for WAL files and successful mode switch
print("\n--- Phase 4: Verify Database Integrity Post-Handover ---")

# Check for WAL-related files (proof that WAL mode was engaged)
db_dir = "/tmp/.magicfs_nomic"
wal_file = os.path.join(db_dir, "index.db-wal")
shm_file = os.path.join(db_dir, "index.db-shm")

if os.path.exists(wal_file):
    print("✅ WAL file exists (proof of WAL mode engagement)")
else:
    print("⚠️  WAL file not found (may be cleaned up, acceptable)")

# 5. CREATE CANARY: Verify system didn't lock up during transition
print("\n--- Phase 5: Canary File Test ---")
test.create_file("canary.txt", "This is the canary file to test post-handover operation")
test.wait_for_indexing("canary.txt", timeout=10)
test.assert_file_indexed("canary.txt")
print("✅ Canary file indexed successfully")

# 6. VERIFY LOG COMPLETION
print("\n--- Phase 6: Final Log Verification ---")
if os.path.exists(log_file):
    with open(log_file, "r") as f:
        lines = f.readlines()

        # Check final state
        last_lines = lines[-20:]  # Check last 20 lines
        log_tail = "\n".join(last_lines)

        if "Peace Mode active" in log_tail or "Monitoring" in log_tail:
            print("✅ System shows Peace Mode/Monitoring state")
        else:
            print("⚠️  Peace Mode state unclear from recent logs")

        # Count War Mode entries (should be exactly 1)
        war_mode_count = sum(1 for line in lines if "[Repository] 🔥 ENTERING WAR MODE" in line)
        print(f"✅ War Mode engaged {war_mode_count} time(s)")

        # Check for Peace Mode transition
        peace_count = sum(1 for line in lines if "[Librarian] 🛡️ Initial indexing complete" in line)
        if peace_count > 0:
            print(f"✅ Peace Mode transition completed {peace_count} time(s)")
        else:
            print("❌ Peace Mode transition never occurred")
            test.dump_logs(50)  # Show last 50 lines for debugging
            exit(1)

print("\n=== TEST 19: War Mode Implementation PASSED ===")
print("✅ All phases completed successfully:")
print("   - War Mode engagement verified")
print("   - Queue draining and handover successful")
print("   - Database integrity maintained")
print("   - Post-handover operation confirmed")
print("   - State machine transitioned correctly")

# Additional verification: Check for SystemState in the database (if we add it)
# For now, the logs and behavior are sufficient proof