# FILE: tests/cases/test_22_tag_listing.py
from common import MagicTest
import os
import subprocess
import time
import sys

test = MagicTest()
print("--- TEST 22: Tag View Listing & Content Resolution ---")

# 1. Setup: Create a real file on disk
filename = "invoice_2024.pdf"
content = "Total: $500.00 - Paid in Full"
real_path = os.path.join(test.watch_dir, filename)
test.create_file(filename, content)

# 2. Wait for Indexer to pick it up (so we get a valid file_id)
test.wait_for_indexing(filename)

# 3. Retrieve metadata via sudo sqlite3 (to avoid WAL permission issues)
cmd = ["sudo", "sqlite3", test.db_path, f"SELECT file_id, inode FROM file_registry WHERE abs_path = '{real_path}'"]
result = subprocess.run(cmd, capture_output=True, text=True, check=True)
if result.stdout.strip():
    parts = result.stdout.strip().split('|')
    row = (int(parts[0]), int(parts[1])) if len(parts) >= 2 else None
else:
    row = None

if not row:
    print("❌ FAILURE: File not found in registry after indexing.")
    sys.exit(1)

file_id, file_inode = row
print(f"[Setup] File '{filename}' has ID {file_id} and Inode {file_inode}")

# 4. Inject Tag & Link via SUDO (Bypass Permission Lock)
print("[Setup] Injecting tag and link via sudo...")

sql_script = f"""
INSERT INTO tags (name, color, icon) VALUES ('finance', 'green', '💰');
INSERT INTO file_tags (file_id, tag_id, display_name) 
VALUES ({file_id}, (SELECT tag_id FROM tags WHERE name='finance'), '{filename}');
"""

try:
    cmd = ["sudo", "sqlite3", test.db_path, sql_script]
    subprocess.run(cmd, check=True)
    print(f"[Setup] Linked File {file_id} to Tag 'finance'")
except subprocess.CalledProcessError as e:
    print(f"❌ FAILURE: DB Injection failed: {e}")
    sys.exit(1)

# 5. Verify Listing (readdir)
tag_view_path = os.path.join(test.mount_point, "tags", "finance")
print(f"[Action] Listing {tag_view_path}...")

try:
    if not os.path.exists(tag_view_path):
        print(f"❌ FAILURE: Tag directory {tag_view_path} does not exist.")
        sys.exit(1)

    items = os.listdir(tag_view_path)
    print(f"  Contents: {items}")

    if filename not in items:
        print(f"❌ FAILURE: '{filename}' not found in tag view.")
        sys.exit(1)
    
    print("✅ Listing successful.")

except OSError as e:
    print(f"❌ FAILURE: OS Error during listing: {e}")
    sys.exit(1)

# 6. Verify Reading (lookup + read)
virtual_file_path = os.path.join(tag_view_path, filename)
print(f"[Action] Reading {virtual_file_path}...")

try:
    with open(virtual_file_path, "r") as f:
        read_content = f.read()
    
    if read_content == content:
        print("✅ Content match.")
    else:
        print(f"❌ FAILURE: Content mismatch. Got '{read_content}'")
        sys.exit(1)

except Exception as e:
    print(f"❌ FAILURE: Could not read file: {e}")
    sys.exit(1)

print("✅ TAG LISTING TEST PASSED")
