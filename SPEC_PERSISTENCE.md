# 🏛️ MagicFS Persistence & Semantic Graph Specification

> "The Database IS the Filesystem."

This document defines the architecture for the **Semantic Graph**, moving beyond flat tags to a robust, persistent hierarchy that supports both structured organization and dynamic filtering.

---

## 1. Core Concept: The "Dual-Nature" Directory

Every directory under `/magic/tags` operates in two simultaneous modes. The filesystem must seamlessly merge these two views into a single user experience.

### A. The Light Tree (Explicit Structure)
* **Definition:** Persistent, user-created directory relationships.
* **Storage:** Stored in the SQLite `tags` table via `parent_tag_id`.
* **Behavior:** Acts like a standard folder.
    * `mkdir /tags/Finance/2024` -> Persists `2024` as a child of `Finance`.
    * `ls /tags/Finance` -> Lists `2024`.

### B. The Dark Graph (Implicit Filter)
* **Definition:** Ad-hoc intersection queries generated on the fly.
* **Storage:** Ephemeral (Computed Inodes).
* **Behavior:** Acts as a boolean `AND` filter.
    * `cd /tags/Finance/Urgent` (Where 'Urgent' is NOT a child).
    * **Logic:** "Show me files that are in 'Finance' AND 'Urgent'".
    * **Visibility:** These folders are **hidden** from `readdir` (to prevent infinite recursion tools like `find` from exploding) but are **accessible** via `lookup` (explicit navigation).

---

## 2. Database Schema (Source of Truth)

No changes to the schema structure are required, but the *interpretation* of the data changes.

### Table: `tags`
| Column | Type | Purpose |
| :--- | :--- | :--- |
| `tag_id` | `INTEGER PK` | Unique ID (High-Bit Inode Source). |
| `parent_tag_id` | `INTEGER FK` | **The Hierarchy Link.** If NULL, tag is a Root Tag. |
| `name` | `TEXT` | Display name. Unique per `parent_tag_id`. |

### Table: `file_tags`
| Column | Type | Purpose |
| :--- | :--- | :--- |
| `file_id` | `INTEGER FK` | Link to physical file. |
| `tag_id` | `INTEGER FK` | Link to *specific* tag node. |
| `display_name` | `TEXT` | Virtual filename within this tag context. |

---

## 3. Read Operations (The View Layer)

### A. `readdir(parent_inode)`
This function must merge three distinct data sources into a single file list.

1.  **Persistent Sub-Tags (The Light Tree):**
    * *Query:* `SELECT tag_id, name FROM tags WHERE parent_tag_id = ?`
    * *Type:* Directory.
    * *Inode:* Persistent (`tag_id | PERSISTENT_FLAG`).

2.  **Direct Files:**
    * *Query:* `SELECT f.inode, ft.display_name FROM file_tags ft... WHERE ft.tag_id = ?`
    * *Type:* File.
    * *Inode:* Persistent Physical Inode.
    * *Note:* Implement "Virtual Aliasing" for duplicate display names here.

3.  **Lexical Guard:**
    * Do **NOT** list "Dark Graph" candidates. `ls /tags/Finance` should NOT list every other tag in the universe. It should only list explicit children and files.

### B. `lookup(parent_inode, name)`
This is the "Router". It determines what `name` resolves to.

**Priority Order:**
1.  **Is `name` a Child Tag?** (Light Tree)
    * Check DB: `SELECT tag_id FROM tags WHERE parent_tag_id = parent AND name = name`.
    * If found: Return `Persistent Inode`.

2.  **Is `name` a File?**
    * Check DB: `SELECT file_id FROM file_tags WHERE tag_id = parent AND display_name = name`.
    * If found: Return `Physical Inode`.

3.  **Is `name` a Global Tag?** (The Dark Graph)
    * *Condition:* Only if `parent` is a Tag Inode.
    * Check DB: `SELECT tag_id FROM tags WHERE name = name`.
    * *Lexical Guard:* To prevent loops (`/A/B/A`), ONLY allow this if `name > parent_name` (Alphabetical order), OR if we implement a cycle-detection cache.
    * *Action:* If valid, generate an **Ephemeral Intersection Inode**.
        * `Inode = Hash(PathString)`
        * Store context in RAM: `InodeMap[Hash] = { tags: [ParentTags + NewTag] }`.

---

## 4. Write Operations (The Structure Layer)

### A. `mkdir(parent, name)` -> Creating Structure
* **Context:** `parent` is a Persistent Tag.
* **Logic:**
    1.  `INSERT INTO tags (name, parent_tag_id) VALUES (name, parent_id)`.
    2.  Return `new_tag_id | PERSISTENT_FLAG`.
* **UX:** This permanently creates a sub-folder.

### B. `rename(source, dest)` -> Moving Structure or Files
* **Case 1: File Retagging** (Existing Implementation)
    * Moving a file between tags changes its `file_tags` association.
* **Case 2: Tag Reparenting** (New)
    * Moving a directory (`mv /tags/A /tags/B/A`).
    * Logic: `UPDATE tags SET parent_tag_id = B_id WHERE tag_id = A_id`.
    * *Constraint:* Prevent circular parenthood (A cannot be child of A).

### C. `rmdir(parent, name)` -> Pruning
* **Logic:**
    * If directory is empty (no files, no sub-tags): `DELETE FROM tags`.
    * If not empty: Return `ENOTEMPTY` (Standard POSIX safety).

### D. `create(parent, name)` -> Import
* **Logic:** (Existing "Landing Zone" Pattern).
    * Create physical file in `_imported`.
    * Link to `parent` tag ID.

---

## 5. Semantic Relevance (Search Tuning)

To ensure the "Search" view feels intelligent, we must elevate the importance of filenames and explicit structural tags over raw text content.

* **Mechanism:** Update the `Indexer` payload generation.
* **Current Payload:** `"{content}"`
* **New Payload:**
    ```text
    Filename: {filename}
    Tags: {parent_tag_name} {grandparent_tag_name}
    ---
    {content}
    ```
* **Effect:** Vector embedding will now strongly encode the file's name and semantic location, making `/search/invoice` highly likely to return files named "Invoice" even if the word appears rarely in the text.
