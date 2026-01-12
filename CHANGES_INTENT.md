# Phase 41: The Lazy Reaper (Self-Healing Views)

## Objective
Implement "Verify at the Point of View" logic to automatically purge non-existent files ("Ghosts") from the database when they are accessed or listed.

## Problem
Database entries occasionally persist after physical files are deleted (e.g., race conditions, missed watcher events, or transient files like `.part` that vanished). This causes `ls` to show files that cannot be read.

## Solution
Modify `HollowDrive::readdir()` (specifically for Tag Views) to:
1.  Iterate through the candidate files returned by the database.
2.  Perform a physical existence check (`std::path::Path::new(&path).exists()`) for each.
3.  **If the file is missing:**
    * Exclude it from the FUSE directory listing (it vanishes instantly).
    * Add its `file_id` to a `ghosts` list.
    * After the query iteration finishes, execute `Repository::delete_file_by_id` for all detected ghosts to permanently clean the database.

## Verification
* **Test:** `tests/cases/test_42_lazy_reaper.py` (Current status: Failing ❌)
* **Expected:** `ls` should not show `phantom_file.txt`, and subsequent DB checks should show 0 records.
