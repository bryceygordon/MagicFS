================================================
FILE: ROADMAP.md
================================================
# MagicFS Development Roadmap

## 🎯 Vision: The Universal Context Layer

MagicFS is not just a search tool; it is the "Context Layer" of the OS. Its goal is to allow users to manipulate files based on *meaning* rather than location, and to aggregate scattered "Data Islands" (Obsidian, GDrive, ~/Documents) into a single, unified interface.

It operates on the philosophy that **"The Filesystem is the Interface."** Users configure the system using standard tools (`ln`, `mkdir`, `cp`) rather than config files or GUIs.

---

## 📜 History

* **Phases 1-5 (Foundation)**: Basic FUSE loop, SQLite storage, FastEmbed integration.
* **Phase 6 (Hardening)**: "Three-Organ" Architecture, Chunking, Binary Safety, 10MB limits.
* **Phase 6.5 (Scalability)**: LRU Caching, Incremental Indexing, Zero-byte retry logic, Stress Testing.

---

## 🏗️ Era 2: Scalability & Content [ACTIVE]

### 🚧 Phase 7: The Universal Reader [ACTIVE]
**Goal:** Break the "Format Barrier". Users store high-value knowledge in PDFs and DOCX, not just `.txt`. MagicFS must read them all.

1.  **Rich Media Ingestion**: PDF and Office Document support.
2.  **Contextual Visibility (`_CONTEXT.md`)**: Exposing *why* a file matched.

### 🔮 Phase 8: Aggregation & Persistence [PLANNED]
**Goal:** Transform from "Single Folder Watcher" to "System-Wide Aggregator".

1.  **Multi-Root Support**: `/sources` directory for symlinking external paths.
2.  **Saved Views**: Persistent queries via `/saved` directory.
3.  **XDG Compliance**: Separate config (`~/.config`) from cache (`~/.cache`).

---

## 🛡️ Era 3: Utility [PLANNED]

### 🔮 Phase 9: The "Thin Client"
**Goal:** A lightweight GUI that relies entirely on MagicFS for logic.
* **Instant Search**: `ls /magic/search/...`
* **Smart Sidebar**: Mapped to `/magic/saved/`

---

## 📏 Critical Constraints

1.  **The 10ms Law**: FUSE ops must never block >10ms.
2.  **The 500MB Cap**: RAM usage must remain stable regardless of file count (Enforced via LRU).
3.  **Zero Config**: Configuration via filesystem operations only.
