# MagicFS Development Roadmap

## 📜 History
* **Phases 1-5**: Foundation.
* **Phase 6**: Basic Hardening (10MB limits, Binary checks).

---

## 🏗️ Era 2: Scalability & Resilience [ACTIVE]

### 🚧 Phase 6.5: "The Conveyor Belt" (Concurrency & Flow Control) [DONE]
**Goal:** Stop the system from choking on massive file dumps.
* [x] **Dynamic Scaling:** Use `available_parallelism()` to set worker limits.
* [x] **Priority Interlock:** Indexer (Writer) takes precedence over Searcher (Reader).
* [x] **Lockout/Tagout:** Prevent race conditions on individual files.

### 🚧 Phase 6.9: "The Safety Circuits" (Stability) [NEXT UP]
**Goal:** Prevent structural failures and user error loops.
1.  **Anti-Feedback Loop:** Prevent recursive mounting.
2.  **Thermal Overload:** "Cooling down" hot files (logs).
3.  **Manual Sync:** "Push button" scanning for dumb drives.

---

## 📚 Era 3: Content [PLANNED]

### 🔮 Phase 7: The Universal Reader
**Goal:** PDF/DOCX support.
* **Note:** We will not start this until `test_07` (The Real World) passes 100% consistently.

---

## 🧠 The "Thin Client" Vision
**Goal:** An Evernote-killer that uses MagicFS as its backend.
* **Design:** The client has no database. It just reads the file system.
* **Manual Sync:** The client will use the `touch .magic/refresh` API to force updates.
