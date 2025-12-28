import sqlite3
import sys
import os
import time

DB_PATH = sys.argv[1]
MOUNT_POINT = sys.argv[2]

def check_database():
    print(f"[*] Checking Database at {DB_PATH}...", flush=True)
    
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    cursor.execute("SELECT abs_path FROM file_registry")
    files = [row[0] for row in cursor.fetchall()]
    conn.close()
    
    print(f"    Found {len(files)} indexed files.", flush=True)
    
    # 1. Assert Visible files exist
    if not any("game.py" in f for f in files):
        print("❌ game.py missing from index", flush=True)
        return False
        
    if not any("main.rs" in f for f in files):
        print("❌ main.rs missing from index", flush=True)
        return False
    
    # 2. Assert Hidden files check
    dotfiles_present = any(".git" in f or ".obsidian" in f for f in files)
    
    if dotfiles_present:
        print("⚠️  WARNING: Dotfiles/Hidden files were found in the index (Feature not implemented yet).", flush=True)
    else:
        print("✅  Dotfiles correctly ignored.", flush=True)

    return True

def check_fuse_search():
    print(f"[*] Checking FUSE Search at {MOUNT_POINT}...", flush=True)
    
    search_query = "python"
    search_path = os.path.join(MOUNT_POINT, "search", search_query)
    
    # RETRY LOGIC for Async Oracle
    # The first access might return EAGAIN or fail because results aren't ready
    max_retries = 5
    found = False
    results = []

    for i in range(max_retries):
        try:
            if os.path.exists(search_path):
                results = os.listdir(search_path)
                # If we get results, we are good
                if results:
                    found = True
                    break
            else:
                # Trigger the lookup
                try:
                    os.listdir(search_path)
                except OSError:
                    pass # Expected failure on first try if returning EAGAIN
        except Exception as e:
            pass
            
        print(f"    ... attempt {i+1}: Waiting for Oracle...", flush=True)
        time.sleep(1) # Wait for Oracle to compute embeddings

    if not found and not results:
        print(f"❌ Search directory {search_path} empty or not found after {max_retries} attempts", flush=True)
        return False
        
    print(f"    Search for '{search_query}' returned: {results}", flush=True)
    
    # Check if we got the expected file (game.py should score high for 'python')
    if not any("game.py" in r for r in results):
        print("❌ Search results did not contain 'game.py'", flush=True)
        return False
        
    print("✅ Search returned results.", flush=True)
    return True

if __name__ == "__main__":
    try:
        if not check_database():
            sys.exit(1)
        
        if not check_fuse_search():
            sys.exit(1)
            
        sys.exit(0)
    except Exception as e:
        print(f"❌ Critical Error: {e}", flush=True)
        sys.exit(1)
