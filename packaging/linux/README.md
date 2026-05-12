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

## User and group of the running service

The installer patches `User=` and `Group=` in the systemd unit to the target user (`--user <name>` or `$SUDO_USER`) before installing it. The shipped template defaults to `User=portsage Group=portsage` for a multi-tenant setup, but the installer rewrites both lines so the running service has `fsuid` and `fsgid` matching the dev user's primary group.

This is **load-bearing for the Process column in the Mac UI**. The scanner that maps listening ports to process names reads `/proc/<other_pid>/fd/*`, which the kernel's `__ptrace_may_access(PTRACE_MODE_FSCREDS)` gates on a match of **both** `fsuid` and `fsgid` against the target process's creds (not just uid). If the service ran as `portsage:portsage` (gid 987) but your `vite` / `node` / `python` processes run as `you:you` (gid 1000), the gid mismatch makes the kernel return `EACCES` on the readlink even though the uid matches - and every port in the UI shows `?` for the process.

If you need to revert to a multi-tenant setup later (one service shared by several users), edit `/etc/systemd/system/portsage-server.service`, set `User=portsage Group=portsage`, `systemctl daemon-reload`, restart. Accept that the Process column will read `?` for ports owned by users other than `portsage`.

### When the Process column shows `?`

Even with the installer's default setup, you can hit this:

- **Process owned by a user other than the one the service runs as**: kernel blocks the readlink. The port is still listed as active; only the process name + PID are missing.
- **`kill_port` against that process**: fails. Run `portsage kill <port>` as that owner, or as root.
- **Process under a different primary group**: same EACCES path (kernel checks both uid and gid).

## Manual launch (no systemd)

```sh
PORTSAGE_SOCKET=/tmp/portsage.sock /usr/local/bin/portsage-server --socket /tmp/portsage.sock
```

The CLI auto-spawns the server on demand when it's installed at `/usr/local/bin/portsage-server`, so even without systemd you can just run `portsage list` and the backend will come up.
