
# 💡 Concept: The "Thin Client" (Local Knowledge Browser)

**The Vision:** An "Evernote-killer" that respects your resources.
Most knowledge apps (Obsidian, Evernote, Notion) are bloated "Fat Clients." They bundle their own database, search engine, file watchers, and PDF parsers into a massive Electron app that eats 1GB of RAM.

This project proposes a **Thin Client** that does none of that. It offloads all the heavy lifting to the OS (specifically, to **MagicFS**) and focuses 100% on a blazing-fast, native User Interface.

## 1. The Architecture: Filesystem as API

The app treats the MagicFS mount point (`/magic`) as a REST API.

| Layer | Component | Responsibility | Technology |
| --- | --- | --- | --- |
| **Backend** | **MagicFS** | Indexing, OCR, Vector Search, File Watching. | Rust (FUSE) |
| **API** | **The Filesystem** | Exposing results as standard directories. | POSIX |
| **Frontend** | **The App** | Rendering, Navigation, Previews. | Rust (Iced/Tauri) |

**Key Advantage:** The App has **zero** indexing logic. It starts instantly because it just reads files.

## 2. Core Features & Implementation

### A. The "Instant Search" Bar

Instead of querying a SQL database, the app simply lists directories.

* **User Action:** Types "Tax Invoice 2024".
* **App Logic:** `ls "/magic/search/Tax Invoice 2024"`
* **Result:** Instant results populated by the MagicFS kernel thread.

### B. The "Context-Aware" Preview

Instead of parsing PDFs or highlighted text itself, the app relies on MagicFS's pre-computed context.

* **User Action:** Selects `invoice.pdf`.
* **App Logic:**
1. Reads `_CONTEXT.md` from the search result folder.
2. Renders the Markdown snippet: *"Total Amount Due: $500"*.
3. Highlights the relevant text in the UI.


* **Result:** You see *why* the file matched without opening the heavy PDF.

### C. The "Smart" Sidebar

Your sidebar isn't a static list of folders; it's a live view of your **Persistent Views** (from MagicFS Phase 8).

* **App Logic:** Reads `~/.config/magicfs/views.json`.
* **Display:**
* 📁 **Project Apollo** (Mapped to `/magic/saved/Apollo`)
* 📁 **Urgent** (Mapped to `/magic/saved/Urgent`)


* **Benefit:** If you change a folder in the terminal, the App updates instantly via filesystem events.

### D. "Drag-and-Drop" Import

* **User Action:** Drags a file into the App.
* **App Logic:** Copies the file to `~/MagicInbox/` (a standard folder watched by MagicFS).
* **Result:** MagicFS indexes it, and it appears in search results seconds later.

## 3. The Tech Stack (No Electron)

Since the "Business Logic" is entirely in MagicFS, the GUI can be incredibly lightweight.

* **Language:** Rust.
* **Framework:** **Iced** (Pure Rust, Native) or **Tauri** (System Webview, tiny footprint).
* **Memory Target:** < 50MB RAM.

## 4. Why This Wins

1. **Zero Lock-in:** Your data is just files. If you delete the app, you still have your files.
2. **Format Agnostic:** If MagicFS adds support for `.epub` or `.mobi` next year, the App supports it for free.
3. **Unix Philosophy:** It does one thing well (UI) and leaves the hard stuff (Indexing) to the tool designed for it.

---

*Drafted: Dec 2025*
