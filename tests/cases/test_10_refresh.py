from common import MagicTest
import os
import time
import sqlite3
import subprocess
import sys

test = MagicTest()
print("--- TEST 10: Manual Refresh (The Kick Button) ---")

# 1. Setup: Create a standard file
filename = "ghost.txt"
content = "I am a ghost in the machine."
test.create_file(filename, content)
test.wait_for_indexing(filename)
print(f"✅ Initial indexing complete for {filename}")

# 2. Sabotage: Corrupt the record in DB (Set size/mtime to 0)
# FIX: Run as ROOT because the DB is owned by the daemon (root).
print("[Sabotage] Manually corrupting record in DB (via sudo)...")

sabotage_script = f"""
import sqlite3
conn = sqlite3.connect('{test.db_path}')
cursor = conn.cursor()
cursor.execute("UPDATE file_registry SET size = 0, mtime = 0 WHERE abs_path LIKE '%{filename}'")
conn.commit()
conn.close()
"""

# Run the sabotage snippet as root
result = subprocess.run(
    ["sudo", sys.executable, "-c", sabotage_script],
    capture_output=True,
    text=True
)

if result.returncode != 0:
    print(f"❌ SETUP FAILURE: Sudo sabotage failed.\n{result.stderr}")
    exit(1)

# Verify Sabotage (Read-only is fine here)
def get_file_size_in_db():
    try:
        conn = sqlite3.connect(test.db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT size FROM file_registry WHERE abs_path LIKE ?", (f"%{filename}",))
        row = cursor.fetchone()
        conn.close()
        return row[0] if row else -1
    except:
        return -1

if get_file_size_in_db() == 0:
    print(f"✅ Sabotage successful: {filename} size set to 0 in DB.")
else:
    print("❌ SETUP FAILURE: DB record not corrupted (Size is not 0).")
    exit(1)

# 3. The Trigger: Touch .magic/refresh
refresh_button = os.path.join(test.mount_point, ".magic", "refresh")
print(f"[Action] Touching {refresh_button}...")

try:
    if os.path.exists(refresh_button):
        # We need to explicitly update time to trigger the FUSE setattr
        os.utime(refresh_button, None)
    else:
        print(f"❌ FAILURE: Refresh button not found at {refresh_button}")
        exit(1)
except OSError as e:
    print(f"❌ FAILURE: Could not touch refresh button: {e}")
    exit(1)

# 4. Wait for Recovery
print("[Wait] Waiting for manual scan to repair the record...")
start = time.time()
repaired = False
while time.time() - start < 15:
    sz = get_file_size_in_db()
    if sz > 0:
        print(f"✅ SUCCESS: Manual refresh repaired the file! (Size: {sz})")
        repaired = True
        break
    time.sleep(0.5)

if not repaired:
    print("❌ FAILURE: Manual refresh did not repair the file.")
    exit(1)
