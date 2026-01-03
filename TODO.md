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
- [ ] the above typwriter task is fixed though there remains an issue. when interacting with a directory within dolhpin, if i right mouse all of the options (.zip, .tar etc) load as searches. so i am thinking is it worth revisiting this behaviour all together. i think there is benefit to having solved the typwriter fault, but it probably speaks to a bigger issue in that a lot of general interaction with the file system is triggering noise searches. even .hidden is a search.
further to this, i found that if the search `search/foobar/` ends with a forward slash `/` then if you edit within that forward slash it will start typwriting again. this
is sometimes needed when you have a long semantic search and you want to add or remove items from the middle of the search line. 
do we need to think differently about interacting with this file system alltogether?
- [ ] i made a new file in one of the watched directories and it did not appear in the MagicFS
- [ ] what happens if i want to delete a saved search?
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
