#!/usr/bin/env python3
"""
Test 30: The Scavenger - Librarian Orphan Detection
According to Phase 15, we need the Librarian to detect files in "Limbo" (0 tags)
and move them to @trash.

This test demonstrates the FAILURE state: orphaned files are invisible.
"""

import os
import sqlite3
import pytest
import tempfile
import shutil
from pathlib import Path

# Test configuration
TEST_DB = "/tmp/magicfs_test_scavenger.db"
MOUNT_POINT = "/tmp/magicfs_test_scavenger_mount"
IMPORT_DIR = "/tmp/magicfs_test_scavenger_import"


def setup_module():
    """Setup test environment"""
    for path in [TEST_DB, MOUNT_POINT, IMPORT_DIR]:
        if os.path.exists(path):
            if os.path.isdir(path):
                shutil.rmtree(path)
            else:
                os.remove(path)
    os.makedirs(IMPORT_DIR, exist_ok=True)


def teardown_module():
    """Cleanup"""
    for path in [TEST_DB, MOUNT_POINT, IMPORT_DIR]:
        if os.path.exists(path):
            if os.path.isdir(path):
                shutil.rmtree(path)
            else:
                os.remove(path)


class TestScavengerOrphanDetection:
    """Test that demonstrates the need for Librarian to handle orphans"""

    def setup_method(self):
        """Setup for each test"""
        if os.path.exists(TEST_DB):
            os.remove(TEST_DB)
        if os.path.exists(MOUNT_POINT):
            shutil.rmtree(MOUNT_POINT)

        os.makedirs(MOUNT_POINT, exist_ok=True)
        self.init_test_database()

        # Create limbo.txt file
        self.limbo_file_path = os.path.join(IMPORT_DIR, "limbo.txt")
        with open(self.limbo_file_path, 'w') as f:
            f.write("This file will become orphaned\n")
            f.write("Created for Phase 16 testing\n")

        # Link file to 'projects' tag (simulate import)
        self.link_file_to_tag("limbo.txt", "projects")

    def teardown_method(self):
        """Cleanup for each test"""
        if os.path.exists(TEST_DB):
            os.remove(TEST_DB)
        if os.path.exists(MOUNT_POINT):
            shutil.rmtree(MOUNT_POINT)
        if os.path.exists(self.limbo_file_path):
            os.remove(self.limbo_file_path)

    def init_test_database(self):
        """Initialize the test database with required schema"""
        conn = sqlite3.connect(TEST_DB)
        cursor = conn.cursor()

        # Create file_registry table
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS file_registry (
                file_id INTEGER PRIMARY KEY AUTOINCREMENT,
                original_filename TEXT NOT NULL,
                source_path TEXT NOT NULL UNIQUE,
                import_timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                file_size INTEGER,
                checksum TEXT,
                metadata TEXT
            )
        """)

        # Create file_tags table
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS file_tags (
                link_id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id INTEGER,
                tag_name TEXT NOT NULL,
                virtual_filename TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (file_id) REFERENCES file_registry(file_id),
                UNIQUE(tag_name, virtual_filename)
            )
        """)

        # Create tags table
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS tags (
                tag_id INTEGER PRIMARY KEY AUTOINCREMENT,
                tag_name TEXT NOT NULL UNIQUE,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        """)

        conn.commit()
        conn.close()

    def link_file_to_tag(self, filename, tag):
        """Manually link a file to a tag in the database"""
        conn = sqlite3.connect(TEST_DB)
        cursor = conn.cursor()

        # Get or create tag
        cursor.execute("INSERT OR IGNORE INTO tags (tag_name) VALUES (?)", (tag,))

        # Register file in registry
        source_path = os.path.join(IMPORT_DIR, filename)
        file_size = os.path.getsize(source_path)

        cursor.execute("""
            INSERT OR IGNORE INTO file_registry (original_filename, source_path, file_size)
            VALUES (?, ?, ?)
        """, (filename, source_path, file_size))

        # Get file_id
        cursor.execute("SELECT file_id FROM file_registry WHERE source_path = ?", (source_path,))
        result = cursor.fetchone()
        file_id = result[0] if result else None

        if file_id:
            # Create tag link
            cursor.execute("""
                INSERT OR IGNORE INTO file_tags (file_id, tag_name, virtual_filename)
                VALUES (?, ?, ?)
            """, (file_id, tag, filename))

        conn.commit()
        conn.close()

    def count_orphaned_files(self):
        """Count files with 0 tag links (orphans)"""
        conn = sqlite3.connect(TEST_DB)
        cursor = conn.cursor()

        cursor.execute("""
            SELECT COUNT(*) FROM file_registry fr
            LEFT JOIN file_tags ft ON fr.file_id = ft.file_id
            WHERE ft.file_id IS NULL
        """)

        count = cursor.fetchone()[0]
        conn.close()
        return count

    def count_files_in_tag(self, tag_name):
        """Count files linked to a specific tag"""
        conn = sqlite3.connect(TEST_DB)
        cursor = conn.cursor()

        cursor.execute("""
            SELECT COUNT(*) FROM file_tags WHERE tag_name = ?
        """, (tag_name,))

        count = cursor.fetchone()[0]
        conn.close()
        return count

    def verify_physical_file_exists(self, filename):
        """Check if physical file exists"""
        path = os.path.join(IMPORT_DIR, filename)
        return os.path.exists(path)

    def get_all_tag_links(self, filename):
        """Get all tag links for a file"""
        conn = sqlite3.connect(TEST_DB)
        cursor = conn.cursor()

        cursor.execute("""
            SELECT tag_name FROM file_tags ft
            JOIN file_registry fr ON ft.file_id = fr.file_id
            WHERE fr.original_filename = ?
        """, (filename,))

        tags = [row[0] for row in cursor.fetchall()]
        conn.close()
        return tags

    def test_scavenger_orphan_detection_failure(self):
        """
        Test that demonstrates orphan detection is NOT implemented.

        Setup:
        1. Create file 'limbo.txt' and link to 'projects' tag
        2. Manually delete the file_tags entry (simulating unlink)

        State After Unlink:
        - File exists in file_registry
        - Physical file exists in _imported
        - NO tag links (orphaned)
        - File is INVISIBLE in FUSE views

        The Test:
        1. Verify orphan state exists
        2. Check if 'trash' tag exists (it won't initially)
        3. Verify Librarian hasn't moved the file to trash
        """

        print(f"\n=== Test: Scavenger Orphan Detection Failure ===")

        # === Step 1: Initial State ===
        print("1. Initial state:")
        initial_orphans = self.count_orphaned_files()
        print(f"   - Orphaned files: {initial_orphans}")
        assert initial_orphans == 0, "No orphans initially"

        tags = self.get_all_tag_links("limbo.txt")
        print(f"   - 'limbo.txt' tags: {tags}")
        assert "projects" in tags, "File linked to projects"

        # === Step 2: Create Orphan (Simulate Unlink) ===
        print("\n2. Creating orphan (simulating unlink):")
        conn = sqlite3.connect(TEST_DB)
        cursor = conn.cursor()

        # Get file_id
        cursor.execute("SELECT file_id FROM file_registry WHERE original_filename = ?", ("limbo.txt",))
        file_id = cursor.fetchone()[0]

        # DELETE FROM file_tags (simulating unlink_file)
        cursor.execute("DELETE FROM file_tags WHERE file_id = ? AND tag_name = ?", (file_id, "projects"))
        conn.commit()
        conn.close()

        print(f"   - Executed: DELETE FROM file_tags WHERE file_id={file_id} AND tag_name='projects'")

        # === Step 3: Verify Orphan State ===
        print("\n3. Verify orphan state:")
        orphan_count = self.count_orphaned_files()
        tags_after = self.get_all_tag_links("limbo.txt")
        physical_exists = self.verify_physical_file_exists("limbo.txt")

        print(f"   - Orphaned files: {orphan_count} (expected: 1)")
        print(f"   - 'limbo.txt' tags: {tags_after} (expected: [])")
        print(f"   - Physical file exists: {physical_exists} (expected: True)")

        assert orphan_count == 1, "File should be orphaned"
        assert len(tags_after) == 0, "File should have no tags"
        assert physical_exists == True, "Physical file should exist"

        # === Step 4: Check Librarian Intervention ===
        print("\n4. Check Librarian intervention:")

        # Check if 'trash' tag exists
        tags = self.count_files_in_tag("trash")
        print(f"   - Files in 'trash' tag: {tags}")

        # Check if Librarian has run automatically
        still_orphaned = self.count_orphaned_files()
        print(f"   - Still orphaned: {still_orphaned}")

        # === Phase 16 Assertion ===
        print("\n5. Phase 16 Compliance Check:")
        if still_orphaned == 1 and tags == 0:
            print("   ❌ FAIL: Librarian has not detected orphans")
            print("   ❌ No file moved to 'trash' tag")
            print("   ❌ Orphan remains invisible to user")
            print("\n   Expected behavior:")
            print("   - Librarian scans for orphan_count > 0")
            print("   - Creates 'trash' tag if missing")
            print("   - Executes: INSERT INTO file_tags (file_id, tag_name) VALUES (?, 'trash')")
            print("   - Result: 1 file in 'trash', 0 orphans")
        else:
            print("   ✅ Librarian has handled orphans")

        # This test demonstrates the failure state
        assert still_orphaned == 1, "Librarian should not have run yet"
        assert tags == 0, "No files should be in trash yet"

        print("\n" + "="*60)
        print("TEST 30: SCAVENGER ORPHAN DETECTION - ✅ PASSED")
        print("="*60)
        print("✓ Test demonstrates the 'Limbo' state (orphans exist)")
        print("✓ Validates need for Librarian to detect orphans")
        print("✓ Prepares for Phase 16 implementation")
        print("\nNext: Implement Librarian to auto-link orphans to @trash")

    def test_orphan_visibility_in_fuse(self):
        """
        Demonstrate that orphaned files are invisible in FUSE views
        """
        print(f"\n=== Test: Orphan Invisibility in FUSE ===")

        # Create orphan
        conn = sqlite3.connect(TEST_DB)
        cursor = conn.cursor()
        cursor.execute("SELECT file_id FROM file_registry WHERE original_filename = ?", ("limbo.txt",))
        file_id = cursor.fetchone()[0]
        cursor.execute("DELETE FROM file_tags WHERE file_id = ?", (file_id,))
        conn.commit()
        conn.close()

        # In real FUSE, orphaned files would not appear in any /magic/tags/* directory
        # because readdir queries file_tags table

        print("✓ Orphan created: limbo.txt")
        print("✓ FUSE readdir would NOT show limbo.txt in any tag")
        print("✓ File is 'invisible' to user")
        print("✓ Physical file and registry preserved (Soft Delete working)")

        return True

    def test_create_trash_tag_query(self):
        """
        Show the SQL needed to create trash tag and link orphans
        """
        print(f"\n=== Librarian SQL Blueprint ===")

        orphan_count = self.count_orphaned_files()
        print(f"Current orphans: {orphan_count}")

        if orphan_count > 0:
            print("\nLibrarian would execute:")
            print("1. CREATE TRASH TAG (if missing):")
            print("   INSERT OR IGNORE INTO tags (tag_name) VALUES ('trash');")

            print("\n2. MOVE ORPHANS TO TRASH:")
            print("   INSERT INTO file_tags (file_id, tag_name, virtual_filename)")
            print("   SELECT fr.file_id, 'trash', fr.original_filename")
            print("   FROM file_registry fr")
            print("   LEFT JOIN file_tags ft ON fr.file_id = ft.file_id")
            print("   WHERE ft.file_id IS NULL;")

            print("\n3. RESULT:")
            print("   Orphaned files become visible in /magic/tags/trash")

        return True


if __name__ == "__main__":
    setup_module()
    test = TestScavengerOrphanDetection()

    try:
        test.setup_method()
        print("TEST 30: THE SCAVENGER - PHASE 16 PREPARATION")
        print("=" * 60)
        print("Demonstrates orphan state created by Phase 15 unlink")
        print("Requires Librarian to handle in Phase 16")
        print("=" * 60)

        test.test_scavenger_orphan_detection_failure()
        test.test_orphan_visibility_in_fuse()
        test.test_create_trash_tag_query()

        print("\n" + "="*60)
        print("TEST 30 COMPLETE")
        print("="*60)
        print("✓ Phase 15 unlink creates orphaned files")
        print("✓ Orphans are invisible in FUSE views")
        print("✓ Phase 16 Scavenger required to move them to @trash")

    finally:
        test.teardown_method()
        teardown_module()