# FILE: tests/cases/test_23_retagging.py
from common import MagicTest
import os
import subprocess
import sys
import sqlite3
import time

test = MagicTest()
print("--- TEST 23: Semantic Retagging (Move) ---")

# 1. Setup: Create physical file
filename = "receipt.pdf"
test.create_file(filename, "Amount: $100")
test.wait_for_indexing(filename)

# 2. Get File ID
conn = sqlite3.connect(test.db_path)
cursor = conn.cursor()
cursor.execute("SELECT file_id FROM file_registry WHERE abs_path LIKE ?", (f"%{filename}",))
file_id = cursor.fetchone()[0]
conn.close()

# 3. Inject Tags and Initial Link (File -> Inbox)
print("[Setup] Injecting tags 'inbox' and 'finance'...")
setup_sql = f"""
INSERT INTO tags (name, color) VALUES ('inbox', 'blue');
INSERT INTO tags (name, color) VALUES ('finance', 'green');
INSERT INTO file_tags (file_id, tag_id, display_name) 
VALUES ({file_id}, (SELECT tag_id FROM tags WHERE name='inbox'), '{filename}');
"""
subprocess.run(["sudo", "sqlite3", test.db_path, setup_sql], check=True)

# 4. Verify Initial State
inbox_path = os.path.join(test.mount_point, ".magic", "tags", "inbox", filename)
finance_path = os.path.join(test.mount_point, ".magic", "tags", "finance", filename)

if not os.path.exists(inbox_path):
    print("❌ FAILURE: File not found in Inbox before move.")
    sys.exit(1)

# 5. Perform the Move
print(f"[Action] Moving file from Inbox to Finance...")
try:
    # This triggers FUSE rename()
    os.rename(inbox_path, finance_path)
except OSError as e:
    print(f"❌ FAILURE: OS Rename failed: {e}")
    sys.exit(1)

# 6. Verify Physical Illusion (File should appear in Finance immediately)
if not os.path.exists(finance_path):
    print("❌ FAILURE: File missing from Finance after move.")
    sys.exit(1)

if os.path.exists(inbox_path):
    print("❌ FAILURE: File still exists in Inbox after move.")
    sys.exit(1)

# 7. Verify Database State
print("[Assert] Checking DB consistency...")
conn = sqlite3.connect(test.db_path)
cursor = conn.cursor()

# Check that the 'inbox' link is gone or updated
cursor.execute("""
    SELECT t.name 
    FROM file_tags ft 
    JOIN tags t ON ft.tag_id = t.tag_id 
    WHERE ft.file_id = ?
""", (file_id,))
tags = [r[0] for r in cursor.fetchall()]
conn.close()

print(f"  Current Tags: {tags}")

if "finance" in tags and "inbox" not in tags:
    print("✅ SUCCESS: Database reflects the move.")
else:
    print(f"❌ FAILURE: Database state incorrect. Expected ['finance'], got {tags}")
    sys.exit(1)
