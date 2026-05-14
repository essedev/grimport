//! Centralized OS-aware path resolution.
//!
//! macOS keeps every file under `~/Library/Application Support/portsage/`
//! (existing convention, unchanged).
//!
//! Linux follows XDG Base Directory:
//!   - DB:     `$XDG_DATA_HOME/portsage/portsage.db`  (default `~/.local/share/portsage/`)
//!   - Socket: `$XDG_RUNTIME_DIR/portsage/portsage.sock`
//!     (fallback `/tmp/portsage-<uid>.sock` when `XDG_RUNTIME_DIR` is unset)
//!   - State:  `$XDG_STATE_HOME/portsage/`            (default `~/.local/state/portsage/`)
//!
//! Headless mode accepts `--socket <path>` and `PORTSAGE_SOCKET=<path>` to
//! override the default socket location; this is how the system-wide systemd
//! unit places the socket at `/run/portsage/portsage.sock`.

use std::path::{Path, PathBuf};

/// Data directory: where the SQLite database lives.
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("portsage")
}

/// Where the SQLite database file lives.
pub fn db_path() -> PathBuf {
    data_dir().join("portsage.db")
}

/// Runtime directory: where the Unix socket lives.
///
/// On macOS this is the same as the data directory (one folder, the way it
/// has always been). On Linux it follows `$XDG_RUNTIME_DIR`, which is cleaned
/// up automatically on logout in a typical systemd-user session.
pub fn runtime_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        if let Some(rt) = dirs::runtime_dir() {
            return rt.join("portsage");
        }
        // Server / minimal-env fallback. Use the effective uid so multiple
        // users on the same machine don't collide on `/tmp/portsage.sock`.
        // SAFETY: geteuid() is async-signal-safe and never fails.
        let uid = unsafe { libc::geteuid() };
        return PathBuf::from(format!("/tmp/portsage-{}", uid));
    }
    #[allow(unreachable_code)]
    {
        data_dir()
    }
}

/// Default location of the Unix socket. The headless server may override this
/// via `--socket <path>`; see [`resolve_socket_path`].
pub fn socket_path() -> PathBuf {
    runtime_dir().join("portsage.sock")
}

/// Where the extracted MCP server files live (mirror of
/// `portsage_cli::mcp::install_dir`). Used by the Tauri-side install flow so
/// the GUI install and the `portsage mcp install` CLI flow target the same
/// directory.
#[allow(dead_code)]
pub fn mcp_install_dir() -> PathBuf {
    data_dir().join("mcp")
}

/// State directory: logs, future caches.
#[allow(dead_code)]
pub fn state_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        if let Some(s) = dirs::state_dir() {
            return s.join("portsage");
        }
    }
    data_dir()
}

/// Resolve the socket path with overrides applied. Precedence (highest first):
///   1. `argv_override` (the `--socket` CLI flag)
///   2. `PORTSAGE_SOCKET` env var
///   3. Default per-OS [`socket_path`]
pub fn resolve_socket_path(argv_override: Option<&Path>) -> PathBuf {
    if let Some(p) = argv_override {
        return p.to_path_buf();
    }
    if let Some(p) = std::env::var_os("PORTSAGE_SOCKET") {
        return PathBuf::from(p);
    }
    socket_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_socket_path_prefers_argv_override() {
        let p = PathBuf::from("/tmp/explicit.sock");
        assert_eq!(resolve_socket_path(Some(&p)), p);
    }

    #[test]
    fn resolve_socket_path_uses_env_when_no_argv() {
        // SAFETY: tests can race on env vars. Use a unique value so we can
        // detect interference and restore the previous value at the end.
        let prev = std::env::var_os("PORTSAGE_SOCKET");
        std::env::set_var("PORTSAGE_SOCKET", "/tmp/from-env.sock");
        let got = resolve_socket_path(None);
        match prev {
            Some(v) => std::env::set_var("PORTSAGE_SOCKET", v),
            None => std::env::remove_var("PORTSAGE_SOCKET"),
        }
        assert_eq!(got, PathBuf::from("/tmp/from-env.sock"));
    }

    #[test]
    fn db_path_ends_in_portsage_db() {
        let p = db_path();
        assert_eq!(p.file_name().and_then(|s| s.to_str()), Some("portsage.db"));
        assert_eq!(
            p.parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str()),
            Some("portsage")
        );
    }

    #[test]
    fn socket_path_ends_in_portsage_sock() {
        let p = socket_path();
        assert_eq!(
            p.file_name().and_then(|s| s.to_str()),
            Some("portsage.sock")
        );
    }

    /// `paths::socket_path` (used by the Tauri server) and
    /// `portsage_client::default_socket_path` (used by the CLI / MCP server
    /// when no override is set) must point at the same file - otherwise a
    /// CLI / MCP client speaks to a socket the server never bound, and every
    /// call fails with "app not running". This test pins the parity so any
    /// future drift in either implementation gets caught at `cargo test`.
    ///
    /// We clear `PORTSAGE_SOCKET` for the duration so the env override doesn't
    /// short-circuit the comparison on whoever's CI is running this.
    #[test]
    fn socket_path_matches_portsage_client_default() {
        // SAFETY: tests can race on env vars. Snapshot, clear, compare, restore.
        let prev = std::env::var_os("PORTSAGE_SOCKET");
        std::env::remove_var("PORTSAGE_SOCKET");
        let app_side = socket_path();
        let client_side = portsage_client::default_socket_path();
        match prev {
            Some(v) => std::env::set_var("PORTSAGE_SOCKET", v),
            None => std::env::remove_var("PORTSAGE_SOCKET"),
        }
        assert_eq!(
            app_side, client_side,
            "paths::socket_path drifted from portsage_client::default_socket_path \
             - update both so the CLI/MCP can reach the server"
        );
    }
}
