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

### 6. The Zombie Mount (Testing Hazard)
* **Why:** Killing the FUSE process (`pkill`) leaves the mount point locked by the kernel ("Transport endpoint is not connected") for several seconds.
* **Decision:** Test harnesses must explicitly force-unmount (`umount -l`) before attempting to restart the daemon.

### 6. The "Self-DDOS" (Concurrency Limits)
* **Why:** Allowing 8 concurrent vector searches saturated the SQLite connection ("Database Locked"). This starved the Indexer and the Test Runner, causing timeouts.
* **Decision:** Reduced `MAX_CONCURRENT_SEARCHERS` from 8 to 2. We prioritize *responsiveness* over raw throughput.

### 7. Time Inversion (Queue Logic)
* **Why:** When a file was locked (Lockout/Tagout), we pushed its ticket to the *back* of the queue. If a "Create" was locked and a "Delete" arrived later, we processed "Delete" then "Create", inverting reality.
* **Decision:** Locked tickets are now **prepended** (pushed to the front) to strictly preserve FIFO causality.

### 8. Test Suite Isolation (State Leakage)
* **Why:** Running tests sequentially without restarting the daemon caused "Ghost Queries" from Test A to clog the queue during Test B.
* **Decision:** The test runner must enforce a **Clean Slate Protocol**: Kill Daemon -> Force Unmount -> Wipe DB -> Start Daemon between *every* test case.

### 9. The Permission Race (Retry on Lock)
* **Why:** Rapidly deleting and recreating a file (The "Reincarnation Race") often leaves the file locked or permissions flushing when the Indexer tries to read it.
* **Bug:** Indexer treated `PermissionDenied` as "Skip this file forever," causing data loss because the Lockout system thought the job was done.
* **Decision:** Treat `PermissionDenied` as a **Transient Error** (like 0-byte reads). Retry for up to 2 seconds before giving up.

### 10. Thermal Protection (The Chatterbox Problem)
* **Why:** A single file updating 50x/second (e.g., logs) can flood the event queue, starving legitimate indexing requests for other files.
* **Decision:**
    * **Debounce:** Ignore updates for a file if processed < 2s ago.
    * **The Final Promise:** If we ignore an update, mark the file as "Pending". When the 2s timer expires, fire a synthetic event. This guarantees the *final* state is indexed even if 99 intermediate states were dropped.

### 9. The "Ignore Paradox" (Event Ordering)
* **Why:** We added `.magic` to the Ignore List to hide internal files from search results. However, the "Kick Button" (`.magic/refresh`) is *inside* that ignored directory.
* **Failure:** The Librarian ignored the refresh event because it checked "Is Ignored?" before "Is Trigger?".
* **Decision:** **Trigger Checks MUST happen before Ignore Checks.** High-priority control signals override passive ignore rules.

### 10. The Watcher Race (Test Harness)
* **Why:** In tests, creating a directory and immediately creating a file inside it causes a race condition. The OS watcher takes a non-zero amount of time to attach to the new directory.
* **Decision:** When testing file events in new directories, **Create Dir -> Wait (1s) -> Create File**.

### 11. The "Blind Update" (Metadata Sync)
* **Why:** We tried to fix a corrupted file record by checking `mtime`. But changing a file's content doesn't always change `mtime` (e.g. rapid edits or metadata sabotage).
* **Decision:** The "Should Index?" check now verifies **BOTH** `mtime` and `size`. If either disagrees with the DB, we re-index.

### 12. Structure-Aware Chunking (The Dilution Fix)
* **Why:** Blind sliding windows (e.g. 256 chars) cut words in half ("Extr-" + "-act") and create "Micro-Sliding Loops" on dense text, generating thousands of redundant vectors.
* **Decision:** Replace sliding windows with **Recursive Character Splitting**.
    * **Priority:** Paragraphs (`\n\n`) -> Lines (`\n`) -> Words (` `).
    * **Impact:** Preserves semantic boundaries. Drastically reduces vector count while improving search relevance scores (>0.50).

### 13. UTF-8 Safety (The Panic Fix)
* **Why:** Rust strings are UTF-8. Slicing at arbitrary byte indices (e.g. `start + 150`) causes a panic if the cut lands inside a multi-byte char (e.g. emoji or symbols).
* **Decision:** Always use `text.is_char_boundary(index)` before slicing. If invalid, scan forward to the next valid boundary.

