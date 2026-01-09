# FILE: tests/cases/test_25_import.py
from common import MagicTest
import os
import shutil
import sys
import subprocess
import time

test = MagicTest()
print("--- TEST 25: Import via Copy (The Landing Zone) ---")

# 1. Setup: Create an external source file (outside MagicFS)
source_path = "/tmp/external_source.txt"
with open(source_path, "w") as f:
    f.write("I am an external file being imported.")

# 2. Setup: Create 'projects' tag
subprocess.run(["sudo", "sqlite3", test.db_path, "INSERT INTO tags (name) VALUES ('projects');"], check=True)

# 3. Action: Copy file into the Tag View
dest_path = os.path.join(test.mount_point, "tags", "projects", "imported_doc.txt")
print(f"[Action] Copying {source_path} -> {dest_path}")

try:
    # This triggers create() -> write() -> release()
    shutil.copyfile(source_path, dest_path)
    print("✅ Copy operation completed successfully.")
except IOError as e:
    print(f"❌ FAILURE: Copy failed: {e}")
    sys.exit(1)

# 4. Assert: Persistence (Does it show up in ls?)
print("[Assert] Checking virtual existence...")
if os.path.exists(dest_path):
    print("✅ File visible in Tag View.")
else:
    print("❌ FAILURE: File not visible after copy.")
    sys.exit(1)

# 5. Assert: Physicality (Did it land in _imported?)
# Note: In our test env, WATCH_DIR is the root.
landing_zone = os.path.join(test.watch_dir, "_imported")
imported_physical = os.path.join(landing_zone, "imported_doc.txt")

print(f"[Assert] Checking physical existence in {landing_zone}...")
if os.path.exists(imported_physical):
    print("✅ File physically exists in _imported.")
    with open(imported_physical, "r") as f:
        if f.read() == "I am an external file being imported.":
            print("✅ Content verified.")
        else:
            print("❌ FAILURE: Content corruption.")
            sys.exit(1)
else:
    print(f"❌ FAILURE: File not found in {landing_zone}")
    sys.exit(1)

# 6. Cleanup
os.remove(source_path)
print("✅ IMPORT TEST PASSED")
