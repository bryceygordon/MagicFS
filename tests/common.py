import sqlite3
import os
import time
import sys
import shutil
import subprocess

class MagicTest:
    def __init__(self):
        if len(sys.argv) < 4:
            print("Usage: python test.py <db_path> <mount_point> <watch_dir>")
            sys.exit(1)
        
        self.db_path = sys.argv[1]
        self.mount_point = sys.argv[2]
        self.watch_dir = sys.argv[3]
        
        # NEW: Read log location from Env, default to tests/magicfs.log
        self.log_file = os.environ.get("MAGICFS_LOG_FILE", "tests/magicfs.log")

    def dump_logs(self, lines=100):
        """Reads the log file directly and dumps it to stdout."""
        print(f"\n--- FATAL ERROR: DUMPING LAST {lines} LOG LINES ({self.log_file}) ---")
        try:
            if os.path.exists(self.log_file):
                with open(self.log_file, "r") as f:
                    content = f.readlines()
                    
                    if not content:
                        print("⚠️  Log file exists but is EMPTY.")
                    
                    for line in content[-lines:]:
                        print(line.rstrip())
            else:
                print(f"❌ Log file not found at {self.log_file}")
        except Exception as e:
            print(f"❌ Failed to read log file: {e}")
            # Fallback: Try system cat in case of weird permission issues
            try:
                import subprocess
                print("--- Trying system 'cat' ---")
                subprocess.run(["cat", self.log_file])
            except:
                pass
        print("---------------------------------------------------\n")

    def create_file(self, rel_path, content):
        full_path = os.path.join(self.watch_dir, rel_path)
        dir_name = os.path.dirname(full_path)
        if not os.path.exists(dir_name):
            os.makedirs(dir_name, exist_ok=True)
            time.sleep(0.2) 
        with open(full_path, "w") as f:
            f.write(content)
        print(f"[Setup] Created file: {rel_path}")

    def add_ignore_rule(self, rule):
        ignore_path = os.path.join(self.watch_dir, ".magicfsignore")
        with open(ignore_path, "a") as f:
            f.write(f"\n{rule}\n")
        print(f"[Setup] Added ignore rule: {rule}")
        time.sleep(0.5)

    def get_db_count(self):
        """Robust DB count using sudo sqlite3 to handle daemon WAL locks."""
        try:
            result = subprocess.run(
                ["sudo", "sqlite3", self.db_path, "SELECT count(*) FROM file_registry;"],
                capture_output=True, text=True, timeout=5
            )
            if result.returncode == 0 and result.stdout.strip():
                return int(result.stdout.strip())
            else:
                print(f"[WARN] get_db_count failed: {result.stderr}")
                return 0
        except Exception as e:
            print(f"[WARN] get_db_count exception: {e}")
            return 0

    def wait_for_stable_db(self, stability_duration=3, max_wait=120):
        """
        Polls the DB. 
        If count is increasing, we keep waiting (resetting the stable timer).
        If count stays the same for 'stability_duration' seconds, we assume conveyor is empty.
        """
        print("[Sensor] Monitoring conveyor belt (DB activity)...")
        last_count = -1
        stable_start = None
        start_time = time.time()

        while time.time() - start_time < max_wait:
            current_count = self.get_db_count()
            
            if current_count != last_count:
                # Conveyor is moving!
                if last_count != -1:
                    print(f"  [Moving] Processed {current_count} files...")
                last_count = current_count
                stable_start = None # Reset stable timer
            else:
                # Conveyor looks stopped. How long has it been stopped?
                if stable_start is None:
                    stable_start = time.time()
                
                elapsed_stable = time.time() - stable_start
                if elapsed_stable >= stability_duration:
                    print(f"  [Stopped] DB stable at {current_count} files for {stability_duration}s.")
                    return True
            
            time.sleep(0.5)
        
        print("❌ Timeout waiting for DB to stabilize.")
        return False

    def wait_for_indexing(self, filename_substr, timeout=10):
        # We wrap the smart waiter first, then do a quick check
        print(f"[Wait] Waiting for '{filename_substr}'...")
        start = time.time()
        while time.time() - start < timeout:
            if self.check_file_in_db(filename_substr):
                print(f"✅ Found '{filename_substr}' in index.")
                return True
            time.sleep(0.1)
        
        print(f"❌ Timeout waiting for {filename_substr}")
        self.dump_logs()
        sys.exit(1)

    def check_file_in_db(self, filename_substr):
        try:
            conn = sqlite3.connect(self.db_path)
            cursor = conn.cursor()
            cursor.execute("SELECT abs_path FROM file_registry")
            files = [row[0] for row in cursor.fetchall()]
            conn.close()
            return any(filename_substr in f for f in files)
        except:
            return False

    def assert_file_indexed(self, filename_substr):
        if self.check_file_in_db(filename_substr):
            print(f"✅ Found '{filename_substr}' in index.")
            return True
        print(f"❌ FAILURE: '{filename_substr}' missing from index.")
        self.dump_logs()
        sys.exit(1)

    def assert_file_not_indexed(self, filename_substr):
        # We wait a moment to ensure any pending ops would have finished
        time.sleep(1.0)
        if self.check_file_in_db(filename_substr):
            print(f"❌ FAILURE: Should ignore '{filename_substr}', but found it in DB.")
            self.dump_logs()
            sys.exit(1)
        print(f"✅ Correctly ignored '{filename_substr}'.")
        return True

    def search_fs(self, query, expected_filename, retries=30):
        search_path = os.path.join(self.mount_point, "search", query)
        print(f"[*] Searching for '{query}'...")
        
        for i in range(retries):
            try:
                if os.path.exists(search_path):
                    results = os.listdir(search_path)
                    if any(expected_filename in r for r in results):
                        print(f"✅ Found '{expected_filename}' in search results.")
                        return True
            except OSError:
                pass 
            
            if i % 5 == 0:
                print(f"   ... waiting for Oracle (attempt {i+1}/{retries})")
            time.sleep(0.5)
            
        print(f"❌ FAILURE: Search for '{query}' failed.")
        self.dump_logs()
        sys.exit(1)
