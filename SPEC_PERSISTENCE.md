This is the correct move. A specification document bridges the gap between our high-level architectural chats and the nitty-gritty code implementation. It prevents "drift" when we (or others) start building.

I have created a comprehensive specification document. This is not just a database schema; it is the blueprint for the **Inode Router** architecture.

### 📄 New File: `SPEC_PERSISTENCE.md`

```markdown
# 🏛️ MagicFS Persistence Specification: The Inode Router

> "The Database IS the Filesystem."

This document defines the architecture for **Phase 12 (Organization)**. It introduces the **Inode Router**, a mechanism to support persistent "Saved Views" and "Tags" alongside ephemeral search queries without introducing state duplication or startup latency.

---

## 1. The Core Philosophy: "The Router"

MagicFS rejects the traditional pattern of "Load DB into RAM on boot." Instead, the `InodeStore` acts as a **Logic Gate** that routes requests to the correct storage backend based on the Inode ID.

### The Flow
When FUSE requests `lookup(inode_id)`:
1.  **The Check:** `InodeStore` inspects the `inode_id`.
2.  **The Routing:**
    * **Low IDs (< 2^63):** Routed to **RAM (Transient Store)**. Fast, ephemeral, wiped on reboot.
    * **High IDs (≥ 2^63):** Routed to **SQLite (Persistent Store)**. Durable, ACID-compliant, survives reboot.



---

## 2. The Address Space (Zoning)

We partition the 64-bit Inode space using the most significant bit (High Bit).

| Zone | Range | Backend | Purpose | Properties |
| :--- | :--- | :--- | :--- | :--- |
| **System** | `1 - 100` | Hardcoded | Root, `.magic`, `/search` | Static, Immutable. |
| **Transient** | `101 - 9.22e18` | `BTreeMap<u64, Inode>` | Active Searches, Mirror Cache | Fast, reset on boot. |
| **Persistent** | `> 9.22e18` | `sqlite3` | Saved Views, Tags, Pins | Durable, relational. |

**The Boundary:** `const PERSISTENT_OFFSET: u64 = 1 << 63;`

---

## 3. Database Schema

The persistent zone is backed by a single, hierarchical table in `index.db`.

### Table: `virtual_nodes`
Represents the file tree of `/magic/saved`.

```sql
CREATE TABLE virtual_nodes (
    -- The High-Bit Inode ID (Primary Key)
    inode_id INTEGER PRIMARY KEY,
    
    -- Hierarchy (Adjacency List)
    parent_id INTEGER NOT NULL,
    
    -- File Metadata
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('Container', 'SmartFolder')),
    
    -- The "Magic" Payload
    -- For 'Container': NULL
    -- For 'SmartFolder': The search query string (e.g., "tax invoices 2024")
    payload TEXT,
    
    created_at INTEGER DEFAULT (unixepoch()),
    updated_at INTEGER DEFAULT (unixepoch()),

    -- Constraints
    UNIQUE(parent_id, name), -- No duplicate names in a folder
    FOREIGN KEY(parent_id) REFERENCES virtual_nodes(inode_id) ON DELETE CASCADE
);

-- Index for fast directory listing
CREATE INDEX idx_parent ON virtual_nodes(parent_id);

```

### Initial Seed (The Root)

On DB initialization, we must ensure the "Saved" root exists.

```sql
-- The root of the persistent world. 
-- ID = PERSISTENT_OFFSET + 1
INSERT OR IGNORE INTO virtual_nodes (inode_id, parent_id, name, kind)
VALUES (9223372036854775809, 1, 'saved', 'Container');

```

---

## 4. Behavioral Contracts

### A. `lookup(parent, name)`

* **Transient Parent:** Check RAM map.
* **Persistent Parent:** Execute SQL:
```sql
SELECT inode_id, kind, payload FROM virtual_nodes 
WHERE parent_id = ? AND name = ?

```



### B. `readdir(inode)`

* **Transient Inode:** Iterate RAM children vector.
* **Persistent Inode:** Execute SQL:
```sql
SELECT inode_id, name, kind FROM virtual_nodes 
WHERE parent_id = ?

```



### C. `rename(old_parent, old_name, new_parent, new_name)` aka "The Migration"

This is the critical atomic handoff when a user drags a folder from `/search` to `/saved`.

**Logic:**

1. **Identify Source:**
* If Source is **Transient** (e.g., `/search/taxes`):
* Read the `query` from RAM.
* **Mint** a new Persistent ID (Increment Max ID from DB).
* **Insert** into `virtual_nodes` (Kind: 'SmartFolder', Payload: query).
* (Implicitly) The transient inode is dropped from RAM later by LRU/reboot.


* If Source is **Persistent** (Moving within `/saved`):
* Update `parent_id` and `name` in DB.




2. **Identify Destination:**
* Must be a **Persistent** folder (Kind: 'Container').
* If Destination is Transient, return `EXDEV` (Cross-device link not permitted).



---

## 5. Implementation Strategy

We will not rewrite `InodeStore` from scratch. We will implement the **Router Pattern** via extension.

```rust
// Pseudocode for the Router
impl InodeStore {
    pub fn get_inode(&self, id: u64) -> Result<Inode> {
        if id >= PERSISTENT_OFFSET {
            // Forward to DB Layer
            self.repo.get_persistent_inode(id)
        } else {
            // Handle active RAM layer
            Ok(self.ram_inodes.get(&id).cloned())
        }
    }
}

```

This ensures that the "Transient" logic (which is complex, involving the Bouncer and Lazy Loading) remains untouched, while the "Persistent" logic (which is simple CRUD) lives in a separate, clean module.

```

### Next Step
This spec is now the source of truth. Whenever you are ready to begin **Phase 12**, we simply implement the `virtual_nodes` table and hook up the Router logic in `InodeStore`.

Would you like to commit this spec to the repository so it's official?

```
