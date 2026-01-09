# MagicFS

**MagicFS** is a semantic virtual filesystem that turns your chaos into an API. It mounts a FUSE drive where files are organized by *meaning*, not just location.

## 🌟 Key Features

* **Semantic Search**: `ls ~/MagicFS/search/"tax invoice"` instantly lists relevant files.
* **Mirror Navigation**: Browse your source directories normally via `~/MagicFS/mirror/`.
* **Universal Reader & Writer**: Open, edit, and save files directly from the virtual mount. MagicFS passes reads/writes through to the real file on disk.
* **Multi-Root Monitoring**: Watch `~/Documents` and `~/Projects` simultaneously.
* **Privacy First**: All embeddings are calculated locally. No data leaves your machine.

## 📦 Installation (Arch Linux)

```bash
cd pkg
makepkg -si
```

## 🚀 Usage

### 1. Start the Daemon
You can run it as a systemd service or manually.

```bash
# Manual Run (Multi-Root)
magicfs ~/MagicFS ~/Documents,~/Pictures
```

### 2. Navigate
MagicFS exposes two primary interfaces:

1.  **Search Mode** (`/search`):
    * Navigate to `/search/<query>` to trigger a semantic search.
    * Example: `cd "/home/user/MagicFS/search/beef recipes"`
    * Result: Files are listed by relevance (e.g., `0.95_steak.txt`).

2.  **Mirror Mode** (`/mirror`):
    * Browse your watched directories as a unified tree.
    * Example: `/home/user/MagicFS/mirror/Documents/` maps to `~/Documents`.

### 3. Edit
You can open any file in MagicFS with your favorite editor (Micro, Vim, VS Code).
* **Read**: File content is streamed from the real disk (Passthrough).
* **Write**: Saving a file in MagicFS updates the real file on your disk instantly.

## 🛠️ Configuration

Configuration is handled via `~/.config/magicfs/daemon.conf` or CLI arguments.

```bash
MAGICFS_MOUNT="/home/user/MagicFS"
MAGICFS_WATCH="/home/user/Documents,/home/user/Obsidian"
```

## ⚠️ Known Limitations

### File Operations

**1. `cp` vs. `cat` vs. `>` (Copy Import Behavior)**
* **Observation**: The standard `cp` command may fail with certain atomic copy utilities (rsync temp-file strategy).
* **Reason**: FUSE create() limitations with specific atomic patterns.
* **Workaround**: Use shell redirection `cat source > dest` or `cp source dest` works for regular files. This affects the "Import via Copy" workflow for complex file managers.
* **Impact**: Low. Direct write operations (editors, shell redirection) work perfectly.

**2. Large File Import**
* **Limit**: Files > 10MB are automatically ignored by the indexer.
* **Reason**: Memory safety and vectorization constraints.
* **Workaround**: None. Large files serve little value for semantic search.

**3. FUSE Zombie Mounts**
* **Issue**: Killing the daemon without proper unmount leaves a zombie mount.
* **Fix**: Use `sudo umount -l <mount>` to force cleanup before restarting.
* **Current**: `dev.sh` and `tests/run_suite.sh` handle this automatically.

### Performance

**4. First-Run Latency**
* **Issue**: Initial vector model download (Nomic Embed v1.5) takes time.
* **Behavior**: First startup may take 10-30s longer to download ~50MB model.
* **Cache**: Model cached in `~/.cache/fastembed/` for subsequent runs.

**5. Indexing Bulk Import**
* **War Mode**: During initial scan, WAL mode is disabled for speed.
* **Risk**: Power loss during initial indexing can corrupt database (requires restart).
* **Safety**: Database switches to safe WAL mode once backlog is cleared.

### Semantic Search

**6. Relevance Scoring**
* **Logic**: Scores are 0.0 to 1.0 based on cosine similarity.
* **Threshold**: Scores > 0.5 are generally relevant, > 0.8 is very strong.
* **Variation**: Scores fluctuate based on model version and embedding precision.

**7. Noise Filtering**
* **Bouncer**: Rejects hidden files (`.`), archives (`.zip`), and binary formats.
* **Reason**: Prevents database pollution with unsearchable content.
* **Override**: Modify `src/core/bouncer.rs` to adjust patterns.

### Data Safety

**8. Soft Delete Only**
* **Current**: `rm` from tag view preserves physical data in `~/[WatchDir]/_imported/`.
* **Timeline**: Physical files in `_imported/` are not automatically cleaned.
* **Future**: Phase 16+ will implement Scavenger and Incinerator.

**9. Database Permissions**
* **Issue**: Daemon runs as root, creating WAL files owned by root.
* **Fix**: Permission hardening changes ownership to real user.
* **Verify**: Check `/tmp/.magicfs_nomic/index.db*` files if CLI tools can't read DB.

---

## 🏗️ Architecture Documents

For deep technical specifications:
* **`CHARTER.md`** - The core philosophies and "Prime Directives"
* **`CONCEPTS.md`** - Vision for Thin Clients and The Lens
* **`PERFORMANCE_OPTIMISATION.md`** - War Mode vs. Peace Mode strategies
* **`SPEC_PERSISTENCE.md`** - Database schema and Inode Zoning
* **`SPEC_AUTO_ORGANIZATION.md`** - Magnetic Tags (Future)
