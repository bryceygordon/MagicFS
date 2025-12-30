# MagicFS Task List

## 🔴 CRITICAL BLOCKER: The "Safe.txt" Fault (Race Condition)
**Context:** `test_07_real_world.py` Scenario 1 fails. After destroying a folder (`trap/`) and immediately rebuilding it, `safe.txt` is missing from the index.

**The Electrician's Diagnosis:**
We have a "Stop" button (Delete Event) and a "Start" button (Create Event) being pressed simultaneously. The "Stop" relay seems to be dropping out *after* the "Start" relay latches, cutting power to the valid file.

**Current Safety Mechanisms (Installed):**
1.  **The Interlock (Oracle):** Indexing (Forward) has priority over Searching (Reverse).
2.  **The Lockout (Oracle):** We serialize operations. `safe.txt` and `DELETE:safe.txt` cannot run in the same tick.
3.  **The Arbitrator (Indexer):** Before deleting, we check `Path::exists`. If the file is on disk, we *should* reject the Delete ticket.

**⚠️ The Anomaly:**
Despite the Arbitrator, the file is still gone.
* *Hypothesis A:* The Arbitrator check happens *too fast*. The file isn't created yet when we check `exists()`, so we proceed to delete.
* *Hypothesis B:* The Librarian is sending the events in the wrong order (Delete *after* Create).

**Next Steps (Immediate Actions):**
* [ ] **Verify Reality:** Modify `test_07` to assert if `safe.txt` exists *on disk* when the failure occurs. (Did we fail to index, or did we actively delete it?)
* [ ] **Debug the Arbitrator:** Add logging to `Indexer::remove_file`. Is it seeing the file?
* [ ] **Refine the Sensor:** Ensure `wait_for_stable_db` isn't giving false positives on "Motor Stopped".

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
