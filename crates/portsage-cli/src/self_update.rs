//! `portsage self-update`.
//!
//! Compares the running binary's version against the latest GitHub release,
//! and on macOS where Homebrew is detected runs `brew upgrade --cask portsage`
//! after confirmation. On Linux (where the install is a packaged tarball
//! managed by `packaging/linux/install.sh` and lives in `/usr/local/bin/`),
//! we don't attempt to clobber the installed binary - we just print the
//! download URL and the steps to take. Self-replacement under sudo with a
//! systemd unit holding the binary open is too risky for a feature whose only
//! upside is one fewer command.
//!
//! Network access uses `curl` (universally available, no TLS dep to add).

use std::process::Command;

pub const REPO: &str = "essedev/portsage";
pub const LATEST_API_URL: &str = "https://api.github.com/repos/essedev/portsage/releases/latest";
pub const RELEASES_PAGE_URL: &str = "https://github.com/essedev/portsage/releases/latest";

#[derive(Debug)]
pub enum SelfUpdateError {
    Io(std::io::Error),
    Json(serde_json::Error),
    CurlMissing,
    CurlFailed { stderr: String },
    ParseFailed { reason: String },
    BrewFailed { stderr: String },
    Aborted,
}

impl From<std::io::Error> for SelfUpdateError {
    fn from(e: std::io::Error) -> Self {
        SelfUpdateError::Io(e)
    }
}

impl From<serde_json::Error> for SelfUpdateError {
    fn from(e: serde_json::Error) -> Self {
        SelfUpdateError::Json(e)
    }
}

impl std::fmt::Display for SelfUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelfUpdateError::Io(e) => write!(f, "io: {e}"),
            SelfUpdateError::Json(e) => write!(f, "parse: {e}"),
            SelfUpdateError::CurlMissing => write!(
                f,
                "curl is required for self-update but was not found on PATH"
            ),
            SelfUpdateError::CurlFailed { stderr } => write!(f, "curl failed: {stderr}"),
            SelfUpdateError::ParseFailed { reason } => {
                write!(f, "could not parse latest release ({reason})")
            }
            SelfUpdateError::BrewFailed { stderr } => write!(f, "brew upgrade failed: {stderr}"),
            SelfUpdateError::Aborted => write!(f, "aborted"),
        }
    }
}

/// Strip a leading `v` from a tag like `v0.11.0`.
fn strip_v(s: &str) -> &str {
    s.strip_prefix('v').unwrap_or(s)
}

/// Parse a dotted version like `0.11.0` into a sortable tuple of u64s. Returns
/// `None` for empty / non-numeric components so callers can fall back to a
/// string compare instead of pretending we did a real comparison.
pub fn parse_version(s: &str) -> Option<Vec<u64>> {
    let parts: Vec<&str> = strip_v(s).split('.').collect();
    if parts.is_empty() {
        return None;
    }
    parts.iter().map(|p| p.parse::<u64>().ok()).collect()
}

#[derive(Debug, PartialEq, Eq)]
pub enum VersionCmp {
    UpToDate,
    Outdated,
    Ahead,
    /// Either version isn't parseable as dotted numbers. The CLI surfaces this
    /// as "unable to compare" and lets the user judge.
    Unknown,
}

pub fn compare_versions(current: &str, latest: &str) -> VersionCmp {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => match c.cmp(&l) {
            std::cmp::Ordering::Equal => VersionCmp::UpToDate,
            std::cmp::Ordering::Less => VersionCmp::Outdated,
            std::cmp::Ordering::Greater => VersionCmp::Ahead,
        },
        _ => VersionCmp::Unknown,
    }
}

/// Shell out to `curl` to fetch the latest release tag from the GitHub API.
pub fn fetch_latest_version() -> Result<String, SelfUpdateError> {
    let attempt = Command::new("curl")
        .args([
            "-fsSL",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: portsage-cli",
            LATEST_API_URL,
        ])
        .output();
    let out = match attempt {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(SelfUpdateError::CurlMissing);
        }
        Err(e) => return Err(SelfUpdateError::Io(e)),
    };
    if !out.status.success() {
        return Err(SelfUpdateError::CurlFailed {
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    let body = String::from_utf8(out.stdout).map_err(|e| SelfUpdateError::ParseFailed {
        reason: format!("non-utf8 body: {e}"),
    })?;
    extract_tag_name(&body)
}

pub fn extract_tag_name(body: &str) -> Result<String, SelfUpdateError> {
    let v: serde_json::Value = serde_json::from_str(body)?;
    let tag = v["tag_name"]
        .as_str()
        .ok_or_else(|| SelfUpdateError::ParseFailed {
            reason: "tag_name missing from response".into(),
        })?;
    Ok(strip_v(tag).to_string())
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Is `brew` on PATH? Used to decide whether to offer the auto-upgrade path
/// on macOS.
pub fn has_brew() -> bool {
    Command::new("brew")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `brew update && brew upgrade --cask portsage`. We intentionally do the
/// `update` first so the cask's `version`/`sha256` reflect what was just
/// published - otherwise a fresh release isn't visible to brew until the next
/// auto-update tick.
pub fn brew_upgrade() -> Result<(), SelfUpdateError> {
    let update = Command::new("brew").arg("update").status()?;
    if !update.success() {
        return Err(SelfUpdateError::BrewFailed {
            stderr: "brew update returned non-zero".into(),
        });
    }
    let upgrade = Command::new("brew")
        .args(["upgrade", "--cask", "portsage"])
        .status()?;
    if !upgrade.success() {
        return Err(SelfUpdateError::BrewFailed {
            stderr: "brew upgrade --cask portsage returned non-zero".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_strips_v_and_handles_three_parts() {
        assert_eq!(parse_version("v0.11.0"), Some(vec![0, 11, 0]));
        assert_eq!(parse_version("0.11.0"), Some(vec![0, 11, 0]));
        assert_eq!(parse_version("1.0"), Some(vec![1, 0]));
    }

    #[test]
    fn parse_version_rejects_garbage() {
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("abc"), None);
        assert_eq!(parse_version("0.x.0"), None);
    }

    #[test]
    fn compare_versions_basic_orderings() {
        assert_eq!(compare_versions("0.11.0", "0.11.0"), VersionCmp::UpToDate);
        assert_eq!(compare_versions("0.10.0", "0.11.0"), VersionCmp::Outdated);
        assert_eq!(compare_versions("0.12.0", "0.11.0"), VersionCmp::Ahead);
        assert_eq!(compare_versions("0.9.0", "0.10.0"), VersionCmp::Outdated);
    }

    #[test]
    fn compare_versions_unknown_when_not_numeric() {
        assert_eq!(compare_versions("dev", "0.11.0"), VersionCmp::Unknown);
        assert_eq!(compare_versions("0.11.0", "v?"), VersionCmp::Unknown);
    }

    #[test]
    fn extract_tag_name_strips_v() {
        let body = r#"{"tag_name": "v1.2.3", "name": "Release 1.2.3"}"#;
        assert_eq!(extract_tag_name(body).unwrap(), "1.2.3");
    }

    #[test]
    fn extract_tag_name_errors_when_missing() {
        let body = r#"{"name": "no tag here"}"#;
        let err = extract_tag_name(body).unwrap_err();
        assert!(matches!(err, SelfUpdateError::ParseFailed { .. }));
    }

    #[test]
    fn current_version_matches_workspace() {
        // Cheap canary: the workspace version is sourced from Cargo.toml, so
        // env!() must yield a non-empty string.
        assert!(!current_version().is_empty());
        assert!(current_version().chars().next().unwrap().is_ascii_digit());
    }
}
