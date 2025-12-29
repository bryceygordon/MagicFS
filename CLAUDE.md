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
│  ┌───────────────┐       ┌─────────────────┐       ┌───────────────────────┐  │
│  │ Hollow Drive  │◄────►│  Inode Store    │◄─────┤       Orchestrator      │  │
│  │ (FUSE Interface)      │ (Shared State)  │       │       (Oracle.rs)       │  │
│  └───────────────┘       └─────────────────┘       └───────────┬───────────┘  │
│          ▲                                                       │             │
│          │                                                ┌────────▼────────┐      │
│        Syscalls                                           │      Engine     │      │
│                                                           │ (Async Workers) │      │
│                                                           └─┬─────────────┬─┘      │
│                                                             │             │        │
│                                                      ┌────▼────┐   ┌────▼────┐  │
│                                                      │ Indexer │   │ Searcher│  │
│                                                      └────┬────┘   └────┬────┘  │
│                                                           │             │        │
│                                                      ┌─────▼─────────────▼────┐  │
│                                                      │        Repository        │  │
│                                                      │    (SQLite + Vec)        │  │
│                                                      └────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1. Hollow Drive (`src/hollow_drive.rs`)
* **Role**: The dumb FUSE terminal.
* **Rule**: NEVER blocks. Checks `InodeStore`. If data is missing, returns `EAGAIN`.

### 2. Inode Store (`src/core/inode_store.rs`)
* **Role**: The source of truth for "Virtual Files".
* **Data**: Maps `Query String <-> Inode` and holds `SearchResults`.

### 3. The Orchestrator (`src/oracle.rs`)
* **Role**: Event loop manager.
* **Job**:
    * Receives file events from `Librarian`.
    * Dispatches tasks to `Indexer`.
    * Checks `InodeStore` for pending searches -> Dispatches to `Searcher`.

### 4. The Engine (`src/engine/`)
* **Indexer**: Handles file reading, **Format Conversion (PDF/DOCX)**, chunking text, generating embeddings, and DB writes.
* **Searcher**: Generates query embeddings, searches DB, updates `InodeStore`.

### 5. The Librarian (`src/librarian.rs`)
* **Role**: The background watcher.
* **Job**: Monitors physical directories for changes and queues them for the Orchestrator.

## 📂 Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point. Initializes all services. |
| `src/hollow_drive.rs` | FUSE implementation. |
| `src/oracle.rs` | Async Orchestrator (Event Loop). |
| `src/engine/indexer.rs` | Business Logic: File -> **Text (Rich Media)** -> Chunks -> DB. |
| `src/engine/searcher.rs` | Business Logic: Query -> Embedding -> DB -> InodeStore. |
| `src/core/inode_store.rs` | Shared state for VFS consistency. |
| `src/storage/repository.rs` | Centralized SQL logic. |
| `src/storage/text_extraction.rs` | **Universal Reader**: Handles PDF, DOCX, and Text parsing. |
| `src/librarian.rs` | File watcher integration (notify crate). |

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
| `test_01_indexing` | Dynamic Indexing |
| `test_02_dotfiles` | Ignore Rules |
| `test_03_search` | End-to-End Search |
| `test_04_hardening` | Binary/Large file rejection |
| `test_05_chunking` | Sliding Window & Semantic Dilution |
| `test_06_rich_media` | **(Active)** PDF/DOCX indexing |

## 📅 Roadmap Status

* **Phase 1-5**: Foundation (Done)
* **Phase 6**: Architecture Refactor & Hardening (Done)
* **Phase 7**: **The Universal Reader** (Active) - Support for PDF, DOCX extraction.
* **Phase 8**: **Persistence** (Planned) - Saved Views (`mkdir` in `/saved/`) and Workflows.
