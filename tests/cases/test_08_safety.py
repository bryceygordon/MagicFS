from common import MagicTest
import subprocess
import time
import os
import signal

test = MagicTest()
print("--- TEST 08: Safety Systems (Feedback Loop Prevention) ---")

# 1. Kill the 'Good' Daemon started by run_suite.sh
print("[Setup] Killing existing daemon...")
subprocess.run(["sudo", "pkill", "-x", "magicfs"])
time.sleep(1)
# Force unmount to clear the slate
subprocess.run(["sudo", "umount", "-l", test.mount_point], stderr=subprocess.DEVNULL)

# 2. Configure the Dangerous Scenario (Microphone pointing at Speaker)
# We attempt to WATCH a directory that is INSIDE the MOUNT point.
# Mount: /tmp/magicfs-test-mount
# Watch: /tmp/magicfs-test-mount/internal_folder
dangerous_watch = os.path.join(test.mount_point, "internal_folder")
# Note: We can't actually mkdir inside the mountpoint if it's not mounted yet,
# but the Daemon calculates absolute paths, so the string comparison is enough.

print(f"[Action] Attempting to mount with feedback loop configuration:")
print(f"   Mount: {test.mount_point}")
print(f"   Watch: {dangerous_watch}")

binary = "./target/debug/magicfs"
log_file = "tests/test_08_crash.log"

# 3. Launch Daemon and expect immediate failure
with open(log_file, "w") as log:
    proc = subprocess.Popen(
        ["sudo", "RUST_LOG=info", binary, test.mount_point, dangerous_watch],
        stdout=log,
        stderr=log
    )

print("[Wait] Waiting for daemon to realize the error...")
time.sleep(2)

# 4. Check if process is still alive
if proc.poll() is None:
    print("❌ FAILURE: Daemon started successfully in a feedback loop configuration!")
    print("   The process should have panic()'d immediately.")
    
    # Kill it so we don't leave a mess
    subprocess.run(["sudo", "kill", str(proc.pid)])
    exit(1)

# 5. Analyze Logs for the Safety Switch Message
print("[Assert] Checking logs for Safety Switch engagement...")
found_error = False
with open(log_file, "r") as f:
    content = f.read()
    if "Feedback Loop Detected" in content:
        found_error = True
        print("✅ Found expected panic: 'Feedback Loop Detected'")

if not found_error:
    print("❌ FAILURE: Process died, but specific error message was missing.")
    print(f"   Log contents:\n{content}")
    exit(1)

print("✅ Safety Systems Test Passed.")
