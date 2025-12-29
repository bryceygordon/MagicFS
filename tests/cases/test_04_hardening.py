from common import MagicTest
import os
import time

test = MagicTest()
print("--- TEST 04: Hardening & Resilience ---")

# 1. Create a Binary File (Should be ignored due to null byte)
# We use 'wb' to write raw bytes including the null terminator
binary_path = os.path.join(test.watch_dir, "app.exe") # Fake exe
with open(binary_path, "wb") as f:
    f.write(b"ELF\x00\x01\x02\x03This is a binary file")
print(f"[Setup] Created binary file: app.exe")

# 2. Create a Large File (Should be ignored due to >10MB limit)
# We create an 11MB file efficiently
large_path = os.path.join(test.watch_dir, "huge_log.txt")
with open(large_path, "wb") as f:
    # Write 11MB of 'A's
    f.write(b"A" * (11 * 1024 * 1024))
print(f"[Setup] Created large file: huge_log.txt (11MB)")

# 3. Create a Control File (Should be indexed)
# This acts as our "Canary" - if this gets indexed, we know the Oracle 
# didn't crash while processing the bad files.
test.create_file("valid.rs", "fn main() { println!(\"I am valid\"); }")

# 4. Wait for the control file (proves the system is alive)
test.wait_for_indexing("valid.rs")

# 5. Assertions
print("[Assert] Verifying bad files were rejected...")

# The Oracle logic is: Extract -> If empty, Skip -> Register.
# Since extraction returns "" for binary/large files, they should NEVER reach the registry.

if test.check_file_in_db("app.exe"):
    print("❌ FAILURE: Binary file 'app.exe' was indexed!")
    exit(1)
else:
    print("✅ Binary file correctly ignored.")

if test.check_file_in_db("huge_log.txt"):
    print("❌ FAILURE: Large file 'huge_log.txt' was indexed!")
    exit(1)
else:
    print("✅ Large file correctly ignored.")

print("✅ HARDENING TEST PASSED")