### 14. The Illusion of Physicality (Async UI)
* **Why:** Humans interact differently than machines. Humans `cd` (navigate) then `ls` (look). Machines probe specific files.
* **Problem:** If we wait for the DB on every `lookup`, `cd` feels sluggish. If we return empty on `readdir`, `ls` shows nothing.
* **Decision:**
    * **The Ephemeral Promise (Navigation):** `lookup` always returns `OK` immediately for plausible directories. Allows instant `cd`.
    * **The Smart Waiter (Consumption):** `readdir` blocks (waits) until the Oracle finishes or a timeout occurs. Ensures `ls` sees files.

### 15. Infinite Space is Read-Only (The Paradox)
* **Why:** In a virtual search space, "creating" a folder is a logical paradox.
* **Failure:** Dolphin enters an infinite loop trying to create "New Folder (1..N)" because it detects collisions but thinks it has permission.
* **Decision:** Virtual directories (Search Root, Query Results) must be **Read-Only (0o555)**. This disables the "Create New Folder" UI in file managers, preventing the loop.

### 16. Time Stability (The Nervous Twitch)
* **Why:** File managers cache directory contents based on `mtime`. If `mtime` updates on every access (SystemTime::now), the file manager invalidates its cache and rescans constantly (Infinite Loop).
* **Decision:** Virtual directories must report a **Stable Timestamp** (Daemon Start Time). This allows the OS to trust its cache and stop probing for hidden files.

### 17. The Hash Stability Law (FUSE Inodes)
* **Why:** Rust's `DefaultHasher` is randomized per process instance to prevent HashDoS attacks.
* **Failure:** If the daemon restarts, the hash of "file_A" changes. The OS (kernel) still holds the old inode number. When it calls `open(old_inode)`, the daemon calculates `hash(path) -> new_inode` and fails to match.
* **Decision:** ALWAYS use a deterministic hasher (e.g., **FNV-1a**) for generating inodes from file paths. FUSE inodes must be mathematically stable functions of their path.

### 18. The Metadata Precision Gap (Efficiency)
* **Why:** Linux filesystems store `mtime` with nanosecond precision. SQLite stores it as seconds (INTEGER).
* **Failure:** When checking `if fs_mtime > db_mtime`, a file modified at `100.5s` looks newer than the DB record of `100s`, causing infinite re-indexing loops on startup.
* **Decision:** The Librarian must allow a **1-second epsilon (drift)** when comparing timestamps. `abs(fs_mtime - db_mtime) > 1`.

### 19. The SQLite Permission Wall (WAL Mode)
* **Why:** When the daemon (running as `root`) enables WAL mode, it creates `-shm` and `-wal` shared memory files owned by `root`.
* **Failure:** User-level scripts (like our test suite) cannot query the database, even if the main `.db` file has wide permissions, because they cannot attach to the shared memory.
* **Decision:** Integration tests that need to inspect/modify the live database must use `sudo sqlite3` or run as the same user as the daemon.

### 20. The Landing Zone Pattern (Virtual Creation)
* **Why:** You cannot "write" bytes to a SQL query. When a user runs `cp file.txt /magic/tags/foo/`, the kernel needs a physical inode to write to.
* **Decision:** `create()` in a Tag View performs three atomic actions:
    1. Creates a physical file in `~/[WatchDir]/_imported/`.
    2. Registers the file in the `file_registry`.
    3. Links the file to the active Tag ID in `file_tags`.
* **Result:** The kernel gets a real inode, so subsequent `write()` calls pass through naturally to the physical disk.

### 21. The Two-Speed Engine (War Mode)
* **Context:** "We cannot afford fsync() on every file during the initial 10,000 file scan."
* **Decision:** Use `journal_mode=MEMORY` for the backlog, then strictly checkpoint and switch to `WAL` for steady state.
* **Implementation:**
    * **War Mode:** `PRAGMA synchronous = OFF; PRAGMA journal_mode = MEMORY;` for maximum throughput during initial bulk scan
    * **Handover:** `PRAGMA wal_checkpoint(TRUNCATE);` to flush memory journal to disk
    * **Peace Mode:** `PRAGMA synchronous = NORMAL; PRAGMA journal_mode = WAL;` for durability during steady-state monitoring
* **Key Constraint:** The Librarian state machine ensures strict handover - War Mode persists ONLY until the `files_to_index` queue is drained and Oracle is idle. This prevents data loss in steady-state operations.
