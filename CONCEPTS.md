cat <<EOF > CONCEPTS.md
# 🧠 MagicFS Concepts & Vision

> "The filesystem is the only API that has survived 50 years."

MagicFS is not just a search tool; it is an attempt to upgrade the operating system by injecting intelligence into the standard POSIX interface. This document outlines the core concepts driving its development.

---

## 1. The "Thin Client" (Local Knowledge Browser)

**The Vision:** An "Evernote-killer" that respects your resources.
Most knowledge apps (Obsidian, Evernote, Notion) are "Fat Clients" that bundle their own database, search engine, and file watchers into a massive Electron app.

MagicFS proposes a **Thin Client** that offloads all heavy lifting to the OS (via MagicFS) and focuses 100% on a blazing-fast, native User Interface.

### Architecture: Filesystem as API
The app treats the MagicFS mount point (\`/magic\`) as a REST API.

| Layer | Component | Responsibility | Technology |
| --- | --- | --- | --- |
| **Backend** | **MagicFS** | Indexing, OCR, Vector Search, File Watching. | Rust (FUSE) |
| **API** | **The Filesystem** | Exposing results as standard directories. | POSIX |
| **Frontend** | **The App** | Rendering, Navigation, Previews. | Rust (Iced/Tauri) |

**Why This Wins:** Zero lock-in (files remain files), format agnosticism, and extremely low memory usage (<50MB RAM).

---

## 2. The "Oracle" File (Generative Read)

**The Concept:** Scriptable Intelligence via \`cat\`.
Currently, we search for files that *exist*. The Oracle File allows us to ask questions and receive generated answers as if they were files.

* **User Action:** \`cat ~/MagicFS/ask/"What is the total budget?"/answer.txt\`
* **MagicFS Logic:**
    1.  Performs vector search for "budget".
    2.  Feeds the top 5 chunks into a local LLM (e.g., Llama 3).
    3.  Streams the LLM's answer back as the file content.

**Impact:** You can script intelligence into standard shell pipelines.
\`cat ~/MagicFS/ask/"summary of last meeting"/answer.txt >> meeting_notes.md\`

---

## 3. The "Black Hole" Inbox (Auto-Organization)

**The Concept:** A self-sorting library.
Users shouldn't have to manually file receipts, invoices, or contracts.

* **User Action:** Drag \`scan_001.pdf\` into \`~/MagicFS/inbox/\`.
* **MagicFS Logic:**
    1.  Extracts text and identifies the document type (e.g., "Receipt").
    2.  Identifies the date (e.g., "Jan 1, 2026").
    3.  **Moves** the file to \`~/me/Financial/Receipts/2026/Jan/\` and renames it \`2026-01-01_Receipt.pdf\`.

**Impact:** The filesystem organizes itself based on *meaning*, not just user effort.

---

## 4. "Virtual Joins" (The Semantic Graph)

**The Concept:** A filesystem that understands relationships.
In a standard FS, a project folder is isolated from an invoice folder.

* **User Action:** Open \`~/MagicFS/mirror/me/Projects/Apollo/@Related/\`.
* **MagicFS Logic:**
    1.  Analyzes the content of the "Apollo" folder.
    2.  Finds semantically related files elsewhere (e.g., an invoice in \`~/Finance\`, an email in \`~/Downloads\`).
    3.  Populates \`@Related\` with symlinks to those files.

**Impact:** You stop caring *where* files are stored and only care *what* they are related to. It turns the strict hierarchy of directories into a flexible graph.

---

## 5. "The Lens" (The Universal HUD)

**The Concept:** Alfred/Raycast for your data (The "Slim Client" Implementation).
A persistent, keyboard-driven HUD that sits above your windows, acting as the primary interface to MagicFS.

**The Tech:** Rust + Iced (Native GUI). No Electron. ~15MB binary. 0ms startup.

* **Workflow:**
    1.  **Trigger:** \`Ctrl+Space\`.
    2.  **Search:** Type query. Behind the scenes, it \`mkdir\`s in \`/magic/search\`.
    3.  **Preview:** It \`read()\`s the first 1KB of the selected file from MagicFS to show an instant text preview.
    4.  **Action:** \`Enter\` to open in default editor.

**The "Snappy" Factor:**
* **Passive Context:** If you are coding in VS Code, The Lens automatically suggests files related to your active project.
* **Instant Preview:** It relies on MagicFS's "Passthrough Reading" to preview PDFs/Docs without needing its own parsers.

**Why it Wins:** It transforms the OS into a "Bicycle for the Mind." You don't open a file manager; you just summon your data.
EOF
