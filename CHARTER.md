# 📜 The MagicFS Charter & Manual

> "The filesystem is the interface."

MagicFS is a system primitive that turns **Chaos into API**. It aims to make semantic understanding of data—regardless of format—as native to the OS as `ls` or `cp`.

---

## 1. The Prime Directives

### ⏱️ The "Illusion of Physicality"
**The Drive must feel real.**
Users expect folders to exist immediately when they navigate to them.
* **The Ephemeral Promise:** `lookup()` must always return success (Directory) for potential search queries. Never return `EAGAIN` or make the OS wait during navigation.
* **The Smart Waiter:** `readdir()` must block (wait) until results are ready. Never show an empty folder unless the search actually returned 0 results. This ensures scripts (like `ls`) and GUIs (like Dolphin) see populated content.

### 🛡️ Fail Safe, Fail Small
**The filesystem is hostile.**
Users have 100GB logs, corrupted PDFs, and deep symlinks.
* **Rule:** A failure to process a single file must **never** crash the daemon.
* **Implementation:** Skip bad files, log the error, and move on. "Partial results are better than no filesystem."

### 🔒 Infinite Space is Read-Only
**You cannot build rooms in an infinite hotel.**
* **Rule:** The `/search` directory is **Read-Only (555)**.
* **Reason:** Prevents OS file managers from entering "Creation Loops" (trying to create "New Folder", "New Folder 1"...) when checking for collisions.
* **Separation:**
    * `/search`: Ephemeral, Read-Only, Infinite navigation.
    * `/saved` (Future): Concrete, Read-Write, Curated organization.

---

## 2. Architecture: Service-Oriented

MagicFS uses a single-process architecture composed of strictly isolated services, coordinated via specific signals and synchronization primitives.

```text
┌───────────────────────────── Process Boundary ──────────────────────────────┐
│                                                                             │
│  ┌───────────────┐        ┌─────────────────┐        ┌───────────────────────┐  │
│  │ Hollow Drive  │◄────►│   Inode Store    │◄─────┤       Orchestrator    │  │
│  │ (FUSE Interface)     │ (Shared State)   │        │        (Oracle.rs)    │  │
│  └───────┬───────┘        └────────┬────────┘        └───────────┬───────────┘  │
│          │                         │                             │              │
│          │ Signal:                 │ Condition Var:              │              │
│          │ .magic/refresh          │ SearchWaiter                │              │
│       Syscalls                     │ (Smart Waiter)              │              │
│          │                         │                             │              │
│          ▼                         ▼                             ▼              │
│   [The Bouncer]             [The Promise]                  [The Engine]         │
│  (Rejects .zip)           (Instant Lookup)               (Async Workers)        │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

### The Organs
1.  **Hollow Drive (`src/hollow_drive.rs`)**: The FUSE terminal.
    * **The Bouncer**: Active rejection of noise (`.zip`, `.hidden`) to prevent phantom searches.
    * **The Promise**: Instant success for navigation (`cd`), deferring work to `readdir`.
2.  **Inode Store (`src/core/inode_store.rs`)**: The In-Memory source of truth.
3.  **The Orchestrator (`src/oracle.rs`)**: Async event loop. Handles "Lockout/Tagout" safety to prevent race conditions.
4.  **The Engine**:
    * **Indexer**: Extraction, Chunking, Embedding.
    * **Searcher**: Query Embedding, DB Lookup.
5.  **The Librarian (`src/librarian.rs`)**: The background watcher. Handles thermal protection (debouncing) and manual overrides.

---

## 3. Maintenance & Testing

### 🔗 Version Control
* **Commit Frequently**: Small, atomic commits.
* **Sync**: Pull before starting work.

### 🧪 The Golden Rule
`tests/run_suite.sh` must pass before any merge.

```bash
# Run the full integration suite
tests/run_suite.sh
```

### 📂 Key Test Cases
| Test | Purpose |
|------|---------|
| `test_00_stress` | Startup Storm & Zombie Check |
| `test_09_memory_leak` | Stress tests the Smart Waiter (Blocking behavior) |
| `test_14_mirror` | Mirror Mode Navigation |
| `test_17_illusion` | Verifies Bouncer, Instant CD, and Blocking LS |
| `test_18_readonly` | Verifies `mkdir` is blocked in `/search` to prevent loops |

---
*Adopted: Jan 2026*
*Version: 4.0 (The Physical Illusion)*
