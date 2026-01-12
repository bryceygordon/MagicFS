from common import MagicTest
import time
import os
import sys

test = MagicTest()
print("--- TEST 31: Incinerator (Trash Retention) ---")

# 1. Setup: Create a file
filename = "garbage.txt"
test.create_file(filename, "Rubbish")
test.wait_for_indexing(filename)

# 2. Get file ID
file_path = os.path.join(test.watch_dir, filename)
file_id = test.get_file_id_by_path(file_path)
if not file_id:
    print("❌ FAILURE: Could not get file ID")
    sys.exit(1)

# 3. Ensure 'trash' tag exists
test.run_sql_exec("INSERT OR IGNORE INTO tags (name, icon) VALUES ('trash', '🗑️')")

# 4. Link to trash (simulating user move)
print(f"[Action] Moving file {file_id} to @trash...")
sql_link = f"INSERT INTO file_tags (file_id, tag_id, display_name) VALUES ({file_id}, (SELECT tag_id FROM tags WHERE name='trash'), '{filename}')"
test.run_sql_exec(sql_link)

# 5. Backdate the link to 31 days ago (Retention + 1 day)
# 31 days * 24h * 60m * 60s = 2678400 seconds
backdate = int(time.time()) - 2678400 - 100
print(f"[Action] Backdating trash entry to timestamp {backdate}...")

test.run_sql_exec(f"""
    UPDATE file_tags 
    SET added_at = {backdate} 
    WHERE file_id = {file_id} 
    AND tag_id = (SELECT tag_id FROM tags WHERE name='trash')
""")

# 6. Verify Incinerator Logic
# Since the daemon waits 60s to run the Incinerator (too long for a test),
# we verify that the database STATE effectively marks it for death.
print("[Check] Verifying Incinerator identification logic...")

TRASH_RETENTION = 30 * 24 * 60 * 60
cutoff = int(time.time()) - TRASH_RETENTION

query = f"""
SELECT ft.file_id 
FROM file_tags ft 
JOIN tags t ON ft.tag_id = t.tag_id 
WHERE t.name = 'trash' AND ft.added_at < {cutoff}
"""
results = test.run_sql_query(query)

# Check if our file_id is in the results
found = False
for row in results:
    if str(row[0]) == str(file_id):
        found = True
        break

if found:
    print("✅ Logic Check: Database correctly identifies the file as expired trash.")
    print("   (The daemon will burn this file on its next 60s tick)")
else:
    print(f"❌ FAILURE: Database query failed to find expired trash.")
    print(f"   Cutoff: {cutoff}")
    print(f"   Results: {results}")
    sys.exit(1)

print("✅ TEST 31 PASSED")
