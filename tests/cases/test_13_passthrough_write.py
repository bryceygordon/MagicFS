from common import MagicTest
import os
import time
import sys

test = MagicTest()
print("--- TEST 13: Passthrough Writing ---")

# 1. Create a file with initial content
filename = "minutes.txt"
initial_content = "Meeting Minutes: \n- Topic A: Pending\n- Topic B: Pending"
test.create_file(filename, initial_content)

# 2. Wait for indexing
test.wait_for_indexing(filename)

# 3. Search to find the virtual handle
print(f"[*] Searching for 'Meeting Minutes'...")
query = "Meeting Minutes"

# Retry loop to find the file
search_dir = os.path.join(test.mount_point, "search", query)
target_file = None

for i in range(20):
    if os.path.exists(search_dir):
        try:
            files = os.listdir(search_dir)
            target_file = next((f for f in files if filename in f), None)
            if target_file: break
        except OSError:
            pass
    time.sleep(0.5)

if not target_file:
    print("❌ FAILURE: Could not find virtual file.")
    sys.exit(1)

virtual_path = os.path.join(search_dir, target_file)
print(f"✅ Found virtual file: {virtual_path}")

# 4. PASSTHROUGH WRITE TEST
# We will append a new line to the virtual file.
print("[*] Writing to virtual file...")
new_line = "\n- Topic C: APPROVED BY MAGICFS"
expected_content = initial_content + new_line

try:
    # 'a' mode (append) usually triggers open() -> write().
    # 'w' mode (write) usually triggers setattr(size=0) -> write().
    # Let's test 'w' (overwrite) to verify Truncate + Write support.
    
    with open(virtual_path, "w") as f:
        f.write(expected_content)
        
    print("    Write operation completed without error.")

except Exception as e:
    print(f"❌ FAILURE: IO Error writing to virtual file: {e}")
    sys.exit(1)

# 5. Verify the REAL file on disk
real_path = os.path.join(test.watch_dir, filename)
print(f"[*] Verifying real file: {real_path}")

with open(real_path, "r") as f:
    real_content = f.read()

if real_content == expected_content:
    print("✅ SUCCESS: Real file updated correctly via MagicFS.")
else:
    print("❌ FAILURE: Real file content mismatch!")
    print(f"    Expected:\n{expected_content}")
    print(f"    Got:\n{real_content}")
    sys.exit(1)

# 6. Verify Metadata Update (Optional but good)
# The write should have updated the mtime on the real file.
# Note: Python's os.stat might cache, but let's check.
# Since we overwrote it, the mtime should be very recent.
mtime = os.path.getmtime(real_path)
now = time.time()
if (now - mtime) < 5.0:
    print("✅ Metadata: File modification time was updated.")
else:
    print(f"⚠️  Warning: File mtime seems old ({now - mtime}s ago). Setattr might have failed.")
