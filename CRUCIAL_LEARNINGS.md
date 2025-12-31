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
