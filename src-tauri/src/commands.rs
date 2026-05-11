use crate::db::{Database, ProjectWithPorts};
use crate::scanner::{self, scan_active_ports, ActivePort};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tauri::{Manager, State};

/// Read and parse a JSON file at `path`. If the file does not exist, returns
/// an empty object. If the file exists but is malformed, returns Err with a
/// clear "refusing to overwrite" message. This is the **safety-critical**
/// helper used by `install_mcp` before merging into the user's `~/.claude.json`
/// and `~/.claude/settings.json`: falling back to `{}` on parse failure would
/// silently destroy the user's entire editor config.
fn parse_existing_or_empty(path: &Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| {
        format!(
            "{} appears to be corrupt and cannot be parsed: {}. Refusing to overwrite. \
             Fix or back up the file manually before retrying.",
            path.display(),
            e
        )
    })
}

#[derive(Debug, Serialize)]
pub struct PortStatus {
    pub id: i64,
    pub project_id: i64,
    pub service: String,
    pub port: i64,
    pub active: bool,
    pub process: Option<String>,
    pub pid: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectStatus {
    pub id: i64,
    pub name: String,
    pub path: Option<String>,
    pub range_start: i64,
    pub range_end: i64,
    pub created_at: String,
    pub ports: Vec<PortStatus>,
}

fn enrich_with_status(
    projects: Vec<ProjectWithPorts>,
    active_ports: &[ActivePort],
) -> Vec<ProjectStatus> {
    use std::collections::HashMap;
    let port_map: HashMap<i64, &ActivePort> = active_ports
        .iter()
        .map(|ap| (ap.port, ap))
        .collect();

    projects
        .into_iter()
        .map(|pwp| ProjectStatus {
            id: pwp.project.id,
            name: pwp.project.name,
            path: pwp.project.path,
            range_start: pwp.project.range_start,
            range_end: pwp.project.range_end,
            created_at: pwp.project.created_at,
            ports: pwp
                .ports
                .into_iter()
                .map(|p| {
                    let ap = port_map.get(&p.port);
                    PortStatus {
                        active: ap.is_some(),
                        process: ap.map(|a| a.process.clone()),
                        pid: ap.map(|a| a.pid),
                        id: p.id,
                        project_id: p.project_id,
                        service: p.service,
                        port: p.port,
                        created_at: p.created_at,
                    }
                })
                .collect(),
        })
        .collect()
}

#[tauri::command]
pub fn list_projects(db: State<Arc<Database>>) -> Result<Vec<ProjectStatus>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let active = scanner::scan_active_ports_detailed();
    Ok(enrich_with_status(projects, &active))
}

