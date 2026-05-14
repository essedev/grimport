use serde::{Deserialize, Serialize};

/// A port row inside a project, enriched with live status from the host scanner.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// A project with its assigned range and the live status of every port.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectStatus {
    pub id: i64,
    pub name: String,
    pub path: Option<String>,
    pub range_start: i64,
    pub range_end: i64,
    pub created_at: String,
    pub ports: Vec<PortStatus>,
}

/// A TCP port currently in LISTEN on the host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivePort {
    pub port: i64,
    pub process: String,
    pub pid: i64,
}

/// Result of a kill attempt against a single PID.
///
/// `DockerStopped` and `DockerError` are emitted when the listening PID
/// belongs to a Docker port-forwarding proxy (`com.docker.backend`,
/// `vpnkit`, `docker-proxy`): we cannot kill the proxy without nuking
/// every other container's published port, so the action resolves the
/// host port to its container and calls `docker stop` instead.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KillOutcome {
    Terminated,
    Killed,
    NotActive,
    PermissionDenied,
    DockerStopped,
    DockerError,
}

/// One entry returned by `kill_project`: the registered port that was active
/// and the outcome of attempting to kill its process.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct KillEntry {
    pub port: i64,
    pub outcome: KillOutcome,
}

/// Inclusive range bounds returned by `next_range`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RangeBounds {
    pub range_start: i64,
    pub range_end: i64,
}

/// Current global configuration snapshot. Values are returned as strings to
/// match the SQLite column type (the backend stores everything as TEXT).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigSnapshot {
    pub base_port: String,
    pub range_size: String,
}

/// A remote-backend catalogue row, returned by `get_remote_backend`. Exists
/// in the wire types so the CLI can ask the Mac socket for a backend's
/// `local_socket_path` and then point its own `Client` at that path. The
/// CLI does not open tunnels itself; that stays with the Mac UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteBackend {
    pub id: i64,
    pub name: String,
    pub ssh_alias: String,
    pub remote_socket_path: String,
    pub local_socket_path: String,
    pub auto_forward_enabled: bool,
    pub created_at: String,
}
