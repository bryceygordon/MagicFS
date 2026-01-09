#!/usr/bin/env python3
"""
Test 35: War Mode Pragma Error Detection
This test detects the specific War Mode failure mentioned in the requirements:
[Librarian] Failed to enter War Mode: Database error: Execute returned results

The issue: PRAGMA statements return data (the new mode), but rusqlite::execute()
expects no results and fails when data is returned.

This test will:
1. Check if the daemon is running (via MagicTest infrastructure)
2. Read the log file to detect the War Mode failure
3. Fail if the error is present (to prove the fix is needed)
4. Pass if no War Mode errors exist (after fix)
"""

import os
import sys
import time

# Add the test directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from common import MagicTest

#!/usr/bin/env python3
"""
Test 35: War Mode Pragma Error Detection
Detects the War Mode failure: "Execute returned results" from PRAGMA statements
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

    print("=== TEST 35: War Mode Pragma Error Detection ===")

    log_file = os.environ.get("MAGICFS_LOG_FILE", "/tmp/magicfs_debug.log")

    print(f"[1] Checking for War Mode errors in log: {log_file}")

    if not os.path.exists(log_file):
        print(f"❌ Log file not found: {log_file}")
        print("   This test requires the daemon to be running to capture logs")
        return False

    # Wait a moment for any startup logs to appear
    import time
    time.sleep(2)

    # Read the log file
    try:
        with open(log_file, "r") as f:
            log_content = f.read()
    except Exception as e:
        print(f"❌ Failed to read log file: {e}")
        return 1

    # Check for the specific error pattern
    error_patterns = [
        "Failed to enter War Mode",
        "Database error: Execute returned results",
        "[Repository] Failed to enter War Mode",
        "Execute returned results"
    ]

    found_errors = []
    for pattern in error_patterns:
        if pattern in log_content:
            found_errors.append(pattern)

    if found_errors:
        print("❌ WAR MODE ERROR DETECTED!")
        print(f"   Found {len(found_errors)} error pattern(s):")
        for error in found_errors:
            print(f"   - '{error}'")

        print("\n   Context:")
        lines = log_content.split('\n')
        context_lines = [line for line in lines if any(err in line for err in found_errors)]
        for line in context_lines[:5]:  # Show first 5 matches
            print(f"   >>> {line}")

        print("\n🔧 DIAGNOSIS:")
        print("   The issue is in src/storage/repository.rs::set_performance_mode():")
        print("   - self.conn.execute('PRAGMA synchronous = OFF', [])")
        print("   - self.conn.execute('PRAGMA journal_mode = MEMORY', [])")
        print("   ")
        print("   PRAGMA statements RETURN the new mode string (e.g., 'memory').")
        print("   execute() expects 0 rows returned and fails with 'Execute returned results'!")
        print("   ")
        print("   FIX: Use pragma_update() instead:")
        print("   - self.conn.pragma_update(None, \"synchronous\", \"OFF\")?;")
        print("   - self.conn.pragma_update(None, \"journal_mode\", \"MEMORY\")?;")
        print("   ")
        print("   Apply this fix to ALL 4 PRAGMA lines in set_performance_mode().")

        return 1

    print("✅ No War Mode errors detected")
    return 0

if __name__ == "__main__":
    sys.exit(main())