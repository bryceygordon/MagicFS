
---

## 📦 Installation (Arch Linux)

MagicFS provides a native Arch Linux package via `PKGBUILD` and a setup wizard to handle configuration and persistence.

### 1. Build & Install Package

This compiles the project from source and installs the binary, service templates, and management tools to your system.

```bash
# Navigate to the packaging directory
cd pkg

# Build and install using makepkg
makepkg -si

```

### 2. Configure & Start

Once installed, use the **MagicFS Manager** to set your mount point, watch directory, and enable the background service.

```bash
# Run the interactive setup wizard
magicfs-manager setup

```

The wizard will:

1. Ask where you want to mount MagicFS (Default: `~/MagicFS`).
2. Ask which directory to monitor (Default: `~/Documents`).
3. Generate a persistent Systemd configuration.
4. Start the daemon automatically on login.

To verify it's working:

```bash
# Check the service status
systemctl --user status magicfs

# View live logs
journalctl --user -u magicfs -f

```

---

## 🗑️ Uninstallation

To completely remove MagicFS from your system, follow this two-step process to ensure no configuration files or zombie processes are left behind.

### 1. Stop Service & Remove Config

Use the manager to stop the daemon and remove user-specific configurations.

```bash
magicfs-manager remove

```

*You will be asked if you want to delete the local database index (`~/.magicfs`). Select 'Y' for a complete scrub.*

### 2. Uninstall Package

Remove the binary and system files using pacman.

```bash
sudo pacman -Rns magicfs-git

```
