from common import MagicTest
import time

test = MagicTest()
print("--- TEST 37: Transient File Suppression ---")

# 1. Create transient files that should be ignored
test.create_file("ignore_me.part", "This should be ignored")
test.create_file("ignore_me.tmp", "This should be ignored")
test.create_file("ignore_me.crdownload", "This should be ignored")

# 2. Create a valid file that should be indexed
test.create_file("valid.txt", "This should be indexed")

# 3. Wait for processing to complete
print("Waiting for conveyor to stabilize...")
test.wait_for_stable_db(stability_duration=2)

# 4. Assertions
test.assert_file_indexed("valid.txt")          # Should be found
test.assert_file_not_indexed("ignore_me.part") # Should NOT be found
test.assert_file_not_indexed("ignore_me.tmp")  # Should NOT be found
test.assert_file_not_indexed("ignore_me.crdownload") # Should NOT be found

# 5. Log verification - check that transient files are NOT processed
print("Checking logs for transient file processing...")
try:
    with open(test.log_file, "r") as f:
        log_content = f.read()

    # These patterns should NOT appear in logs
    bad_patterns = ["Processing: ignore_me.part", "Processing: ignore_me.tmp", "Processing: ignore_me.crdownload"]

    found_bad = []
    for pattern in bad_patterns:
        if pattern in log_content:
            found_bad.append(pattern)

    if found_bad:
        print(f"❌ FAILURE: Found unwanted log patterns: {found_bad}")
        test.dump_logs()
        exit(1)
    else:
        print("✅ Log verification passed: Transient files were properly ignored")

except Exception as e:
    print(f"⚠️  Warning: Could not verify logs: {e}")

print("✅ TEST 37 PASSED")