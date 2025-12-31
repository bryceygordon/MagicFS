# CLAUDE.md

## 🎯 Project Overview

**MagicFS** is a **Semantic Virtual Filesystem**. It exposes a FUSE interface where users can navigate to `/search/[query]` to see files relevant to that query.

**The Vision: "The Universal Text Interface"**
MagicFS abstracts away file formats. It indexes concepts regardless of whether they are locked in `.txt`, `.pdf`, or `.docx` files, exposing them as standard, scriptable file objects.

**Critical Constraint**: **The 10ms Law** - Every FUSE operation must complete in <10ms. Never block the FUSE loop.

## 🏗️ Architecture: Service-Oriented

MagicFS uses a single-process architecture composed of strictly isolated services:

```text
┌───────────────────────────── Process Boundary ──────────────────────────────┐
│                                                                             │
│  ┌───────────────┐        ┌─────────────────┐        ┌───────────────────────┐  │
│  │ Hollow Drive  │◄────►│  Inode Store    │◄─────┤       Orchestrator      │  │
│  │ (FUSE Interface)      │ (Shared State)  │        │       (Oracle.rs)       │  │
│  └───────┬───────┘        └─────────────────┘        └───────────┬───────────┘  │
│          │ Signal:                                               │              │
│          │ .magic/refresh ──────────────────┐           ┌────────▼────────┐     │
│          │                                  │           │      Engine     │     │
│      Syscalls                               │           │ (Async Workers) │     │
│                                             │           └─┬─────────────┬─┘     │
│                                             │             │             │       │
│                                    ┌────────▼────────┐    │Indexer      │Searcher│
│                                    │    Librarian    │    │(Retry Logic)│       │
│                                    │(Debounce/Watch) │    └────┬────────┴───┬───┘
│                                    └────────┬────────┘         │            │    
│                                             │                  │            │    
│                                    ┌─────▼─────────────▼────────┴────────────┐  │
│                                    │               Repository                │  │
│                                    │           (SQLite + Vector)             │  │
│                                    └─────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1. Hollow Drive (`src/hollow_drive.rs`)
* **Role**: The dumb FUSE terminal.
* **Rule**: NEVER blocks. Checks `InodeStore`. If data is missing, returns `EAGAIN`.
* **Features**:
    * **Manual Refresh**: Intercepts `touch .magic/refresh` and sets the `refresh_signal` atomic flag.

### 2. Inode Store (`src/core/inode_store.rs`)
* **Role**: The source of truth for "Virtual Files".
* **Data**: Maps `Query String <-> Inode` and holds `SearchResults`.

### 3. The Orchestrator (`src/oracle.rs`)
* **Role**: Event loop manager.
* **Safety Systems**:
    * **Lockout/Tagout**: Prevents race conditions between Delete/Create tasks.
    * **Arbitrator**: Verifies file existence before deletion to prevent "Ghost Deletes".
    * **Prioritization**: Prioritizes Indexing over Searching.

### 4. The Engine (`src/engine/`)
* **Indexer**: Handles file reading, chunking, and embeddings.
    * **Retry Logic**: Retries on `PermissionDenied` to survive rapid file locking.
* **Searcher**: Generates query embeddings, searches DB, updates `InodeStore`.

### 5. The Librarian (`src/librarian.rs`)
* **Role**: The background watcher.
* **Hardening**:
    * **Thermal Protection**: Debounces rapid updates (2s window) with "Final Promise" logic to prevent starvation.
    * **Manual Override**: Polls `refresh_signal` to trigger full scans for network/Docker volumes.

## 📂 Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point. Safety checks (Anti-Feedback Switch). |
| `src/hollow_drive.rs` | FUSE implementation + Refresh Signal. |
| `src/oracle.rs` | Async Orchestrator (Event Loop). |
| `src/librarian.rs` | Watcher, Debouncer, Refresh Logic. |
| `src/engine/indexer.rs` | Business Logic: Extraction, Retry, Chunking. |
| `src/storage/text_extraction.rs` | **Universal Reader**: 10MB limit, Binary detection. |

## 🔗 Version Control

* **Commit Frequently**: Small, atomic commits.
* **Message Format**: "Brief Summary" (newline) "Detailed description of changes".
* **Sync**: Pull before starting work.

## 🧪 Testing

**The Golden Rule**: `tests/run_suite.sh` must pass before any merge.

```bash
# Run the full integration suite
tests/run_suite.sh

# Run specific unit tests
cargo test
```

**Test Suite Coverage**:
| Test | Purpose |
|------|---------|
| `test_00_stress` | Startup Storm & Zombie Check |
| `test_01_indexing` | Dynamic Indexing |
| `test_02_dotfiles` | Ignore Rules |
| `test_03_search` | End-to-End Search |
| `test_04_hardening` | Binary/Large file rejection |
| `test_05_chunking` | Sliding Window & Semantic Dilution |
| `test_07_real_world` | Race Conditions & Permissions |
| `test_09_chatter` | Thermal Protection (Debounce) |
| `test_10_refresh` | Manual Override (`touch .magic/refresh`) |

## 📅 Roadmap Status

* **Phase 1-6**: Foundation & Hardening (Done)
    * *Achievements*: Race condition fixes, Chatter suppression, Manual Refresh.
* **Phase 7**: **The Universal Reader** (Active) - Support for PDF, DOCX extraction.
* **Phase 8**: **Persistence** (Planned) - Saved Views (`mkdir` in `/saved/`) and Workflows.
