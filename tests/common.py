import sqlite3
import os
import time
import sys

class MagicTest:
    def __init__(self):
        # Read args passed by run_suite.sh
        if len(sys.argv) < 3:
            print("Usage: python test.py <db_path> <mount_point>")
            sys.exit(1)
        self.db_path = sys.argv[1]
        self.mount_point = sys.argv[2]

    def get_indexed_files(self):
        """Returns a list of all absolute paths currently in the database."""
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT abs_path FROM file_registry")
        files = [row[0] for row in cursor.fetchall()]
        conn.close()
        return files

    def assert_file_indexed(self, filename_substr):
        """Fails if a file containing filename_substr is NOT in the DB."""
        files = self.get_indexed_files()
        if any(filename_substr in f for f in files):
            print(f"✅ Found '{filename_substr}' in index.")
            return True
        print(f"❌ FAILURE: '{filename_substr}' missing from index.")
        sys.exit(1)

    def assert_file_not_indexed(self, filename_substr):
        """Fails if a file containing filename_substr IS in the DB."""
        files = self.get_indexed_files()
        matches = [f for f in files if filename_substr in f]
        if matches:
            print(f"❌ FAILURE: Should ignore '{filename_substr}', but found: {matches}")
            sys.exit(1)
        print(f"✅ Correctly ignored '{filename_substr}'.")
        return True

    def search_fs(self, query, expected_filename, retries=5):
        """Performs a FUSE search with retries for the Oracle."""
        search_path = os.path.join(self.mount_point, "search", query)
        
        print(f"[*] Searching for '{query}'...")
        
        for i in range(retries):
            try:
                if os.path.exists(search_path):
                    results = os.listdir(search_path)
                    if results:
                        # Check content
                        if any(expected_filename in r for r in results):
                            print(f"✅ Found '{expected_filename}' in search results.")
                            return True
            except OSError:
                pass # Expected EAGAIN
            
            print(f"    ... waiting for Oracle (attempt {i+1}/{retries})")
            time.sleep(1)
            
        print(f"❌ FAILURE: Search for '{query}' failed or didn't contain '{expected_filename}'")
        sys.exit(1)
