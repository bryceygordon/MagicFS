# MagicFS Development Roadmap

## 🎯 Vision: The Universal Context Layer

We are building the "Context Layer" of the OS. The goal is to allow users to manipulate files based on *meaning* rather than location, and to aggregate scattered "Data Islands" (Obsidian, GDrive, ~/Documents) into a single, unified interface.

---

## 📜 History

* **Phases 1-5 (Foundation)**: Basic FUSE loop, SQLite storage, FastEmbed integration.
* **Phase 6 (Hardening)**: "Three-Organ" Architecture, Chunking, Binary Safety, 10MB limits.

---

## 🛡️ Era 3: Utility [ACTIVE]

### 🔮 Phase 7: The Universal Reader (Current Priority)
**Goal:** Break the "Format Barrier". Users generally store high-value knowledge in PDFs and DOCX, not just `.txt`.

1.  **Rich Media Ingestion**:
    * Integrate `pdf-extract` for PDFs.
    * Integrate `dotext` for Office Documents (DOCX/XLSX).
    * *Success Metric:* `test_06_rich_media.py` finds a needle inside a PDF.
2.  **Contextual Visibility**:
    * Experiment: Virtual files (e.g., `file.pdf.summary`) that show *why* a match occurred.

### 🔮 Phase 8: Aggregation & Persistence (Next Up)
**Goal:** Transform from "Single Folder Watcher" to "System-Wide Aggregator".

1.  **Multi-Root Support (The "Sources" Directory)**:
    * **Feature**: Expose a virtual directory `/sources`.
    * **Workflow**: Users add watch paths via symlink: `ln -s ~/Obsidian /mountpoint/sources/notes`.
    * **Backend**: Librarian upgrades to handle multiple, dynamic `notify` watchers.
2.  **Saved Views (Workflows)**:
    * **Feature**: Expose a virtual directory `/saved`.
    * **Workflow**: Users create smart folders via mkdir: `mkdir /mountpoint/saved/ProjectApollo`.
3.  **State vs. Cache Separation (Backup Strategy)**:
    * **State (Precious)**: `~/.config/magicfs/` stores `sources.json` and `views.json`. This is small and must be backed up.
    * **Cache (Disposable)**: `~/.cache/magicfs/` stores `index.db`. This is heavy and can be rebuilt.

---

## 📏 Critical Constraints

1.  **The 10ms Law**: FUSE ops must never block >10ms.
2.  **Memory Cap**: ~500MB RAM. (Parsing PDFs can be heavy; we must stream or chunk aggressively).
3.  **Dependency Weight**: Avoid `libpoppler` if possible, but prioritize correctness for now.
4.  **Zero Config**: Adding a source happens via the filesystem (`ln -s`), not a YAML file.
