# 🏛️ MagicFS Persistence & Semantic Graph Specification v2.0

> "The Directory is the Database View."

This document defines the architecture for the **Semantic Graph**. It moves MagicFS from a search tool to a hierarchical Knowledge Operating System (KOS), implementing a "Sidecar" architecture where the FUSE layer handles IO and an external Client handles rich metadata.

---

## 1. The Core Philosophy: "Option B" (Flat & Robust)

We adhere to **Strict Unix Semantics** backed by **Magic Logic**.
* **Files are Files:** Attachments live next to their notes. If a note links to `./image.png`, that image must physically exist or be tagged in the same context.
* **Folders are Views:** A directory in `/magic/tags` is a real-time SQL query materialized as a folder.
* **Copy is Copy:** `cp` creates physical bytes (Safety). `mv` updates metadata (Organization).

---

## 2. Database Schema (The Source of Truth)

The schema supports a "Multiverse" model where a single physical file can exist in multiple logical locations (Tags).

### 2.1 Table: `tags` (The Hierarchy)
This table defines the folder structure visible in `/magic/tags`.

| Column | Type | Nullable | Description |
| :--- | :--- | :--- | :--- |
| `tag_id` | `INTEGER PK` | NO | Unique ID. **Mapped to Inode High-Bit.** |
| `parent_tag_id` | `INTEGER FK` | YES | Self-referential FK. NULL = Root Tag. |
| `name` | `TEXT` | NO | The folder name. |
| `is_system` | `BOOLEAN` | NO | If TRUE, cannot be renamed/deleted (e.g. Inbox, Trash). |
| `color` | `TEXT` | YES | Hex code (e.g., `#FF0000`). Used by Sidecar UI. |
| `icon` | `TEXT` | YES | Emoji or Icon ID. Used by Sidecar UI. |

*Constraints:* `UNIQUE(parent_tag_id, name)` to prevent duplicate folders in the same level.

### 2.2 Table: `file_tags` (The Edges)
This table links physical files to specific nodes in the tag hierarchy.

| Column | Type | Nullable | Description |
| :--- | :--- | :--- | :--- |
| `file_id` | `INTEGER FK` | NO | Link to physical file (`file_registry`). |
| `tag_id` | `INTEGER FK` | NO | Link to the specific tag folder. |
| `display_name` | `TEXT` | NO | Virtual filename in this specific folder. |
| `added_at` | `INTEGER` | NO | Timestamp for sorting by "Date Added". |

*Constraints:* `PK(file_id, tag_id)` prevents a file appearing twice in the same folder (unless via alias, handled by `display_name`).

---

## 3. Inode Zoning (The Magic Numbers)

To ensure stability across system reboots and to distinguish between "Real" (Persistent) and "Ephemeral" (Search) content, we partition the 64-bit Inode space.

| Range / Bitmask | Zone Name | Persistence | Description |
| :--- | :--- | :--- | :--- |
| `1 - 100` | **System Reserve** | Static | Root, `.magic`, `search`, `mirror`. |
| `1 << 63` (High Bit) | **Persistent Zone** | Database | Mapped 1:1 with `tags.tag_id`. |
| `Everything Else` | **Ephemeral Zone** | Dynamic | Hash-based (Paths, Search Results). |

**The Translation Logic:**
* **DB -> FUSE:** `inode = tag_id | (1 << 63)`
* **FUSE -> DB:** `tag_id = inode & ~(1 << 63)`
* **Check:** `bool is_persistent = (inode >> 63) == 1`

---

## 4. Operational Semantics (The Unix Translation)

This section maps standard POSIX syscalls to Database transactions.

### 4.1 `mkdir(parent, name)` -> Organizing
* **Context:** `parent` is a Persistent Tag Inode.
* **Action:**
    1.  Decode `parent` to `parent_tag_id`.
    2.  `INSERT INTO tags (name, parent_tag_id) VALUES (?, ?)`
    3.  Compute new Inode from result `ROWID`.
* **Result:** A new subfolder appears immediately.

### 4.2 `mv(source, dest)` -> Refiling & Renaming
* **Case A: Rename (Same Directory)**
    * *SQL:* `UPDATE file_tags SET display_name = ? WHERE file_id = ? AND tag_id = ?`
    * *Effect:* File keeps its tags, just changes visual name.
* **Case B: Move (Different Directory)**
    * *SQL:*
        1.  `DELETE FROM file_tags WHERE file_id = ? AND tag_id = old_tag`
        2.  `INSERT INTO file_tags ... VALUES (file_id, new_tag, name)`
    * *Effect:* The file "moves" from one topic to another.
* **Case C: Reparenting a Folder**
    * *SQL:* `UPDATE tags SET parent_tag_id = new_parent WHERE tag_id = current_tag`
    * *Effect:* Moves an entire category subtree.

### 4.3 `cp(source, dest)` -> Duplication
* **Philosophy:** `cp` implies a physical copy of data.
* **Action:**
    1.  Read `source` bytes.
    2.  Create **NEW** physical file in `_imported`.
    3.  Register new file in `file_registry`.
    4.  Link new file to `dest` tag in `file_tags`.
* **Why?** Users expect `cp` to create an independent copy they can edit without changing the original. To "Link" (Multiverse), users should use the Sidecar UI or a CLI tool.

### 4.4 `rm(path)` -> Unlinking
* **Action:** `DELETE FROM file_tags WHERE ...`
* **Safety:** This **only** removes the link (the file vanishes from this folder).
* **Garbage Collection:** A background job (Librarian) checks for files with `COUNT(tags) == 0` and moves them to a physical `_trash` folder or marks them for deletion.

---

## 5. The Inbox Workflow (Landing Zone)

The "Inbox" is the entry point for unstructured data.

1.  **System Tag:** `tag_id=1` is reserved for "Inbox".
2.  **Import Logic:**
    * When a file is created via FUSE in `/magic/inbox/` OR copied there:
    * **Physical:** File is written to `~/[WatchDir]/_imported/`.
    * **Logical:** Linked to `tag_id=1`.
3.  **Processing:**
    * User drags file from `/magic/inbox/` to `/magic/tags/Receipts/`.
    * MagicFS executes **4.2 Case B (Move)**.
    * Result: File vanishes from Inbox, appears in Receipts.

---

## 6. The Sidecar Protocol (Metadata)

The FUSE filesystem is for **Data**. The Sidecar App is for **Context**.

* **Database Mode:** `WAL` (Write-Ahead Logging).
* **Permissions:** The SQLite file must be readable by the user running the GUI.
* **Responsibility:**
    * **MagicFS:** Handles file operations, hierarchy, and indexing.
    * **Sidecar App:** Queries `tags` table to render a colorful tree view. Queries `vec_index` for similarity visualizations.
* **Synchronization:** The Sidecar monitors `index.db` for changes (via SQLite `update_hook` or file polling) to refresh the UI when the filesystem changes.

---

## 7. Search Integration

Search results must respect the "Illusion of Physicality."

* **Search Context:** When searching inside `/magic/tags/Finance`, the search scope is implicitly limited to `tag_id=Finance` and its children.
* **Result Display:** Search results are **Virtual Files**.
    * They are read-only.
    * They are transient (exist only in the `search/` view).
    * Open/Read operations pass through to the real physical file.

---

*Adopted: Jan 2026*
*Version: 2.0 (The Persistence Layer)*
