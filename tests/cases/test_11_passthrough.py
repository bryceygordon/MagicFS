from common import MagicTest
import os
import time
import sys

test = MagicTest()
print("--- TEST 11: Passthrough Reading (Streaming) ---")

# 1. Create a file with known content
filename = "contract.txt"
secret_content = "This is a binding legal document. The secret clause is: SECTION_9_ALPHA."
test.create_file(filename, secret_content)

# 2. Wait for indexing
test.wait_for_indexing(filename)

# 3. Search for the file to expose it in the FUSE layer
print(f"[*] Searching for 'legal document'...")
query = "legal document"

# Use explicit wait loop to debug visibility
search_dir = os.path.join(test.mount_point, "search", query)
found = False

for i in range(30): # 15 seconds
    if os.path.exists(search_dir):
        try:
            files = os.listdir(search_dir)
            # Check if our file is in the list (matching partial name)
            target_file = next((f for f in files if filename in f), None)
            
            if target_file:
                print(f"✅ Found '{target_file}' in search results.")
                found = True
                break
            else:
                # Directory exists but is empty or doesn't have our file yet
                pass
        except OSError:
            pass
    
    time.sleep(0.5)

if not found:
    print("❌ FAILURE: Search timed out.")
    if os.path.exists(search_dir):
        print(f"   Directory contents: {os.listdir(search_dir)}")
    else:
        print("   Directory does not exist (EAGAIN loop failed).")
    test.dump_logs()
    sys.exit(1)

# 4. PASSTHROUGH READ TEST
target_file = next(f for f in os.listdir(search_dir) if filename in f)
virtual_path = os.path.join(search_dir, target_file)

print(f"[*] Reading virtual file: {virtual_path}")
try:
    with open(virtual_path, "r") as f:
        read_content = f.read()
    
    print(f"    Read {len(read_content)} bytes.")
    
    if read_content == secret_content:
        print("✅ SUCCESS: Content matches exactly.")
    else:
        print("❌ FAILURE: Content mismatch!")
        print(f"    Expected: '{secret_content}'")
        print(f"    Got:      '{read_content}'")
        sys.exit(1)

except Exception as e:
    print(f"❌ FAILURE: IO Error reading virtual file: {e}")
    sys.exit(1)

print("✅ PASSTHROUGH TEST PASSED")
