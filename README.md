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
