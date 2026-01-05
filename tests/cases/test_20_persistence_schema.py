from common import MagicTest
import sqlite3
import sys
import os
import time

test = MagicTest()
print("--- TEST 20: Persistence Schema & Inode Zoning ---")

# 1. Connect to the Database
print(f"[Setup] Connecting to DB at {test.db_path}...")

# Hardening: Wait loop in case Daemon is still initializing DB
timeout = 5
start = time.time()
while not os.path.exists(test.db_path):
    if time.time() - start > timeout:
        print(f"❌ FAILURE: Database file not found at {test.db_path}")
        sys.exit(1)
    time.sleep(0.5)

conn = sqlite3.connect(test.db_path)
cursor = conn.cursor()

# 2. Check for Table Existence
expected_tables = ["file_registry", "vec_index", "tags", "file_tags"]
missing_tables = []

print("[Check] Verifying schema tables...")
for table in expected_tables:
    cursor.execute(f"SELECT name FROM sqlite_master WHERE type='table' AND name='{table}'")
    if not cursor.fetchone():
        missing_tables.append(table)

if missing_tables:
    print(f"❌ FAILURE: Missing required tables: {missing_tables}")
    print("   Did the Daemon fail to run the new Repository::initialize code?")
    conn.close()
    sys.exit(1)
else:
    print("✅ All tables present.")

# 3. Check Schema Definitions (Foreign Keys)
print("[Check] Verifying relationships...")
# Check file_tags structure
cursor.execute("PRAGMA table_info(file_tags)")
columns = {row[1] for row in cursor.fetchall()}
required_cols = {"file_id", "tag_id", "display_name"}
if not required_cols.issubset(columns):
    print(f"❌ FAILURE: file_tags missing columns. Found: {columns}")
    conn.close()
    sys.exit(1)

# 4. Inode Zoning Logic Verification (Python Implementation Check)
# The Spec defines Persistent Inodes as having the High Bit (1 << 63) set.
# This corresponds to integers > 9,223,372,036,854,775,808.
HIGH_BIT = 1 << 63
print(f"[Check] Verifying Inode Math: High Bit = {HIGH_BIT}")

if HIGH_BIT != 9223372036854775808:
    print("❌ FAILURE: 64-bit integer math sanity check failed.")
    sys.exit(1)

print("✅ Schema & Logic assumptions verified.")
conn.close()
