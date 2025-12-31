# MagicFS Task List


---

## 🛡️ Phase 6.9: The Safety Systems (Infrastructure Hardening)
**Goal:** Prevent self-destruction and infinite loops before adding new file formats.

* [ ] **The Anti-Feedback Switch (Main):**
    * **Risk:** Mounting MagicFS *inside* the watched directory causes an infinite recursion loop (Microphone pointing at Speaker).
    * **Fix:** Add startup check: `if watch_dir.starts_with(mount_point) { panic!("Feedback Loop Detected"); }`.

* [ ] **Thermal Overload (Chatter Protection):**
    * **Risk:** Log files updating 100x/minute burn out the Indexer.
    * **Fix:** `HashMap` tracking file heat. If > 5 updates/min, lockout for 5 mins.

* [ ] **Manual Override (Forced Sync):**
    * **Risk:** Network drives (NAS) don't send "Motion Sensor" events.
    * **Fix:** Watch for `touch .magic/refresh` to trigger a full manual scan.

---
---

## 🚀 Phase 7: The Universal Reader [ACTIVE]

**Objective:** Break the format barrier. Support PDF, DOCX, and other rich media.

* [ ] **Dependencies**: Add `pdf-extract`.
* [ ] **Extractor Refactor**: Route file types to specific parsers in `src/storage/text_extraction.rs`.
* [ ] **Test**: Create `tests/cases/test_06_rich_media.py`.

## 🔮 Phase 8: Aggregation [PENDING]

* [ ] **Config**: `~/.config/magicfs/sources.json`.
* [ ] **Virtual Dirs**: `/sources` and `/saved` endpoints.
