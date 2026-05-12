# Portsage Linux server

Headless port-allocation server for Linux. Pair it with the Portsage macOS app to manage ports on a remote dev box (e.g. a Hetzner server reachable over Tailscale) from the same menubar that controls your local ports.

This package contains:

- `portsage-server` - the headless backend (the same binary the macOS app runs with `--headless`, just built for Linux musl).
- `portsage` - the CLI. Same UX as the macOS CLI; auto-detects the local socket.
- `portsage-server.service` - systemd unit, system-wide.
- `install.sh` - idempotent installer.

## Install

```sh
sudo ./install.sh
# or: sudo ./install.sh --user <username>
```

The installer creates a `portsage` system user, drops the binaries in `/usr/local/bin/`, installs the systemd unit, and adds the target user to the `portsage` group so they can talk to the socket without sudo. **You must log out and back in for the group change to take effect**.

After install:

```sh
systemctl status portsage-server     # is it running?
journalctl -u portsage-server -f     # follow the logs
portsage doctor                      # the CLI's own health check
```

The socket lives at `/run/portsage/portsage.sock`. The CLI auto-discovers it via `PORTSAGE_SOCKET=/run/portsage/portsage.sock` (set by the systemd unit) or via `--socket`.

## Upgrade

Re-run `sudo ./install.sh` from a newer tarball. It overwrites the binaries, reloads the unit, and restarts the service. Your database (`/var/lib/portsage/portsage.db`) is untouched.

## Uninstall

```sh
sudo systemctl disable --now portsage-server
sudo rm /etc/systemd/system/portsage-server.service
sudo rm /usr/local/bin/portsage-server /usr/local/bin/portsage
sudo systemctl daemon-reload
# optional: also remove the data dir
sudo rm -rf /var/lib/portsage
sudo userdel portsage && sudo groupdel portsage
```

## Known limitation: kill and PID resolution

`portsage-server` runs as the `portsage` system user. The Linux scanner reads `/proc/net/tcp` (visible to anyone) for the list of listening ports, but mapping those sockets back to PIDs requires reading `/proc/<pid>/fd/`, which is restricted to the process owner.

In practice that means:

- **Reserve / register / list / unmanaged**: work fully. Ports are visible because `/proc/net/tcp` is world-readable.
- **`kill_port` against another user's process**: fails. Run `portsage kill <port>` as that user, or as root.
- **Process names in `list_unmanaged`**: shown as `?` for processes owned by other users.

For full kill/PID visibility on a single-user dev box, you can run the server as your own user instead - either swap `User=portsage` in the unit for your username, or use a per-user systemd unit at `~/.config/systemd/user/portsage-server.service`.

## Manual launch (no systemd)

```sh
PORTSAGE_SOCKET=/tmp/portsage.sock /usr/local/bin/portsage-server --socket /tmp/portsage.sock
```

The CLI auto-spawns the server on demand when it's installed at `/usr/local/bin/portsage-server`, so even without systemd you can just run `portsage list` and the backend will come up.
