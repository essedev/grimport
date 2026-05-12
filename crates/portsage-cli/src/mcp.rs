//! `portsage mcp install / uninstall / status`.
//!
//! Lays down the MCP server files into the Portsage data dir, registers it in
//! the user's Claude Code config (`~/.claude.json` for global, or `./.mcp.json`
//! for project-scoped), installs the SKILL file, and patches the allowlist in
//! `~/.claude/settings.json`. Cross-platform: works the same on macOS and on
//! Linux dev boxes (the Linux server install doesn't need it - MCP is consumed
//! by Claude Code, which runs where the developer sits).
//!
//! The MCP source files (`server.py`, `pyproject.toml`, `uv.lock`, `SKILL.md`)
//! are embedded into the binary at compile time via `include_str!`, so a Linux
//! tarball install without the source tree still has everything it needs.

use std::path::{Path, PathBuf};
use std::process::Command;

// Embedded MCP server files. Paths are relative to this source file:
//   crates/portsage-cli/src/mcp.rs  ->  ../../../mcp/<file>
const SERVER_PY: &str = include_str!("../../../mcp/server.py");
const PYPROJECT_TOML: &str = include_str!("../../../mcp/pyproject.toml");
const UV_LOCK: &str = include_str!("../../../mcp/uv.lock");
const SKILL_MD: &str = include_str!("../../../mcp/SKILL.md");

/// Tools we add to the Claude Code allowlist on install. Must stay in sync with
/// the methods exposed by `src-tauri/src/socket.rs` and the tools defined in
/// `mcp/server.py`. Duplicated from `src-tauri/src/commands.rs::MCP_TOOL_PERMISSIONS`
/// on purpose: the CLI must not depend on the Tauri app crate.
const MCP_TOOL_PERMISSIONS: &[&str] = &[
    "mcp__portsage__list_all",
    "mcp__portsage__reserve_range",
    "mcp__portsage__register_port",
    "mcp__portsage__release_project",
    "mcp__portsage__remove_port",
    "mcp__portsage__list_unmanaged",
    "mcp__portsage__next_range",
    "mcp__portsage__get_config",
    "mcp__portsage__set_config",
    "mcp__portsage__scan_active",
    "mcp__portsage__kill_port",
    "mcp__portsage__kill_project",
    "mcp__portsage__open_in_browser",
    "mcp__portsage__find_project_by_path",
];

#[derive(Debug)]
pub enum McpError {
    Io(std::io::Error),
    Json(serde_json::Error),
    /// The user's Claude config file exists but is not valid JSON. We refuse
    /// to overwrite it rather than silently clobbering their setup.
    CorruptConfig {
        path: PathBuf,
        reason: String,
    },
    UvNotFound,
    UvSyncFailed {
        stderr: String,
    },
    NoHome,
}

impl From<std::io::Error> for McpError {
    fn from(e: std::io::Error) -> Self {
        McpError::Io(e)
    }
}

impl From<serde_json::Error> for McpError {
    fn from(e: serde_json::Error) -> Self {
        McpError::Json(e)
    }
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpError::Io(e) => write!(f, "io: {e}"),
            McpError::Json(e) => write!(f, "json: {e}"),
            McpError::CorruptConfig { path, reason } => write!(
                f,
                "{} appears to be corrupt and cannot be parsed: {}. Refusing to overwrite. \
                 Fix or back up the file manually before retrying.",
                path.display(),
                reason
            ),
            McpError::UvNotFound => write!(
                f,
                "uv is not installed. Install it: curl -LsSf https://astral.sh/uv/install.sh | sh"
            ),
            McpError::UvSyncFailed { stderr } => write!(f, "uv sync failed: {stderr}"),
            McpError::NoHome => write!(f, "cannot find home directory ($HOME is unset)"),
        }
    }
}

