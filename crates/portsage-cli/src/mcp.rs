//! `portsage mcp install / uninstall / status`.
//!
//! CLI-specific glue around the shared [`portsage_mcp`] crate: embeds the four
//! MCP source files into the binary at compile time, extracts them on demand,
//! runs `uv sync`, and orchestrates the install / uninstall sequences.
//!
//! Everything that touches the user's Claude config (`~/.claude.json`,
//! `~/.claude/skills/`, `~/.claude/settings.json`) lives in `portsage-mcp`
//! so the Tauri app's Settings panel and `portsage mcp install` share the
//! same atomic-write, parse-or-bail, and allowlist logic.

use std::path::{Path, PathBuf};
use std::process::Command;

pub use portsage_mcp::{McpError, Scope};

// Embedded MCP server files. Paths are relative to this source file:
//   crates/portsage-cli/src/mcp.rs  ->  ../../../mcp/<file>
const SERVER_PY: &str = include_str!("../../../mcp/server.py");
const PYPROJECT_TOML: &str = include_str!("../../../mcp/pyproject.toml");
const UV_LOCK: &str = include_str!("../../../mcp/uv.lock");
const SKILL_MD: &str = include_str!("../../../mcp/SKILL.md");

/// Where the embedded MCP files get written. Mirrors `paths::mcp_install_dir`
/// in the Tauri crate (same path on macOS, XDG-data on Linux) so the GUI
/// install and the CLI install target the same directory.
pub fn install_dir() -> PathBuf {
    portsage_client::default_data_dir().join("mcp")
}

/// Write the embedded MCP files into `dir`, creating it if needed. Idempotent:
/// re-running overwrites with the current binary's embedded copy so app
/// upgrades propagate.
pub fn write_embedded_files(dir: &Path) -> Result<(), McpError> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("server.py"), SERVER_PY)?;
    std::fs::write(dir.join("pyproject.toml"), PYPROJECT_TOML)?;
    std::fs::write(dir.join("uv.lock"), UV_LOCK)?;
    std::fs::write(dir.join("SKILL.md"), SKILL_MD)?;
    Ok(())
}

/// CLI-side error type. Wraps shared MCP errors and adds the uv-specific
/// variants that only the CLI orchestrator surfaces.
#[derive(Debug, thiserror::Error)]
pub enum CliMcpError {
    #[error(transparent)]
    Mcp(#[from] McpError),

    #[error(
        "uv is not installed. Install it: \
         curl -LsSf https://astral.sh/uv/install.sh | sh"
    )]
    UvNotFound,

    #[error("uv sync failed: {stderr}")]
    UvSyncFailed { stderr: String },
}

impl From<std::io::Error> for CliMcpError {
    fn from(e: std::io::Error) -> Self {
        CliMcpError::Mcp(McpError::Io(e))
    }
}

/// Run `uv sync --quiet` in `dir`. Returns [`CliMcpError::UvNotFound`] if `uv`
/// is not on PATH so we can surface a clean install hint.
pub fn run_uv_sync(dir: &Path) -> Result<(), CliMcpError> {
    let attempt = Command::new("uv")
        .args(["sync", "--quiet"])
        .current_dir(dir)
        .output();
    match attempt {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(CliMcpError::UvSyncFailed {
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(CliMcpError::UvNotFound),
        Err(e) => Err(McpError::Io(e).into()),
    }
}

/// Snapshot of the current MCP install state, for `portsage mcp status`.
///
/// Extends [`portsage_mcp::McpStatus`] with `uv_available`, which only the CLI
/// reports (the GUI doesn't need it - the GUI never shells out to uv).
#[derive(Debug, serde::Serialize)]
pub struct McpStatus {
    pub mcp_dir: String,
    pub files_present: bool,
    pub uv_available: bool,
    pub registered_global: bool,
    pub registered_project_cwd: bool,
    pub skill_installed: bool,
    pub allowlist_has_portsage: bool,
}

pub fn status() -> Result<McpStatus, McpError> {
    let mcp_dir = install_dir();
    let base = portsage_mcp::status(&mcp_dir)?;
    let uv_available = Command::new("uv")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    Ok(McpStatus {
        mcp_dir: base.mcp_dir,
        files_present: base.files_present,
        uv_available,
        registered_global: base.registered_global,
        registered_project_cwd: base.registered_project_cwd,
        skill_installed: base.skill_installed,
        allowlist_has_portsage: base.allowlist_has_portsage,
    })
}

/// Paths/dirs touched by [`install`], surfaced so the CLI can print a summary.
pub struct InstallReport {
    pub mcp_dir: PathBuf,
    pub claude_config: PathBuf,
    pub skill_file: PathBuf,
    pub settings_file: PathBuf,
}

pub fn install(scope: Scope, skip_uv: bool) -> Result<InstallReport, CliMcpError> {
    let mcp_dir = install_dir();
    write_embedded_files(&mcp_dir)?;
    if !skip_uv {
        run_uv_sync(&mcp_dir)?;
    }
    let claude_config = portsage_mcp::register_in_claude(scope, &mcp_dir)?;
    let skill_file = portsage_mcp::install_skill(&mcp_dir)?;
    let settings_file = portsage_mcp::add_permissions()?;
    Ok(InstallReport {
        mcp_dir,
        claude_config,
        skill_file,
        settings_file,
    })
}

pub struct UninstallReport {
    pub unregistered_global: bool,
    pub unregistered_project: bool,
    pub skill_removed: bool,
    pub permissions_removed: usize,
    pub files_removed: bool,
}

/// Reverse of `install`. `wipe_files = true` also nukes the data dir's mcp/
/// directory; otherwise we only de-register so re-install is cheap.
pub fn uninstall(wipe_files: bool) -> Result<UninstallReport, McpError> {
    let unregistered_global = portsage_mcp::unregister_from_claude(Scope::Global)?;
    let unregistered_project = portsage_mcp::unregister_from_claude(Scope::Project)?;
    let skill_removed = portsage_mcp::remove_skill()?;
    let permissions_removed = portsage_mcp::remove_permissions()?;
    let mut files_removed = false;
    if wipe_files {
        let dir = install_dir();
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
            files_removed = true;
        }
    }
    Ok(UninstallReport {
        unregistered_global,
        unregistered_project,
        skill_removed,
        permissions_removed,
        files_removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_embedded_files_writes_all_four() {
        let dir = tempdir().unwrap();
        write_embedded_files(dir.path()).unwrap();
        for f in &["server.py", "pyproject.toml", "uv.lock", "SKILL.md"] {
            assert!(dir.path().join(f).exists(), "missing {f}");
        }
        // Cheap canary: server.py must contain real code. Catches an
        // `include_str!` pointing at the wrong path and ending up empty.
        let py = std::fs::read_to_string(dir.path().join("server.py")).unwrap();
        assert!(py.contains("FastMCP"), "server.py should embed real code");
        let skill = std::fs::read_to_string(dir.path().join("SKILL.md")).unwrap();
        assert!(skill.contains("portsage"));
    }

    #[test]
    fn install_dir_is_under_data_dir() {
        let d = install_dir();
        assert_eq!(d.file_name().and_then(|s| s.to_str()), Some("mcp"));
        assert_eq!(
            d.parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str()),
            Some("portsage")
        );
    }
}
