from common import MagicTest
import time

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
# In a non-chunked system, this search often fails or returns a score so low
# it's filtered out, because the embedding is dominated by "onions" and "frying pans".
# We expect MagicFS to find this with high confidence.
print("[*] Searching for 'nuclear launch code'...")

# We manually check the search results to inspect the score
import os
search_path = os.path.join(test.mount_point, "search", "nuclear launch code")

found = False
for i in range(10):
    if os.path.exists(search_path):
        files = os.listdir(search_path)
        # Look for the file. The format is "SCORE_filename".
        # We want to ensure it found it AND the score is decent.
        for f in files:
            if "needle.txt" in f:
                score_str = f.split("_")[0]
                score = float(score_str)
                print(f"✅ Found file with score: {score}")
                
                # Threshold logic:
                # - Noise typically scores < 0.45
                # - A 30-char needle in a 256-char chunk is ~12% signal.
                # - A score of 0.60+ is a statistically significant match.
                # - We saw 0.75 in testing, so 0.60 is a safe robust guardrail.
                if score > 0.60:
                    print("✅ Score indicates strong semantic match (Chunking working).")
                    found = True
                    break
                else:
                    print("❌ Score too low! Semantic dilution occurred.")
                    exit(1)
        if found: break
    
    time.sleep(0.5)

if not found:
    print("❌ FAILURE: File not found in search results.")
    exit(1)

print("✅ CHUNKING TEST PASSED")
