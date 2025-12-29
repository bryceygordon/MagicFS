# MagicFS Development Roadmap

## 🎯 Vision: The Universal Reader

We are building the "Context Layer" of the OS. The goal is to allow users to manipulate files based on *meaning*, not just location, regardless of whether the file is a text file, a PDF, or a Word doc.

---

## 📜 History
* **Phases 1-5 (Foundation)**: Basic FUSE, SQLite, FastEmbed.
* **Phase 6 (Hardening)**: "Three-Organ" Architecture, Chunking, Binary Safety.

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

### 🔮 Phase 8: Persistence & Workflows
**Goal:** Transform from "Search Tool" to "Organization System".

1.  **Write Support (The "Config Filesystem")**:
    * Allow `mkdir` in `/saved/`.
    * Allow writing to `.query` files to define folders.
2.  **Saved Views**:
    * Persist these folder definitions to `~/.magicfs/saved_views.db`.
    * *Scenario:* `mkdir /magic/saved/Tax2024` -> Auto-populates with all tax docs.

---

## 📏 Critical Constraints
1.  **Memory Cap**: ~500MB RAM. (Parsing PDFs can be heavy; we must stream or chunk aggressively).
2.  **Dependency Weight**: Avoid `libpoppler` if possible, but prioritize correctness for now.
