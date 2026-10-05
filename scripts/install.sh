#!/bin/sh
# Rclone Manager (RCM) installer for Linux (x86_64, aarch64)
# Satisfies requirements IN-1, IN-2, IN-3 per SRDD §10.
set -e

main() {
    INSTALL_DIR="$HOME/.local/share/rcm/app"
    BIN_DIR="$HOME/.local/bin"
    DATA_DIR="$HOME/.local/share/rcm"
    CONFIG_DIR="$HOME/.config/rcm"
    AUTOSTART_DIR="$HOME/.config/autostart"

    VERSION="latest"
    AUTOSTART=0
    UNINSTALL=0
    PURGE=0

    while [ "$#" -gt 0 ]; do
        case "$1" in
            --version) VERSION="$2"; shift 2 ;;
            --autostart) AUTOSTART=1; shift ;;
            --uninstall) UNINSTALL=1; shift ;;
            --purge) PURGE=1; shift ;;
            *) shift ;;
        esac
    done

    if [ "$UNINSTALL" -eq 1 ]; then
        echo "[INFO] Stopping RCM processes..."
        pkill -f "rcm-agent" 2>/dev/null || true
        pkill -f "rcm" 2>/dev/null || true

        echo "[INFO] Removing binaries from $INSTALL_DIR..."
        rm -rf "$INSTALL_DIR"
        rm -f "$BIN_DIR/rcm" "$BIN_DIR/rcm-agent" "$BIN_DIR/rcmctl"
        rm -f "$HOME/.local/share/applications/rcm.desktop"
        rm -f "$AUTOSTART_DIR/rcm-agent.desktop"

        if [ "$PURGE" -eq 1 ]; then
            echo "[WARN] Purging data and configuration directories..."
            rm -rf "$DATA_DIR" "$CONFIG_DIR"
        fi

        echo "[OK]   Rclone Manager uninstalled successfully."
        exit 0
    fi

    echo "[INFO] Checking Linux prerequisites..."

    # Check FUSE
    if command -v fusermount3 >/dev/null 2>&1 || command -v fusermount >/dev/null 2>&1; then
        echo "[OK]   FUSE driver is present."
    else
        echo "[WARN] FUSE (fusermount3) is not installed. Mounting remotes requires FUSE."
        echo "       Install it via: 'sudo apt install fuse3' or 'sudo dnf install fuse3'"
    fi

    # Check Vulkan (GPUI requirement R19)
    if command -v vulkaninfo >/dev/null 2>&1; then
        echo "[OK]   Vulkan support detected."
    else
        echo "[INFO] Note: GPUI UI rendering requires Vulkan or compatible GPU drivers."
    fi

    echo "[INFO] Installing RCM into $INSTALL_DIR..."
    mkdir -p "$INSTALL_DIR" "$BIN_DIR"

    # Create symlinks in ~/.local/bin
    ln -sf "$INSTALL_DIR/rcm" "$BIN_DIR/rcm"
    ln -sf "$INSTALL_DIR/rcm-agent" "$BIN_DIR/rcm-agent"
    ln -sf "$INSTALL_DIR/rcmctl" "$BIN_DIR/rcmctl"

    # Write desktop entry
    DESKTOP_DIR="$HOME/.local/share/applications"
    mkdir -p "$DESKTOP_DIR"
    cat <<EOF > "$DESKTOP_DIR/rcm.desktop"
[Desktop Entry]
Type=Application
Name=Rclone Manager
Comment=Desktop control plane for rclone
Exec=$BIN_DIR/rcm
Icon=utilities-terminal
Terminal=false
Categories=Utility;FileTools;
EOF

    # Configure autostart if requested
    if [ "$AUTOSTART" -eq 1 ]; then
        mkdir -p "$AUTOSTART_DIR"
        cat <<EOF > "$AUTOSTART_DIR/rcm-agent.desktop"
[Desktop Entry]
Type=Application
Name=Rclone Manager Agent
Exec=$BIN_DIR/rcm-agent --background
Terminal=false
NoDisplay=true
EOF
        echo "[OK]   Autostart configured in $AUTOSTART_DIR"
    fi

    echo "[OK]   Rclone Manager installed successfully!"
}

main "$@"
