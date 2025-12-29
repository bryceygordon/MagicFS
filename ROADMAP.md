# MagicFS Development Roadmap

## 🎯 Vision: The Universal Context Layer

MagicFS is not just a search tool; it is the "Context Layer" of the OS. Its goal is to allow users to manipulate files based on *meaning* rather than location, and to aggregate scattered "Data Islands" (Obsidian, GDrive, ~/Documents) into a single, unified interface.

It operates on the philosophy that **"The Filesystem is the Interface."** Users configure the system using standard tools (`ln`, `mkdir`, `cp`) rather than config files or GUIs.

---

## 📜 History

* **Phases 1-5 (Foundation)**: Basic FUSE loop, SQLite storage, FastEmbed integration.
* **Phase 6 (Hardening)**: "Three-Organ" Architecture, Chunking, Binary Safety, 10MB limits.

---

## 🏗️ Era 2: Scalability [ACTIVE]

### 🚧 Phase 6.5: The Foundation (Stability & Scalability) [CURRENT]
**Goal:** Fix structural flaws that prevent scaling to 10,000+ files. Ensure the system survives restarts and long uptimes without burning CPU or RAM.

1.  **The "Startup Storm" (Incremental Indexing)**:
    * **Problem:** Currently, MagicFS re-indexes *every* file on startup.
    * **Solution:** Implement `mtime` comparison. Only re-embed if the file has changed since the last DB write.
2.  **The "Zombie File" (State Consistency)**:
    * **Problem:** Files deleted or added to `.magicfsignore` stay in the index forever.
    * **Solution:** Implement a "Purge" routine on startup and during ignore-file updates to remove orphaned entries.
3.  **The "Memory Leak" (LRU Eviction)**:
    * **Problem:** `InodeStore` grows indefinitely.
    * **Solution:** Implement an LRU (Least Recently Used) Cache for search inodes. Cap at ~100 active queries.
4.  **The Stress Test**:
    * **Metric:** System must handle a repository of 1,000 files with <500MB RAM usage and <5s startup time.

---

## 🛡️ Era 3: Utility [PLANNED]

### 🔮 Phase 7: The Universal Reader
**Goal:** Break the "Format Barrier". Users store high-value knowledge in PDFs and DOCX, not just `.txt`. MagicFS must read them all.

1.  **Rich Media Ingestion**: PDF and Office Document support.
2.  **Contextual Visibility (`_CONTEXT.md`)**: Exposing *why* a file matched.

### 🔮 Phase 8: Aggregation & Persistence
**Goal:** Transform from "Single Folder Watcher" to "System-Wide Aggregator".

1.  **Multi-Root Support**: `/sources` directory for symlinking external paths.
2.  **Saved Views**: Persistent queries via `/saved` directory.
3.  **XDG Compliance**: Separate config (`~/.config`) from cache (`~/.cache`).

---

## 📏 Critical Constraints

1.  **The 10ms Law**: FUSE ops must never block >10ms.
2.  **The 500MB Cap**: RAM usage must remain stable regardless of file count.
3.  **Zero Config**: Configuration via filesystem operations only.
