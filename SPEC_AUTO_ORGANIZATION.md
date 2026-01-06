# 🧲 MagicFS Auto-Organization Specification

> "The folder pulls the file."

This document defines the **Magnetic Tag System**, a deterministic and transparent method for auto-organizing files based on vector similarity (Semantic Gravity) and pattern matching (Regex).

---

## 1. Core Concept: Semantic Gravity

Instead of an AI agent "deciding" where a file goes, we treat Tags as gravitational bodies.
* **The Mass:** The Tag's vector representation (Centroid).
* **The Pull:** The cosine similarity between a new file and the Tag.
* **The Snap:** If `Similarity > Threshold`, the file is linked to the Tag.

---

## 2. The Mechanisms

We employ three distinct strategies to determine where a file belongs.

### A. The Centroid (Learned Behavior)
* **Definition:** The mathematical average of all **confirmed** files currently in a tag.
* **Logic:** If you put 10 electric bills in `@Electricity`, the average vector of those 10 documents defines what `@Electricity` "looks like."
* **Update Cycle:** As files are added/removed manually, the Centroid recalculates. The tag "learns" from your manual habits.

### B. The Seed (Cold Start)
* **Definition:** A text description provided by the user when creating a tag.
* **Example:** Tag `@Invoices` -> Seed: *"Tax invoices, receipts, payment confirmations, accounts payable, total amount due"*.
* **Logic:** If a tag has no files (no Centroid), we use the vector of the Seed Description as the reference point.

### C. The Pattern (Deterministic Rules)
* **Definition:** Hard-coded or user-defined Regex patterns for specific tags.
* **Primary Use:** Dates (`2025`, `January`), Formats (`ID-XXXX`).
* **Logic:**
    * Scan first 1KB of text.
    * Match `/\b(20[2-3][0-9])\b/` -> Auto-tag `@Year/$1`.

---

## 3. The "Ghost Link" (Safety Valve)

To prevent the system from cluttering folders with "False Positives," auto-generated links are marked as **Tentative**.

### Schema Update
```sql
ALTER TABLE tags ADD COLUMN centroid BLOB;       -- Cache the 768-dim average vector
ALTER TABLE tags ADD COLUMN description TEXT;    -- The 'Seed' text
ALTER TABLE file_tags ADD COLUMN is_auto INTEGER DEFAULT 0; -- 0=Manual, 1=Ghost
```

### The State Machine

| State | `is_auto` | Description | Transition Trigger |
| :--- | :--- | :--- | :--- |
| **Ghost** | `1` | System proposed this link. Visually distinct in Sidecar UI (50% opacity). | Default on Auto-Tag. |
| **Confirmed** | `0` | User accepted this link. | User opens file, runs `touch`, or moves file. |
| **Rejected** | N/A | Link deleted. | User runs `rm` or `unlink`. |

---

## 4. The Workflow (Lifecycle Integration)

### Step 1: Ingestion (The Magnet Worker)
* **Trigger:** `Indexer` finishes processing a file.
* **Action:**
    1.  **Date Scan:** Run Regex. If "2026" found, Link to `@2026` (`is_auto=1`).
    2.  **Vector Scan:** Compare file vector against **ALL** Tag Centroids.
    3.  **Threshold Check:**
        * If `Score > 0.92` (High Confidence): Link (`is_auto=1`).
        * If `Score > 0.85` (Medium): Only link if Tag is empty (Seed match).

### Step 2: Presentation (The Illusion)
* **FUSE Layer:**
    * Ghost links appear as standard files in `readdir`.
    * *Option:* We could use Extended Attributes (`xattr`) to mark them so terminal power-users can distinguish them.
* **Sidecar UI:**
    * Renders Ghost files with a dashed border or different color.
    * "Approve All" button for the folder.

### Step 3: Confirmation (The Human Loop)
* **Implicit Confirmation:**
    * If a user **READS** (opens) a Ghost file, we assume it's correct. Update `is_auto=0`.
* **Explicit Rejection:**
    * User sees a "Cooking Recipe" in `@Invoices`.
    * User deletes it (`rm recipe.txt`).
    * **Logic:** `DELETE FROM file_tags` -> The file remains in `@Recipes` (its correct home) but vanishes from the wrong folder.

---

## 5. Maintenance (Updating Gravity)

When a Ghost Link becomes Confirmed, or a manual file is added:
1.  **Recalculate:** Fetch all `is_auto=0` embeddings for the tag.
2.  **Average:** `NewCentroid = SUM(Vectors) / Count`.
3.  **Persist:** Update `tags.centroid`.

This ensures the tag's gravity drifts closer to what the user *actually* puts there, not what the system *thought* should be there.
