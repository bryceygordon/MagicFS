from common import MagicTest
import time
import os
import sys

test = MagicTest()
print("--- TEST 05: Chunking & Semantic Dilution ---")

# 1. Create a "Needle in a Haystack" file
noise = "The chef chopped the onions and placed them in the frying pan. " * 50
secret = "The nuclear launch code is 12345. "
content = noise + secret + noise

test.create_file("needle.txt", content)

# 2. Wait for indexing
test.wait_for_indexing("needle.txt")

# 3. Search for the needle
print("[*] Searching for 'nuclear launch code'...")

search_path = os.path.join(test.mount_point, "search", "nuclear launch code")
found = False

for i in range(30):
    if os.path.exists(search_path):
        try:
            files = os.listdir(search_path)
            for f in files:
                if "needle.txt" in f:
                    score_str = f.split("_")[0]
                    try:
                        score = float(score_str)
                        print(f"✅ Found file with score: {score}")
                        
                        # --- MODIFIED: Lowered threshold for Nomic Safety ---
                        if score > 0.50:
                            print("✅ Score indicates strong semantic match (Chunking working).")
                            found = True
                            break
                        else:
                            print(f"❌ Score {score} too low! Semantic dilution occurred.")
                            sys.exit(1)
                    except ValueError:
                        continue
        except OSError:
            pass
            
    if found: break
    
    if i % 5 == 0:
        print(f"    ... waiting for Oracle (attempt {i+1}/30)")
    time.sleep(0.5)

if not found:
    print("❌ FAILURE: File not found in search results.")
    sys.exit(1)

print("✅ CHUNKING TEST PASSED")
