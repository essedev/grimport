#!/usr/bin/env bash
# Install the Portsage Linux server. Idempotent: safe to re-run for upgrades.
#
# Usage:
#   sudo ./install.sh                  # install for the user who invoked sudo
#   sudo ./install.sh --user simone    # install and grant access to a named user
#
# What it does:
#   1. Creates the `portsage` system user/group.
#   2. Copies portsage-server and portsage to /usr/local/bin/.
#   3. Installs the systemd unit at /etc/systemd/system/portsage-server.service.
#   4. Adds the target user to the `portsage` group so they can talk to the
#      socket at /run/portsage/portsage.sock without sudo.
#   5. Enables and (re)starts the service.

set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
    echo "install.sh must run as root (try: sudo ./install.sh)" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SERVER_BIN="$SCRIPT_DIR/portsage-server"
CLI_BIN="$SCRIPT_DIR/portsage"
UNIT_FILE="$SCRIPT_DIR/portsage-server.service"

# --- Parse args ---
TARGET_USER="${SUDO_USER:-}"
while [ $# -gt 0 ]; do
    case "$1" in
        --user)
            TARGET_USER="$2"
            shift 2
            ;;
        --user=*)
            TARGET_USER="${1#--user=}"
            shift
            ;;
        -h|--help)
            sed -n '2,18p' "$0"
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

if [ -z "$TARGET_USER" ]; then
    echo "Cannot determine target user (no SUDO_USER and no --user given)." >&2
    echo "Re-run with --user <username> to grant socket access to a specific user." >&2
    exit 1
fi

if ! id "$TARGET_USER" >/dev/null 2>&1; then
    echo "User '$TARGET_USER' does not exist." >&2
    exit 1
fi

# --- Sanity-check the staged binaries ---
for f in "$SERVER_BIN" "$CLI_BIN" "$UNIT_FILE"; do
    if [ ! -f "$f" ]; then
        echo "missing file: $f" >&2
        echo "Run install.sh from inside the unpacked tarball, not in isolation." >&2
        exit 1
    fi
done

echo "==> Installing Portsage Linux server for user '$TARGET_USER'"

# --- 1. portsage system user/group ---
if ! getent group portsage >/dev/null; then
    echo "    creating system group 'portsage'"
    groupadd --system portsage
fi
if ! id portsage >/dev/null 2>&1; then
    echo "    creating system user 'portsage'"
    useradd --system --no-create-home --shell /usr/sbin/nologin \
        --gid portsage portsage
fi

# --- 2. Binaries ---
echo "    installing /usr/local/bin/portsage-server"
install -m 0755 "$SERVER_BIN" /usr/local/bin/portsage-server
echo "    installing /usr/local/bin/portsage (CLI)"
install -m 0755 "$CLI_BIN" /usr/local/bin/portsage

# --- 3. systemd unit ---
echo "    installing /etc/systemd/system/portsage-server.service"
install -m 0644 "$UNIT_FILE" /etc/systemd/system/portsage-server.service
systemctl daemon-reload

# --- 4. Grant socket access ---
if id -nG "$TARGET_USER" | tr ' ' '\n' | grep -qx portsage; then
    echo "    user '$TARGET_USER' is already in group 'portsage'"
else
    echo "    adding '$TARGET_USER' to group 'portsage'"
    usermod -a -G portsage "$TARGET_USER"
    echo "    NOTE: '$TARGET_USER' must log out and back in for the group change to apply."
fi

# --- 5. Enable + (re)start ---
echo "    enabling and restarting portsage-server.service"
systemctl enable portsage-server.service >/dev/null
# Restart so an existing install picks up the new binary.
systemctl restart portsage-server.service

# Brief health check.
sleep 1
if systemctl is-active --quiet portsage-server.service; then
    echo "==> portsage-server is running."
    echo "    Socket: /run/portsage/portsage.sock"
    echo "    Status: systemctl status portsage-server"
    echo "    Logs:   journalctl -u portsage-server -f"
else
    echo "==> portsage-server failed to start." >&2
    echo "    Inspect: journalctl -u portsage-server -n 50" >&2
    exit 1
fi
