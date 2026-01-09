#!/usr/bin/env python3
"""
Test 36: Peace Mode Exit Error Detection
Detects the regression in Phase 19 where Peace Mode handover fails
due to wal_checkpoint PRAGMA returning data to execute().

This test will be used to verify the fix is working correctly.
"""

import sys
import os

def main():
    # Setup using standard MagicTest pattern
    if len(sys.argv) < 4:
        print("Usage: python test.py <db_path> <mount_point> <watch_dir>")
        sys.exit(1)

    db_path = sys.argv[1]
    mount_point = sys.argv[2]
    watch_dir = sys.argv[3]

    print("=== TEST 36: Peace Mode Exit Error Detection ===")

    log_file = os.environ.get("MAGICFS_LOG_FILE", "/tmp/magicfs_debug.log")

    print(f"[1] Checking for Peace Mode exit errors in log: {log_file}")

    if not os.path.exists(log_file):
        print(f"❌ Log file not found: {log_file}")
        print("   This test requires the daemon to be running to capture logs")
        return 1

    # Read the log file
    try:
        with open(log_file, "r") as f:
            log_content = f.read()
    except Exception as e:
        print(f"❌ Failed to read log file: {e}")
        return 1

    # Check for the Peace Mode exit error
    exit_error_pattern = "Failed to exit War Mode"
    exit_error_found = exit_error_pattern in log_content

    if exit_error_found:
        print("❌ PEACE MODE EXIT ERROR DETECTED!")

        # Find the specific error lines
        lines = log_content.split('\n')
        error_lines = [line for line in lines if exit_error_pattern in line]

        print(f"   Found {len(error_lines)} error line(s):")
        for line in error_lines[:3]:  # Show first 3
            print(f"   >>> {line}")

        print("\n🔧 ROOT CAUSE:")
        print("   Line 423 in src/storage/repository.rs:")
        print("   self.conn.execute('PRAGMA wal_checkpoint(TRUNCATE)', [])?;")
        print("   ")
        print("   Problem: wal_checkpoint returns data (busy, log, checkpointed).")
        print("   execute() expects 0 rows -> ERROR!")
        print("   ")
        print("   FIX:")
        print("   self.conn.query_row(\"PRAGMA wal_checkpoint(TRUNCATE)\", [], |_| Ok(()))?;")

        return 1

    print("✅ No Peace Mode exit errors detected")

    # Also verify War Mode entry is working (the previous fix)
    war_entry_success = "[Repository] 🔥 ENTERING WAR MODE" in log_content
    war_entry_failure = "Failed to enter War Mode" in log_content

    if war_entry_success and not war_entry_failure:
        print("✅ War Mode entry is working correctly")
    elif war_entry_failure:
        print("⚠️  War Mode entry still has issues")
        return 1
    else:
        print("⚠️  No War Mode activity detected (daemon may be in steady state)")

    return 0

if __name__ == "__main__":
    sys.exit(main())