from common import MagicTest
import os
import time
import sqlite3
import subprocess

test = MagicTest()
print("--- TEST 10: Manual Refresh (The Kick Button) ---")

# 1. Create a "Ghost" file
test.create_file("ghost.txt", "I am visible")
test.wait_for_indexing("ghost.txt")
print("✅ Initial indexing complete for ghost.txt")

# 2. Sabotage the Database
print("[Sabotage] Manually corrupting record in DB (via sudo)...")
sabotage_sql = "UPDATE file_registry SET size = 0 WHERE abs_path LIKE '%ghost.txt';"
cmd = ["sudo", "sqlite3", test.db_path, sabotage_sql]
subprocess.run(cmd, check=True)
print("✅ Sabotage successful: ghost.txt size set to 0 in DB.")

# 3. Trigger Manual Refresh (The Kick)
kick_dir = os.path.join(test.watch_dir, ".magic")
kick_file = os.path.join(kick_dir, "refresh")

# FIX: Create directory first and wait for inotify to catch up
if not os.path.exists(kick_dir):
    print(f"[Setup] Creating trigger directory: {kick_dir}")
    os.makedirs(kick_dir, exist_ok=True)
    # Critical wait for recursive watcher to attach to new folder
    time.sleep(1.0) 

print(f"[Action] Touching {kick_file}...")
# Update timestamp to trigger 'modify' event
with open(kick_file, "w") as f:
    f.write(str(time.time()))

# 4. Wait for Repair
print("[Wait] Waiting for manual scan to repair the record...")

repaired = False
for i in range(20):
    try:
        conn = sqlite3.connect(test.db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT size FROM file_registry WHERE abs_path LIKE '%ghost.txt'")
        row = cursor.fetchone()
        conn.close()
        
        # If size > 0, the scan ran and fixed it
        if row and row[0] > 0:
            repaired = True
            print(f"✅ Record repaired! Size restored to {row[0]}")
            break
    except Exception as e:
        print(f"  DB Read Error: {e}")
    
    time.sleep(0.5)

if not repaired:
    print("❌ FAILURE: Manual refresh did not repair the file.")
    test.dump_logs()
    exit(1)

print("✅ TEST 10 PASSED")
