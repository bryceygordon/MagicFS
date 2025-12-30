# 🏗️ Crucial Infrastructure & Learnings

### 1. The 10ms Law (HollowDrive)
* **Why:** FUSE blocks the OS. If `lookup()` takes >10ms, the terminal freezes.
* **Decision:** FUSE never touches the disk or runs AI models. It only reads from memory (`InodeStore`) or returns `EAGAIN` (Try Again).

### 2. Lockout / Tagout (The Foreman & Radio)
* **Why:** The Event Loop is blind. It fires tasks (rockets) and forgets them. A "Delete" task and "Create" task can race, destroying valid data.
* **Decision:**
    * **The Ledger:** Oracle tracks active files in a `HashSet`.
    * **The Rule:** If a file is in the Ledger, **no new workers** can be spawned for it.
    * **The Radio:** Workers must signal "Done" via a channel to remove the tag. This enforces strict serialization.

### 3. The Arbitrator (Reality Check)
* **Why:** "Delete" events might be outdated or lies (e.g., rapid move operations).
* **Decision:** Before deleting a record, the Indexer checks `Path::exists()`.
    * If file exists: **Abort Delete**. Re-index instead.
    * If file missing: **Proceed**.

### 4. Semantic Dilution (Chunking)
* **Why:** Small secrets (passwords) get lost in large files (logs). The "average" vector of a large file washes out the unique signal.
* **Decision:**
    * **Strict Limit:** 256 chars per chunk.
    * **Scoring:** We store *all* chunks, but search results use `MIN(distance)` (Best single chunk wins).

### 5. SQLite Configuration
* **Why:** Default SQLite locks the DB during writes, blocking the Searcher (Reader).
* **Decision:**
    * `journal_mode=WAL`: Readers don't block Writers.
    * `busy_timeout=5000`: Wait 5s for a lock instead of crashing immediately.
