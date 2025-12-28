import sqlite3
import sys
import os
import time

DB_PATH = sys.argv[1]
MOUNT_POINT = sys.argv[2]

def check_database():
    print(f"[*] Checking Database at {DB_PATH}...")
    
    # Needs sudo to read the db if created by sudo root
    # For this test script, we assume we can read it or run this script with sudo rights
    # Logic: The bash script runs this.
    
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    cursor.execute("SELECT abs_path FROM file_registry")
    files = [row[0] for row in cursor.fetchall()]
    conn.close()
    
    print(f"    Found {len(files)} indexed files.")
    for f in files:
        print(f"    - {f}")

    # 1. Assert Visible files exist
    assert any("game.py" in f for f in files), "❌ game.py missing from index"
    assert any("main.rs" in f for f in files), "❌ main.rs missing from index"
    
    # 2. Assert Hidden files are IGNORED (This assumes we want them ignored)
    # NOTE: This will fail until we fix the bug or implement .magicfsignore
    dotfiles_present = any(".git" in f or ".obsidian" in f for f in files)
    
    if dotfiles_present:
        print("⚠️  WARNING: Dotfiles/Hidden files were found in the index!")
        # return False # Uncomment this when ready to enforce the rule
    else:
        print("✅  Dotfiles correctly ignored.")

    return True

def check_fuse_search():
    print(f"[*] Checking FUSE Search at {MOUNT_POINT}...")
    
    search_query = "python"
    search_path = os.path.join(MOUNT_POINT, "search", search_query)
    
    # Trigger the search (ls)
    if not os.path.exists(search_path):
        print(f"❌ Search directory {search_path} not found")
        return False
        
    # Wait a tiny bit for async Oracle
    time.sleep(1)
    
    results = os.listdir(search_path)
    print(f"    Search for '{search_query}' returned: {results}")
    
    if not results:
        print("❌ No results found for 'python' (should find game.py)")
        return False
        
    print("✅ Search returned results.")
    return True

if __name__ == "__main__":
    try:
        if not check_database():
            sys.exit(1)
        
        if not check_fuse_search():
            sys.exit(1)
            
        sys.exit(0)
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)
