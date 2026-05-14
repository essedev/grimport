# Database Schema

The canonical SQLite schema lives in [`src-tauri/src/db.rs`](../src-tauri/src/db.rs) (function `Database::migrate`). This document mirrors it and must be updated in the same commit as any schema change.

## File location

| OS    | Path                                                     |
|-------|----------------------------------------------------------|
| macOS | `~/Library/Application Support/portsage/portsage.db`     |
| Linux | `$XDG_DATA_HOME/portsage/portsage.db` (default `~/.local/share/portsage/portsage.db`) |

Path resolution is centralised in [`src-tauri/src/paths.rs`](../src-tauri/src/paths.rs).

## Tables

### `projects`

```sql
CREATE TABLE projects (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    path        TEXT,
    range_start INTEGER NOT NULL,
    range_end   INTEGER NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

A project owns a contiguous port range `[range_start, range_end]`. Ranges never overlap - allocation is performed under a single mutex lock (see `Database::create_project` + `compute_next_range`) to defeat the read-modify-write race. The regression test `db.rs::concurrent_create_project_produces_no_overlapping_ranges` covers this.

`name` is unique; `path` is optional and points to a project directory on disk (used by `find_project_by_path` and "Open in Finder/Terminal").

### `ports`

```sql
CREATE TABLE ports (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    service    TEXT NOT NULL,
    port       INTEGER NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Each row is one `(project, service, port)` triple. `port` is globally unique. The `project_id` FK is informational - cleanup on project deletion is performed in code (`Database::delete_project` deletes rows from `ports` first, then from `projects`).

### `config`

```sql
CREATE TABLE config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO config (key, value) VALUES ('base_port', '4000');
INSERT OR IGNORE INTO config (key, value) VALUES ('range_size', '10');
```

Free-form key/value store. Only two keys are accepted today (`base_port`, `range_size`). Values are TEXT in SQLite and converted at the boundary - the wire type `ConfigSnapshot` keeps them as strings on purpose to avoid silent precision loss.

### `remote_backends` (multi-host, Phase 2)

```sql
CREATE TABLE remote_backends (
    id                   INTEGER PRIMARY KEY,
    name                 TEXT NOT NULL UNIQUE,
    ssh_alias            TEXT NOT NULL,
    remote_socket_path   TEXT NOT NULL,
    local_socket_path    TEXT NOT NULL,
    auto_forward_enabled INTEGER NOT NULL DEFAULT 0,
    created_at           TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Catalogue of remote Portsage servers the Mac UI knows about. Meaningful only on the Mac - on a Linux server this table stays empty (the server is itself a backend, not a consumer of remotes).

- `ssh_alias` resolves through the user's `~/.ssh/config`. Portsage does not duplicate ssh's host/user/port/identity logic.
- `remote_socket_path` is the absolute path of the socket on the remote box, e.g. `/run/portsage/portsage.sock`.
- `local_socket_path` is the local side of the `ssh -L unix:<local>:<remote>` forward; lives under `paths::state_dir()/<alias>.sock`.
- `auto_forward_enabled` (0/1) gates the Phase 3 auto-forward feature for this backend.

The row type re-exports `portsage_client::RemoteBackend` so the wire shape and the on-disk shape cannot drift.

### `forward_exclusions` (multi-host, Phase 3)

```sql
CREATE TABLE forward_exclusions (
    id         INTEGER PRIMARY KEY,
    backend_id INTEGER NOT NULL REFERENCES remote_backends(id),
    port       INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(backend_id, port)
);
```

Per-backend blocklist of ports the user does not want auto-forwarded (e.g. a port already in use locally by an unrelated process). Cascade on backend deletion is performed in code (`Database::delete_remote_backend` deletes exclusions first); the FK stays informational so error reporting matches the rest of the CRUD path. Regression test: `db.rs::delete_remote_backend_cascades_forward_exclusions`.

## Invariants

- **No overlapping ranges.** `compute_next_range` always uses `MAX(range_end) + 1` (or `base_port` for the empty case), and runs under the same lock as the insert.
- **Globally unique port numbers** across projects (the UNIQUE constraint on `ports.port` is the safety net; the application layer also validates that the port falls inside the project's range).
- **No hard delete cascades from SQL** - all cleanup happens in code so error messages stay consistent.
- **No soft delete.** Portsage uses hard deletes for projects and ports; `created_at` is the only timestamp tracked.

## Migrations

There is no migration framework. `Database::migrate` runs `CREATE TABLE IF NOT EXISTS` on every startup, so adding a new column requires either a fresh DB or a manual `ALTER TABLE` plus a guarded `IF NOT EXISTS` in the migration string. When that day comes, switch to a numbered migration helper rather than stacking `ALTER` statements in `migrate`.
