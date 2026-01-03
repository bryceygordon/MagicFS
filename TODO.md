# FILE: TODO.md
# MagicFS Roadmap

## 🏆 Completed Milestones
- [x] **Phase 10: The AI Engine Upgrade**
    - **Verdict:** Nomic Embed v1.5 (768 dims) + Recursive Chunking is the winner.
    - **Score:** High relevance (>0.70), no semantic dilution, no UTF-8 panics.

---

## 🚀 Phase 11: The "Snap" & "Flow" (UX Refinement)
**Goal:** Make the interaction feel "native" and stop the "Admin Privileges" errors.
- [x] **Search Debouncing / Overrides:** - *Discussion Needed:* How to handle "Typewriter" searching (r -> re -> rec). 
    - *Ideas:* Cancel stale searches? Minimum char limit? 
- [ ] **EAGAIN Handling:** Fix the "Admin Privileges" error in Dolphin by handling async delays better.
- [ ] **Inode Stability:** Prevent "File changed on disk" errors by pinning Inode IDs to search results.

## 🧠 Phase 12: Organization & Persistence (The "Second Brain")
**Goal:** Saved views, tagging, and portable configuration.
- [ ] **Saved Searches (Aliasing):**
    - Map complex queries to simple folder names.
    - *Example:* `/magic/saved/Healthy_Dinner/` -> Query: "low carb high protein dinner"
- [ ] **The Tagging Filesystem:**
    - "Directories are Tags." Files can live in multiple tag folders.
    - *Vision:* `cp file.txt /magic/tags/urgent/` applies the tag.
- [ ] **Portable Configuration (The Map):**
    - A self-sustaining `config.yaml` that remembers watch paths, saved searches, and tags.
    - *Goal:* "New machine, same brain."

## 🛠️ Phase 13: Developer Experience
- [x] **Dev Script Polish:** Add `--keep-db` flag to `dev.sh`.
