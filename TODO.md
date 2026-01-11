# 📝 MagicFS Roadmap & Charter

> "The goal is not just to search; it is to maintain the illusion of a physical, infinite drive."

## 🏆 Completed Milestones
- [x] **Phase 40: Identity & Ownership (Robin Hood Mode)**
    - [x] Identity Masquerade (`getattr` returns User UID/GID).
    - [x] Stable Inodes (FNV-1a hashing).
    - [x] Inbox Deletion (`unlink` support for system inbox).

---

## 🚧 Phase 41: System Isolation & Hygiene (Current Priority)
**Goal:** Decouple MagicFS from user watch directories and fix "Zombie" transient files.

### 1. The "Archive" Directory (Fixing `_moved_from_inbox`)
* **Problem:** Currently, files moved from Inbox -> Tag are physically moved to `[WatchDir]/_moved_from_inbox`. This pollutes the user's data and fails if no watch directory exists.
* **Fix:**
    * Create `~/.local/share/magicfs/archive/` (sibling to `inbox`).
    * Update `hollow_drive.rs::rename()`: When promoting from Inbox, move the physical file to `archive/` instead of `_moved_from_inbox`.
    * **Result:** MagicFS creates a self-contained "Data Lake" (Inbox + Archive) that works even with 0 watch directories.

### 2. The Transient Reaper (Fixing `.part` and `.lock` Zombies)
* **Problem:** `.part` (Firefox) and `.lock` (Kate) files are being indexed and persist in the DB even after deletion.
    * *Cause A:* `Librarian` might be indexing them before they are deleted.
    * *Cause B:* The `Remove` event for these files might be missed or filtered out incorrectly.
* **Fix:**
    * **Hard Filter in Indexer:** `Indexer::index_file` must actively reject files ending in `~`, `.part`, `.lock`, `.swp`, even if the Librarian passes them through.
    * **The Reaper Job:** On `readdir()`, if a file exists in the DB but `stat()` fails (ENOENT), immediately trigger a "Lazy Prune" to remove it from the view. This ensures "What you see is what exists."

### 3. Database Cleanup
* **Task:** Write a migration or startup check to purge existing `.part` and `.lock` entries from `file_registry`.

---

## 🔮 Future Horizons
- [ ] **Phase 42: The Lens (GUI)** - Native Rust UI for search.
- [ ] **Phase 43: OCR Integration** - Tesseract/Apple Vision for images.
