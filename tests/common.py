import sqlite3
import os
import time
import sys
import shutil

class MagicTest:
    def __init__(self):
        if len(sys.argv) < 4:
            print("Usage: python test.py <db_path> <mount_point> <watch_dir>")
            sys.exit(1)
        
        self.db_path = sys.argv[1]
        self.mount_point = sys.argv[2]
        self.watch_dir = sys.argv[3]

    def create_file(self, rel_path, content):
        """Creates a file in the watch directory."""
        full_path = os.path.join(self.watch_dir, rel_path)
        dir_name = os.path.dirname(full_path)
        
        # If we are creating a new directory, give the watcher a split second
        # to attach to it before creating the file. This mimics real user speed.
        if not os.path.exists(dir_name):
            os.makedirs(dir_name, exist_ok=True)
            time.sleep(0.2) 

        with open(full_path, "w") as f:
            f.write(content)
        print(f"[Setup] Created file: {rel_path}")

    def add_ignore_rule(self, rule):
        """Appends a rule to .magicfsignore."""
        ignore_path = os.path.join(self.watch_dir, ".magicfsignore")
        with open(ignore_path, "a") as f:
            f.write(f"\n{rule}\n")
        print(f"[Setup] Added ignore rule: {rule}")
        time.sleep(0.5)

    def wait_for_indexing(self, filename_substr, timeout=5):
        """Polls DB until file appears."""
        print(f"[Wait] Waiting for '{filename_substr}' to be indexed...")
        start = time.time()
        while time.time() - start < timeout:
            if self.check_file_in_db(filename_substr):
                print(f"✅ Found '{filename_substr}' in index.")
                return True
            time.sleep(0.1)
        
        print(f"❌ Timeout waiting for {filename_substr}")
        sys.exit(1) # HARD FAIL to preserve logs

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
        sys.exit(1)

    def assert_file_not_indexed(self, filename_substr):
        time.sleep(1) 
        if self.check_file_in_db(filename_substr):
            print(f"❌ FAILURE: Should ignore '{filename_substr}', but found it in DB.")
            sys.exit(1)
        print(f"✅ Correctly ignored '{filename_substr}'.")
        return True

    def search_fs(self, query, expected_filename, retries=10):
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
            
            print(f"    ... waiting for Oracle (attempt {i+1}/{retries})")
            time.sleep(0.5)
            
        print(f"❌ FAILURE: Search for '{query}' failed.")
        sys.exit(1)
