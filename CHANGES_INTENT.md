# Phase 41: The Lazy Reaper (Completed)

## Status: ✅ Complete

## Objective
Implement "Verify at the Point of View" logic to automatically purge non-existent files ("Ghosts") from the database when they are accessed or listed.

## Changes Delivered
1.  **HollowDrive Update:** `readdir` now performs `std::path::Path::exists()` checks on all candidate files.
2.  **Auto-Purge:** Files failing the check are excluded from the listing and immediately deleted from `file_registry` and `file_tags` via `Repository`.
3.  **Test Suite Hardening:**
    * `test_42_lazy_reaper.py`: New test verifying ghost detection and cleanup.
    * `test_28_tag_moving.py`: Updated to use real files (mocking with non-existent paths triggers the Reaper).
    * `test_32_broken_links.py`: Fixed race condition in daemon restart logic.
    * Removed `pytest` dependencies from recent tests.

## Verification
All tests passed, including the new regression suite for the Reaper logic.
