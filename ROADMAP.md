# MagicFS Development Roadmap

## 🎯 Vision: The Universal Context Layer

MagicFS is not just a search tool; it is the "Context Layer" of the OS. Its goal is to allow users to manipulate files based on *meaning* rather than location, and to aggregate scattered "Data Islands" (Obsidian, GDrive, ~/Documents) into a single, unified interface.

It operates on the philosophy that **"The Filesystem is the Interface."** Users configure the system using standard tools (`ln`, `mkdir`, `cp`) rather than config files or GUIs.

---

## 📜 History

* **Phases 1-5 (Foundation)**: Basic FUSE loop, SQLite storage, FastEmbed integration.
* **Phase 6 (Hardening)**: "Three-Organ" Architecture, Chunking, Binary Safety, 10MB limits.

---

## 🛡️ Era 3: Utility [ACTIVE]

### 🔮 Phase 7: The Universal Reader (Current Priority)
**Goal:** Break the "Format Barrier". Users store high-value knowledge in PDFs and DOCX, not just `.txt`. MagicFS must read them all.

1.  **Rich Media Ingestion**:
    * Integrate `pdf-extract` for PDFs.
    * Integrate `dotext` for Office Documents (DOCX/XLSX).
    * *Success Metric:* `test_06_rich_media.py` finds a "needle" text inside a PDF file.
2.  **Contextual Visibility (`_CONTEXT.md`)**:
    * **Problem:** Semantic search finds *concepts*, not exact keywords, making Ctrl+F useless.
    * **Solution:** Every search directory contains a generated `_CONTEXT.md` file.
    * **Function:** This file aggregates the specific text snippets (sentences/paragraphs) that triggered the match, allowing users to verify relevance instantly.

### 🔮 Phase 8: Aggregation & Persistence (Next Up)
**Goal:** Transform from "Single Folder Watcher" to "System-Wide Aggregator".

1.  **Multi-Root Support (The "Sources" Directory)**:
    * **Feature:** Expose a virtual directory `/sources`.
    * **Interface:** Users add watch paths via standard symlink: `ln -s ~/Obsidian /mountpoint/sources/notes`.
    * **Backend:** Librarian upgrades to handle multiple, dynamic `notify` watchers.
2.  **Saved Views (Workflows)**:
    * **Feature:** Expose a virtual directory `/saved`.
    * **Interface:** Users create persistent smart folders via mkdir: `mkdir /mountpoint/saved/ProjectApollo`.
    * **Configuration:** Users define the query by writing to a hidden file: `echo "apollo specs" > /mountpoint/saved/ProjectApollo/.query`.
3.  **State vs. Cache Separation (Backup Strategy)**:
    * **State (Precious)**: `~/.config/magicfs/` stores `sources.json` and `views.json`. This is small and must be backed up.
    * **Cache (Disposable)**: `~/.cache/magicfs/` stores `index.db`. This is heavy and can be rebuilt if lost.

---

## 📏 Critical Constraints

1.  **The 10ms Law**: FUSE ops must never block >10ms.
2.  **Memory Cap**: ~500MB RAM. (Parsing PDFs is memory-intensive; we must stream or chunk aggressively).
3.  **Dependency Weight**: Avoid `libpoppler` if possible, but prioritize correctness for Phase 7.
4.  **Zero Config**: Adding a source happens via the filesystem (`ln -s`), not a YAML file.
