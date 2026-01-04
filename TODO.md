# MagicFS Roadmap & Charter

## Little jobs up first
- [ ] lower the memory test from 2000 to 500.

> "The goal is not just to search; it is to maintain the illusion of a physical, infinite drive."

## 🏆 Completed Milestones
- [x] **Phase 1: The Foundation**
    - [x] FUSE Mount & Passthrough
    - [x] In-Memory Inode Store
    - [x] Async Orchestrator (The Oracle)
- [x] **Phase 10: The AI Engine**
    - [x] Nomic Embed v1.5 Integration (768 dims)
    - [x] Structure-Aware Chunking (No sentence splicing)

---

## 🚧 Phase 11: The Illusion of Physicality (Current Focus)
**Goal:** MagicFS must behave exactly like a dumb USB drive. It must never "load," never error, and never ask for permissions. It must lie efficiently.

### 1. The Ephemeral Container (Fixing Phantom Searches)
* **Problem:** Right-clicking creates `search.zip` because the OS probes for file existence.
* **Solution:** Implement "The Ephemeral Promise."
    * `lookup()` always returns `OK` + `FileType::Directory` (never `EAGAIN`).
    * `lookup()` does *not* persist the search or trigger the AI.
    * **The Trigger:** Only persist the search and fire the Oracle when `readdir()` (Enter Folder) is called.
    * **Effect:** Machine probes (which expect files) hit a "Directory" wall and stop. Human navigation (which enters folders) triggers the search.

### 2. The Empty Room Promise (Fixing Admin Password Errors)
* **Problem:** Searching `/search/foo` returns `EAGAIN` while calculating, causing OS to show "Access Denied."
* **Solution:** "Lie First, Fix Later."
    * `readdir()` immediately returns an empty list `[]` (Success) instead of blocking or erroring.
    * **Background:** The Oracle runs the embedding search.
    * **Refresh:** Once results are ready, send a kernel invalidation signal (or rely on user refresh) to populate the files.

### 3. The Typewriter Fix (Typing between slashes)
* **Problem:** Editing a path `/search/foo/` triggers searches for `f`, `fo`, `foo`.
* **Solution:** Solved implicitly by **The Ephemeral Container**.
    * Intermediate keystrokes create phantom directories that are never entered (`readdir` never called).
    * Only the final directory that the user commits to (by pressing Enter/opening) gets triggered.

---

## 🧠 Phase 12: Organization & Persistence (The "Second Brain")
**Goal:** Moving from "Search" to "Curated Knowledge."

### 1. Saved Views (Aliasing)
* **Concept:** Map complex queries to simple folder names.
* **Example:** `mv "/search/tax invoice 2024"` `"/magic/saved/2024_Taxes"`
* **Tech:** A persistent `alias` map in SQLite.

### 2. The Tagging Filesystem
* **Concept:** Directories as Tags.
* **Workflow:** Copying a file into `/magic/tags/urgent/` doesn't duplicate bytes; it adds the "urgent" vector/tag to the file's metadata.

### 3. Portable Brain (Config)
* **Concept:** `config.yaml` that makes the setup reproducible across machines.

---

## 🛠️ Phase 13: Developer Experience & Stability
**Goal:** Solidify the binary for daily driving.

- [ ] **Daemonize Properly:** Ensure `systemd` service handles suspend/wake cycles correctly.
- [ ] **The "Nuke" Protocol:** A robust way to clear the cache when the AI model changes (Version pinning).
- [ ] **Log Rotation:** Prevent `magicfs.log` from eating the disk during long runs.