/// Where the embedded MCP files get written. The Tauri app's `get_mcp_dir`
/// historically used `dirs::config_dir()`; the CLI uses the data dir (the
/// SQLite DB lives there too), which matches the user's expectation on Linux
/// (`~/.local/share/portsage/mcp/`) and is identical on macOS.
pub fn install_dir() -> PathBuf {
    portsage_client::default_data_dir().join("mcp")
}

fn home_dir() -> Result<PathBuf, McpError> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(McpError::NoHome)
}

/// Read and parse a JSON file, returning `{}` if it doesn't exist. If the
/// file exists but is malformed, return [`McpError::CorruptConfig`] rather
/// than silently overwriting it - the Claude config file holds the user's
/// entire conversation index and we must never clobber it.
fn parse_existing_or_empty(path: &Path) -> Result<serde_json::Value, McpError> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(|e| McpError::CorruptConfig {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Atomic write via tmp + rename. Prevents leaving a half-written
/// `~/.claude.json` if the process is killed mid-write.
fn write_json_atomically(path: &Path, value: &serde_json::Value) -> Result<(), McpError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("portsage-tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
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

/// Run `uv sync --quiet` in `dir`. Returns [`McpError::UvNotFound`] if `uv` is
/// not on PATH so we can surface a clean install hint.
pub fn run_uv_sync(dir: &Path) -> Result<(), McpError> {
    let attempt = Command::new("uv")
        .args(["sync", "--quiet"])
        .current_dir(dir)
        .output();
    match attempt {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(McpError::UvSyncFailed {
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(McpError::UvNotFound),
        Err(e) => Err(McpError::Io(e)),
    }
}

/// Where to register the MCP server.
#[derive(Debug, Clone, Copy)]
pub enum Scope {
    /// Global: `~/.claude.json` (the default - applies to every Claude Code
    /// project).
    Global,
    /// Project-local: `./.mcp.json` next to the user's current working dir.
    Project,
}

impl Scope {
    fn config_path(&self) -> Result<PathBuf, McpError> {
        match self {
            Scope::Global => Ok(home_dir()?.join(".claude.json")),
            Scope::Project => Ok(std::env::current_dir()?.join(".mcp.json")),
        }
    }
}

fn mcp_server_entry(mcp_dir: &Path) -> serde_json::Value {
    serde_json::json!({
        "type": "stdio",
        "command": "uv",
        "args": ["--directory", mcp_dir.to_string_lossy(), "run", "python", "server.py"],
    })
}

/// Add the `portsage` MCP server entry under `mcpServers` in the target
/// Claude config file.
pub fn register_in_claude(scope: Scope, mcp_dir: &Path) -> Result<PathBuf, McpError> {
    let path = scope.config_path()?;
    let mut cfg = parse_existing_or_empty(&path)?;
    if !cfg.is_object() {
        cfg = serde_json::json!({});
    }
    cfg["mcpServers"]["portsage"] = mcp_server_entry(mcp_dir);
    write_json_atomically(&path, &cfg)?;
    Ok(path)
}

/// Inverse of [`register_in_claude`]. Returns `true` if the entry existed and
/// was removed.
pub fn unregister_from_claude(scope: Scope) -> Result<bool, McpError> {
    let path = scope.config_path()?;
    if !path.exists() {
        return Ok(false);
    }
    let mut cfg = parse_existing_or_empty(&path)?;
    let removed = cfg
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
        .map(|m| m.remove("portsage").is_some())
        .unwrap_or(false);
    if removed {
        write_json_atomically(&path, &cfg)?;
    }
    Ok(removed)
}

/// Copy the SKILL.md into `~/.claude/skills/portsage/SKILL.md`.
pub fn install_skill(mcp_dir: &Path) -> Result<PathBuf, McpError> {
    let dest = home_dir()?.join(".claude").join("skills").join("portsage");
    std::fs::create_dir_all(&dest)?;
    let dest_file = dest.join("SKILL.md");
    std::fs::copy(mcp_dir.join("SKILL.md"), &dest_file)?;
    Ok(dest_file)
}

/// Remove `~/.claude/skills/portsage/`. Returns `true` if it existed.
pub fn remove_skill() -> Result<bool, McpError> {
    let dir = home_dir()?.join(".claude").join("skills").join("portsage");
    if !dir.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(true)
}

/// Add the portsage MCP tools to the `permissions.allow` list in
/// `~/.claude/settings.json`. Idempotent.
pub fn add_permissions() -> Result<PathBuf, McpError> {
    let path = home_dir()?.join(".claude").join("settings.json");
    let mut settings = parse_existing_or_empty(&path)?;
    if !settings.is_object() {
        settings = serde_json::json!({});
    }

    let mut allow: Vec<String> = settings["permissions"]["allow"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    for tool in MCP_TOOL_PERMISSIONS {
        if !allow.iter().any(|s| s == tool) {
            allow.push((*tool).to_string());
        }
    }
    settings["permissions"]["allow"] =
        serde_json::Value::Array(allow.into_iter().map(serde_json::Value::String).collect());

    write_json_atomically(&path, &settings)?;
    Ok(path)
}

/// Remove the portsage MCP tools from `permissions.allow`. Other tools the
/// user added are preserved. Returns the number of entries removed.
pub fn remove_permissions() -> Result<usize, McpError> {
    let path = home_dir()?.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok(0);
    }
    let mut settings = parse_existing_or_empty(&path)?;
    let Some(arr) = settings["permissions"]["allow"].as_array().cloned() else {
        return Ok(0);
    };
    let before = arr.len();
    let kept: Vec<serde_json::Value> = arr
        .into_iter()
        .filter(|v| {
            v.as_str()
                .map(|s| !MCP_TOOL_PERMISSIONS.contains(&s))
                .unwrap_or(true)
        })
        .collect();
    let removed = before - kept.len();
    if removed == 0 {
        return Ok(0);
    }
    settings["permissions"]["allow"] = serde_json::Value::Array(kept);
    write_json_atomically(&path, &settings)?;
    Ok(removed)
}

/// Snapshot of the current MCP install state, for the `status` subcommand.
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
    let files_present = mcp_dir.join("server.py").exists();

    let uv_available = Command::new("uv")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let global = Scope::Global.config_path()?;
    let registered_global = read_has_portsage_server(&global)?;

    let project_cwd = Scope::Project.config_path()?;
    let registered_project_cwd = read_has_portsage_server(&project_cwd)?;

    let skill_installed = home_dir()?
        .join(".claude")
        .join("skills")
        .join("portsage")
        .join("SKILL.md")
        .exists();

    let allowlist_path = home_dir()?.join(".claude").join("settings.json");
    let allowlist_has_portsage = if allowlist_path.exists() {
        let v = parse_existing_or_empty(&allowlist_path)?;
        v["permissions"]["allow"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str())
                    .any(|s| s.starts_with("mcp__portsage__"))
            })
            .unwrap_or(false)
    } else {
        false
    };

    Ok(McpStatus {
        mcp_dir: mcp_dir.to_string_lossy().to_string(),
        files_present,
        uv_available,
        registered_global,
        registered_project_cwd,
        skill_installed,
        allowlist_has_portsage,
    })
}

fn read_has_portsage_server(path: &Path) -> Result<bool, McpError> {
    if !path.exists() {
        return Ok(false);
    }
    let v = parse_existing_or_empty(path)?;
    Ok(v["mcpServers"]["portsage"].is_object())
}

/// Convenience used by the CLI: extract files + `uv sync` + register +
/// install skill + add permissions. Returns the paths/dirs touched so the CLI
/// can print them.
pub struct InstallReport {
    pub mcp_dir: PathBuf,
    pub claude_config: PathBuf,
    pub skill_file: PathBuf,
    pub settings_file: PathBuf,
}

pub fn install(scope: Scope, skip_uv: bool) -> Result<InstallReport, McpError> {
    let mcp_dir = install_dir();
    write_embedded_files(&mcp_dir)?;
    if !skip_uv {
        run_uv_sync(&mcp_dir)?;
    }
    let claude_config = register_in_claude(scope, &mcp_dir)?;
    let skill_file = install_skill(&mcp_dir)?;
    let settings_file = add_permissions()?;
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
    let unregistered_global = unregister_from_claude(Scope::Global)?;
    let unregistered_project = unregister_from_claude(Scope::Project)?;
    let skill_removed = remove_skill()?;
    let permissions_removed = remove_permissions()?;
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
        // Server is non-empty - cheap canary against include_str! pointing at
        // the wrong path and ending up with empty strings.
        let py = std::fs::read_to_string(dir.path().join("server.py")).unwrap();
        assert!(py.contains("FastMCP"), "server.py should embed real code");
        let skill = std::fs::read_to_string(dir.path().join("SKILL.md")).unwrap();
        assert!(skill.contains("portsage"));
    }

    #[test]
    fn parse_existing_or_empty_returns_object_for_missing() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("missing.json");
        let v = parse_existing_or_empty(&p).unwrap();
        assert!(v.is_object());
    }

    #[test]
    fn parse_existing_or_empty_refuses_corrupt() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("corrupt.json");
        std::fs::write(&p, "{not json").unwrap();
        let err = parse_existing_or_empty(&p).unwrap_err();
        assert!(matches!(err, McpError::CorruptConfig { .. }));
    }

    #[test]
    fn write_json_atomically_creates_parent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a/b/c/file.json");
        write_json_atomically(&path, &serde_json::json!({"x": 1})).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["x"], 1);
    }

    // Exercise the JSON merge against a synthetic Claude config to be sure we
    // (a) preserve the user's other entries and (b) overwrite an existing
    // portsage entry rather than duplicating it.
    #[test]
    fn registers_portsage_under_mcp_servers_preserving_siblings() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("claude.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "version": "1.2.3",
                "mcpServers": {
                    "other-server": {"type": "stdio", "command": "x"}
                },
                "history": [{"id": 1}]
            }))
            .unwrap(),
        )
        .unwrap();

        let mut cfg = parse_existing_or_empty(&path).unwrap();
        cfg["mcpServers"]["portsage"] = mcp_server_entry(Path::new("/tmp/mcp"));
        write_json_atomically(&path, &cfg).unwrap();

        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk["version"], "1.2.3");
        assert!(on_disk["mcpServers"]["other-server"].is_object());
        assert_eq!(on_disk["mcpServers"]["portsage"]["command"], "uv");
        assert_eq!(on_disk["history"][0]["id"], 1);
    }

    #[test]
    fn remove_permissions_drops_only_portsage_entries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "permissions": {
                    "allow": [
                        "Bash(ls)",
                        "mcp__portsage__list_all",
                        "mcp__portsage__kill_port",
                        "Bash(grep)"
                    ]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        // Inline emulation of `remove_permissions` against an arbitrary path,
        // since the real fn reads `$HOME`.
        let mut settings = parse_existing_or_empty(&path).unwrap();
        let arr = settings["permissions"]["allow"]
            .as_array()
            .cloned()
            .unwrap();
        let before = arr.len();
        let kept: Vec<serde_json::Value> = arr
            .into_iter()
            .filter(|v| {
                v.as_str()
                    .map(|s| !MCP_TOOL_PERMISSIONS.contains(&s))
                    .unwrap_or(true)
            })
            .collect();
        let removed = before - kept.len();
        settings["permissions"]["allow"] = serde_json::Value::Array(kept);
        write_json_atomically(&path, &settings).unwrap();

        assert_eq!(removed, 2);
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let allow = on_disk["permissions"]["allow"].as_array().unwrap();
        let strs: Vec<&str> = allow.iter().filter_map(|v| v.as_str()).collect();
        assert_eq!(strs, vec!["Bash(ls)", "Bash(grep)"]);
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