#[tauri::command]
pub fn create_project(
    db: State<Arc<Database>>,
    name: String,
    path: Option<String>,
) -> Result<ProjectStatus, String> {
    let project = db
        .create_project(&name, path.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(ProjectStatus {
        id: project.id,
        name: project.name,
        path: project.path,
        range_start: project.range_start,
        range_end: project.range_end,
        created_at: project.created_at,
        ports: Vec::new(),
    })
}

#[tauri::command]
pub fn delete_project(db: State<Arc<Database>>, id: i64) -> Result<(), String> {
    db.delete_project(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_port(
    db: State<Arc<Database>>,
    project_id: i64,
    service: String,
    port: i64,
) -> Result<PortStatus, String> {
    let p = db
        .add_port(project_id, &service, port)
        .map_err(|e| e.to_string())?;
    let active = scan_active_ports();
    Ok(PortStatus {
        active: active.contains(&p.port),
        process: None,
        pid: None,
        id: p.id,
        project_id: p.project_id,
        service: p.service,
        port: p.port,
        created_at: p.created_at,
    })
}

#[tauri::command]
pub fn remove_port(db: State<Arc<Database>>, id: i64) -> Result<(), String> {
    db.remove_port(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn scan_ports() -> Vec<i64> {
    let mut ports: Vec<i64> = scan_active_ports().into_iter().collect();
    ports.sort();
    ports
}

#[tauri::command]
pub fn list_unmanaged_ports(db: State<Arc<Database>>) -> Result<Vec<ActivePort>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let registered: HashSet<i64> = projects
        .iter()
        .flat_map(|p| p.ports.iter().map(|port| port.port))
        .collect();
    let mut unmanaged = scanner::scan_unmanaged_ports(&registered);
    unmanaged.sort_by_key(|p| p.port);
    Ok(unmanaged)
}

#[tauri::command]
pub fn get_next_range(db: State<Arc<Database>>) -> Result<(i64, i64), String> {
    db.next_available_range().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_in_finder(path: String) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_in_terminal(path: String) -> Result<(), String> {
    std::process::Command::new("open")
        .args(["-a", "Terminal", &path])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_in_browser(port: i64) -> Result<(), String> {
    if !(1..=65535).contains(&port) {
        return Err(format!("invalid port: {port}"));
    }
    let url = format!("http://localhost:{port}");
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum KillOutcome {
    /// Process exited after SIGTERM within the grace period.
    Terminated,
    /// Process survived SIGTERM and was force-killed with SIGKILL.
    Killed,
    /// No process found listening on the port at kill time.
    NotActive,
    /// kill(2) returned EPERM - the process belongs to another user.
    PermissionDenied,
}

/// 2 seconds is the empirical sweet spot: enough for Postgres-class daemons
/// to flush and exit cleanly, short enough that the UI doesn't feel stuck.
const KILL_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

fn is_permission_error(stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    s.contains("operation not permitted") || s.contains("not permitted")
}

/// Send SIGTERM, wait for the grace period, escalate to SIGKILL if needed.
/// Errors from `kill` are mapped to KillOutcome rather than bubbled - the
/// frontend only cares about the final state of the port, not which syscall
/// returned what.
async fn kill_pid_with_escalation(pid: i64) -> KillOutcome {
    let term = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output();
    match term {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            if is_permission_error(&stderr) {
                return KillOutcome::PermissionDenied;
            }
            // "No such process" - already gone between scan and SIGTERM.
            return KillOutcome::NotActive;
        }
        Err(_) => return KillOutcome::PermissionDenied,
    }

    tokio::time::sleep(KILL_GRACE).await;

    // kill -0 probes existence without delivering a signal.
    let probe = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output();
    let still_alive = matches!(probe, Ok(o) if o.status.success());
    if !still_alive {
        return KillOutcome::Terminated;
    }

    let force = std::process::Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .output();
    match force {
        Ok(o) if o.status.success() => KillOutcome::Killed,
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            if is_permission_error(&stderr) {
                KillOutcome::PermissionDenied
            } else {
                // Died between probe and SIGKILL - count as terminated.
                KillOutcome::Terminated
            }
        }
        Err(_) => KillOutcome::PermissionDenied,
    }
}

#[tauri::command]
pub async fn kill_port(port: i64) -> Result<KillOutcome, String> {
    // Fresh scan: the PID cached on the frontend can be obsolete by seconds.
    let active = scanner::scan_active_ports_detailed();
    let Some(target) = active.into_iter().find(|p| p.port == port) else {
        return Ok(KillOutcome::NotActive);
    };
    Ok(kill_pid_with_escalation(target.pid).await)
}

#[tauri::command]
pub async fn kill_project(
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<Vec<(i64, KillOutcome)>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let registered: HashSet<i64> = projects
        .iter()
        .find(|p| p.project.id == project_id)
        .ok_or_else(|| format!("project {project_id} not found"))?
        .ports
        .iter()
        .map(|p| p.port)
        .collect();

    let active: Vec<ActivePort> = scanner::scan_active_ports_detailed()
        .into_iter()
        .filter(|ap| registered.contains(&ap.port))
        .collect();

    // Kills run concurrently: with N active ports a sequential loop would
    // take N * KILL_GRACE seconds (e.g. 5 ports = 10s of UI spinner). The
    // grace period is the dominant cost, so parallelism is essentially free.
    let handles: Vec<_> = active
        .into_iter()
        .map(|ap| tokio::spawn(async move { (ap.port, kill_pid_with_escalation(ap.pid).await) }))
        .collect();

    let mut results = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(r) = h.await {
            results.push(r);
        }
    }
    results.sort_by_key(|(port, _)| *port);
    Ok(results)
}

#[tauri::command]
pub fn get_config(db: State<Arc<Database>>) -> Result<serde_json::Value, String> {
    let base_port = db.get_config("base_port").map_err(|e| e.to_string())?;
    let range_size = db.get_config("range_size").map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "base_port": base_port,
        "range_size": range_size,
    }))
}

#[tauri::command]
pub fn set_config(
    db: State<Arc<Database>>,
    key: String,
    value: String,
) -> Result<(), String> {
    db.set_config(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_data(db: State<Arc<Database>>, dest_path: String) -> Result<(), String> {
    let db_path = Database::db_path();

    if !db_path.exists() {
        return Err("Database not found".into());
    }

    // Create a zip with the db
    let file = std::fs::File::create(&dest_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Add database
    zip.start_file("portsage.db", options).map_err(|e| e.to_string())?;
    let db_bytes = std::fs::read(&db_path).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zip, &db_bytes).map_err(|e| e.to_string())?;

    // Add config as JSON
    zip.start_file("config.json", options).map_err(|e| e.to_string())?;
    let base_port = db.get_config("base_port").unwrap_or("4000".into());
    let range_size = db.get_config("range_size").unwrap_or("10".into());
    let config = serde_json::json!({
        "base_port": base_port,
        "range_size": range_size,
    });
    let config_bytes = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zip, config_bytes.as_bytes()).map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn import_data(source_path: String) -> Result<(), String> {
    let db_path = Database::db_path();

    let file = std::fs::File::open(&source_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    // Extract database
    let mut db_file = archive.by_name("portsage.db").map_err(|e| e.to_string())?;
    let mut db_bytes = Vec::new();
    std::io::Read::read_to_end(&mut db_file, &mut db_bytes).map_err(|e| e.to_string())?;
    drop(db_file);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&db_path, &db_bytes).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn get_mcp_dir(app: tauri::AppHandle) -> Result<String, String> {
    let config_mcp = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("portsage")
        .join("mcp");

    // Always prefer bundled resources when available, overwriting any existing files in
    // the config dir. This is critical: it lets app upgrades (e.g. brew upgrade) propagate
    // fixes to server.py / SKILL.md to users who already have a copy from a previous install,
    // instead of leaving them stuck with stale files.
    let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
    let bundled_mcp = resource_dir.join("mcp");
    if bundled_mcp.join("server.py").exists() {
        std::fs::create_dir_all(&config_mcp).map_err(|e| e.to_string())?;
        for file in &["server.py", "pyproject.toml", "SKILL.md"] {
            let src = bundled_mcp.join(file);
            let dst = config_mcp.join(file);
            if src.exists() {
                std::fs::copy(&src, &dst).map_err(|e| e.to_string())?;
            }
        }
        return Ok(config_mcp.to_string_lossy().to_string());
    }

    // No bundled resources (dev mode without resource_dir, or unusual install): if the
    // user already has files in the config dir, use them as-is.
    if config_mcp.join("server.py").exists() {
        return Ok(config_mcp.to_string_lossy().to_string());
    }

    // Dev mode: resolve from executable location, walking up to find a sibling mcp dir.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dev_mcp = exe
        .ancestors()
        .find_map(|p| {
            let candidate = p.join("mcp").join("server.py");
            candidate.exists().then(|| p.join("mcp"))
        });

    if let Some(path) = dev_mcp {
        return Ok(path.to_string_lossy().to_string());
    }

    Err("MCP server files not found".into())
}

#[tauri::command]
pub fn check_mcp_installed() -> Result<bool, String> {
    let claude_json = dirs::home_dir()
        .ok_or("cannot find home dir")?
        .join(".claude.json");

    if !claude_json.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&claude_json).map_err(|e| e.to_string())?;
    let parsed: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;

    Ok(parsed["mcpServers"]["portsage"].is_object())
}

#[tauri::command]
pub fn install_mcp(mcp_dir: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("cannot find home dir")?;
    let mcp_dir = std::path::PathBuf::from(&mcp_dir);

    // 1. Write MCP server config to ~/.claude.json
    let claude_json_path = home.join(".claude.json");
    let mut claude_json = parse_existing_or_empty(&claude_json_path)?;

    let mcp_dir_str = mcp_dir.to_string_lossy().to_string();
    claude_json["mcpServers"]["portsage"] = serde_json::json!({
        "type": "stdio",
        "command": "uv",
        "args": ["--directory", mcp_dir_str, "run", "python", "server.py"]
    });

    std::fs::write(
        &claude_json_path,
        serde_json::to_string_pretty(&claude_json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    // 2. Install skill
    let skill_dir = home.join(".claude").join("skills").join("portsage");
    std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;

    let skill_source = mcp_dir.join("SKILL.md");
    let skill_dest = skill_dir.join("SKILL.md");
    std::fs::copy(&skill_source, &skill_dest).map_err(|e| e.to_string())?;

    // 3. Add tool permissions to ~/.claude/settings.json (same parse-or-bail policy as above)
    let settings_path = home.join(".claude").join("settings.json");
    let mut settings = parse_existing_or_empty(&settings_path)?;

    let tools = vec![
        "mcp__portsage__list_all",
        "mcp__portsage__reserve_range",
        "mcp__portsage__register_port",
        "mcp__portsage__release_project",
        "mcp__portsage__scan_active",
    ];

    let allow = settings["permissions"]["allow"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut allow_set: Vec<String> = allow
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    for tool in &tools {
        if !allow_set.contains(&tool.to_string()) {
            allow_set.push(tool.to_string());
        }
    }
    settings["permissions"]["allow"] =
        serde_json::Value::Array(allow_set.into_iter().map(serde_json::Value::String).collect());

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn uninstall_mcp() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("cannot find home dir")?;

    // 1. Remove from ~/.claude.json
    let claude_json_path = home.join(".claude.json");
    if claude_json_path.exists() {
        let content = std::fs::read_to_string(&claude_json_path).map_err(|e| e.to_string())?;
        let mut parsed: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| e.to_string())?;
        if let Some(servers) = parsed["mcpServers"].as_object_mut() {
            servers.remove("portsage");
        }
        std::fs::write(
            &claude_json_path,
            serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }

    // 2. Remove skill
    let skill_dir = home.join(".claude").join("skills").join("portsage");
    let _ = std::fs::remove_dir_all(&skill_dir);

    // 3. Remove permissions
    let settings_path = home.join(".claude").join("settings.json");
    if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path).map_err(|e| e.to_string())?;
        let mut settings: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| e.to_string())?;
        if let Some(allow) = settings["permissions"]["allow"].as_array_mut() {
            allow.retain(|v| {
                v.as_str()
                    .map(|s| !s.starts_with("mcp__portsage__"))
                    .unwrap_or(true)
            });
        }
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Port, Project, ProjectWithPorts};

    fn project(id: i64, name: &str, range: (i64, i64)) -> Project {
        Project {
            id,
            name: name.into(),
            path: None,
            range_start: range.0,
            range_end: range.1,
            created_at: "now".into(),
        }
    }

    fn port(id: i64, project_id: i64, service: &str, port: i64) -> Port {
        Port {
            id,
            project_id,
            service: service.into(),
            port,
            created_at: "now".into(),
        }
    }

    fn active(port: i64, process: &str) -> ActivePort {
        ActivePort {
            port,
            process: process.into(),
            pid: 999,
        }
    }

    #[test]
    fn enrich_marks_active_ports_and_attaches_process_name() {
        let projects = vec![ProjectWithPorts {
            project: project(1, "alpha", (4000, 4009)),
            ports: vec![port(10, 1, "vite", 4000), port(11, 1, "api", 4001)],
        }];
        // 4000 is active as "node", 4001 is not in the active list.
        let active_list = vec![active(4000, "node")];

        let result = enrich_with_status(projects, &active_list);

        assert_eq!(result.len(), 1);
        let p = &result[0];
        assert_eq!(p.name, "alpha");
        assert_eq!(p.ports.len(), 2);

        let vite = p.ports.iter().find(|p| p.service == "vite").unwrap();
        assert!(vite.active);
        assert_eq!(vite.process.as_deref(), Some("node"));
        // PID must travel alongside process - the UI uses it to confirm kill targets.
        assert_eq!(vite.pid, Some(999));

        let api = p.ports.iter().find(|p| p.service == "api").unwrap();
        assert!(!api.active);
        assert!(api.process.is_none());
        assert!(api.pid.is_none());
    }

    #[test]
    fn enrich_with_no_active_ports_marks_everything_inactive() {
        let projects = vec![ProjectWithPorts {
            project: project(1, "alpha", (4000, 4009)),
            ports: vec![port(10, 1, "vite", 4000)],
        }];
        let result = enrich_with_status(projects, &[]);
        assert!(!result[0].ports[0].active);
        assert!(result[0].ports[0].process.is_none());
        assert!(result[0].ports[0].pid.is_none());
    }

    #[test]
    fn enrich_active_port_outside_any_project_is_ignored() {
        // 9999 is active but not registered to any project. enrich_with_status
        // only annotates registered ports - unmanaged ports go through a
        // different code path - so this should not affect the result.
        let projects = vec![ProjectWithPorts {
            project: project(1, "alpha", (4000, 4009)),
            ports: vec![port(10, 1, "vite", 4000)],
        }];
        let active_list = vec![active(9999, "node")];

        let result = enrich_with_status(projects, &active_list);
        assert_eq!(result[0].ports.len(), 1);
        assert!(!result[0].ports[0].active);
    }

    // --- parse_existing_or_empty ---

    #[test]
    fn parse_existing_or_empty_returns_empty_object_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.json");
        let result = parse_existing_or_empty(&path).unwrap();
        assert_eq!(result, serde_json::json!({}));
    }

    #[test]
    fn parse_existing_or_empty_parses_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"mcpServers": {"foo": {"command": "bar"}}}"#).unwrap();
        let result = parse_existing_or_empty(&path).unwrap();
        assert_eq!(result["mcpServers"]["foo"]["command"], "bar");
    }

    #[test]
    fn parse_existing_or_empty_bails_on_malformed_json() {
        // This is the safety-critical case: if the user's claude.json is broken,
        // we MUST refuse to overwrite it - falling back to {} would destroy
        // all their other MCP servers and editor settings.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.json");
        std::fs::write(&path, "{ this is not valid json").unwrap();
        let err = parse_existing_or_empty(&path).unwrap_err();
        assert!(
            err.contains("appears to be corrupt"),
            "expected 'corrupt' message, got: {err}",
        );
        assert!(
            err.contains("Refusing to overwrite"),
            "expected refusal message, got: {err}",
        );
        assert!(
            err.contains(&path.display().to_string()),
            "expected the path to be mentioned in the error, got: {err}",
        );
    }

    #[test]
    fn parse_existing_or_empty_handles_empty_file_as_corrupt() {
        // An empty file is not valid JSON. It must be treated as corrupt
        // (refusal), not silently turned into {}.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.json");
        std::fs::write(&path, "").unwrap();
        let err = parse_existing_or_empty(&path).unwrap_err();
        assert!(err.contains("appears to be corrupt"));
    }

    #[test]
    fn parse_existing_or_empty_accepts_empty_object() {
        // {} on disk is valid and should round-trip.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty-obj.json");
        std::fs::write(&path, "{}").unwrap();
        let result = parse_existing_or_empty(&path).unwrap();
        assert_eq!(result, serde_json::json!({}));
    }

    // --- is_permission_error ---

    #[test]
    fn is_permission_error_matches_macos_and_linux_phrasing() {
        // macOS bash: "kill: (12345) - Operation not permitted"
        assert!(is_permission_error("kill: (12345) - Operation not permitted"));
        // bsd kill: "kill: 12345: Operation not permitted"
        assert!(is_permission_error("kill: 12345: Operation not permitted"));
        // Case-insensitive match defends against shell capitalization drift.
        assert!(is_permission_error("OPERATION NOT PERMITTED"));
    }

    #[test]
    fn is_permission_error_rejects_other_failures() {
        assert!(!is_permission_error("kill: (12345) - No such process"));
        assert!(!is_permission_error(""));
    }

    #[test]
    fn enrich_preserves_project_order_and_metadata() {
        let projects = vec![
            ProjectWithPorts {
                project: Project {
                    id: 1,
                    name: "alpha".into(),
                    path: Some("/tmp/alpha".into()),
                    range_start: 4000,
                    range_end: 4009,
                    created_at: "t1".into(),
                },
                ports: vec![],
            },
            ProjectWithPorts {
                project: project(2, "bravo", (4010, 4019)),
                ports: vec![],
            },
        ];
        let result = enrich_with_status(projects, &[]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "alpha");
        assert_eq!(result[0].path.as_deref(), Some("/tmp/alpha"));
        assert_eq!(result[0].range_start, 4000);
        assert_eq!(result[0].range_end, 4009);
        assert_eq!(result[1].name, "bravo");
    }
}
