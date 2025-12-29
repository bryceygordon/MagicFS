from common import MagicTest
import time
import os

test = MagicTest()
print("--- TEST 05: Chunking & Semantic Dilution ---")

# 1. Create a "Needle in a Haystack" file
# 100 lines of noise about cooking, followed by one critical secret, then more noise.
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

# Increased retries to 30 (15 seconds) to match other tests
for i in range(30):
    if os.path.exists(search_path):
        try:
            files = os.listdir(search_path)
            # Look for the file. The format is "SCORE_filename".
            for f in files:
                if "needle.txt" in f:
                    score_str = f.split("_")[0]
                    try:
                        score = float(score_str)
                        print(f"✅ Found file with score: {score}")
                        
                        # Threshold logic:
                        if score > 0.60:
                            print("✅ Score indicates strong semantic match (Chunking working).")
                            found = True
                            break
                        else:
                            print(f"❌ Score {score} too low! Semantic dilution occurred.")
                            test.dump_logs()
                            exit(1)
                    except ValueError:
                        continue
        except OSError:
            # Directory might vanish momentarily during updates
            pass
            
    if found: break
    
    if i % 5 == 0:
        print(f"   ... waiting for Oracle (attempt {i+1}/30)")
    time.sleep(0.5)

if not found:
    print("❌ FAILURE: File not found in search results.")
    # DEBUG: List directory if it exists to see what WAS found
    if os.path.exists(search_path):
        print(f"   Contents: {os.listdir(search_path)}")
    else:
        print("   Directory does not exist.")
        
    test.dump_logs()
    exit(1)

print("✅ CHUNKING TEST PASSED")
