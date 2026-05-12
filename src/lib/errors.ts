// Translates raw backend errors (rusqlite messages, IO errors, custom Rust
// strings) into user-facing English. Unknown errors fall through unchanged
// so we never silently lose diagnostic info - the user just sees the raw
// text instead of a friendly version.

type Pattern = {
  match: RegExp;
  // Either a static string or a function that builds the message from regex
  // capture groups, when the original error contains useful values (port
  // numbers, ranges, names, etc).
  message: string | ((m: RegExpMatchArray) => string);
};

const PATTERNS: Pattern[] = [
  // SQLite UNIQUE constraint violations - the most common user-facing errors.
  {
    match: /UNIQUE constraint failed:\s*projects\.name/i,
    message: "A project with this name already exists.",
  },
  {
    match: /UNIQUE constraint failed:\s*ports\.port/i,
    message: "This port is already assigned to another project.",
  },

  // Custom range validation from db.rs::add_port - already readable but
  // we rephrase for consistency with the rest of the messages.
  {
    match: /port (\d+) is outside project range (\d+)-(\d+)/i,
    message: (m) =>
      `Port ${m[1]} is outside this project's range (${m[2]}-${m[3]}).`,
  },

  // SQLite contention. Rare on a single-user app but possible during
  // export/import or when the MCP socket is hammered.
  {
    match: /database is locked/i,
    message: "The database is busy. Please try again in a moment.",
  },

  // Filesystem errors from export/import and MCP install.
  {
    match: /No such file or directory|os error 2/i,
    message: "File not found.",
  },

  // Socket-layer errors from the MCP path - unlikely from the UI but cheap
  // to cover in case a Tauri command starts surfacing them.
  {
    match: /project '([^']+)' not found/i,
    message: (m) => `Project "${m[1]}" not found.`,
  },

  // --- Remote backend / SSH tunnel errors (Phase 2) ---
  //
  // BackendError values arrive as `to_string()`'d strings in the Tauri
  // command Err channel. The Rust-side variant tags are stable, so match on
  // them; the trailing detail is the raw ssh stderr or socket message.
  //
  // These patterns are intentionally listed *before* the generic
  // "Permission denied" filesystem rule below: SSH's "Permission denied
  // (publickey)" is specific enough that it deserves its own message rather
  // than being collapsed into the file-permissions wording.
  {
    match: /unknown remote backend:\s*(.+)/i,
    message: (m) =>
      `Remote backend "${m[1].trim()}" is not configured. Open Settings > Remote backends to add it.`,
  },
  {
    match: /Could not resolve hostname/i,
    message:
      "SSH could not resolve the remote host. Check the alias in ~/.ssh/config (Host entry) and that the network is reachable.",
  },
  {
    match: /Host key verification failed/i,
    message:
      "SSH host key verification failed. Connect once from a terminal (e.g. `ssh <alias>`) to accept the host key, then retry.",
  },
  {
    match: /Permission denied \(publickey/i,
    message:
      "SSH key authentication failed. Check ssh-agent is running and your key is loaded, or add `IdentityFile` to the Host entry.",
  },
  {
    match: /tunnel for '([^']+)' did not become reachable/i,
    message: (m) =>
      `Tunnel for "${m[1]}" did not come up in time. Check that the remote portsage-server is running and your SSH config reaches it.`,
  },
  {
    match: /backend closed connection/i,
    message:
      "Tunnel is open but the remote side closed the connection. The remote portsage-server may not be running.",
  },
  {
    match: /failed to spawn ssh/i,
    message:
      "Could not run the ssh client. Is OpenSSH installed and on PATH?",
  },

  // Generic file-permission errors. Kept *after* the SSH-specific patterns
  // so "Permission denied (publickey)" doesn't get collapsed into the
  // file-permissions wording.
  {
    match: /Permission denied|os error 13/i,
    message: "Permission denied. Check the file permissions and try again.",
  },
];

function normalize(raw: unknown): string {
  if (typeof raw === "string") return raw;
  if (raw instanceof Error) return raw.message;
  try {
    return JSON.stringify(raw);
  } catch {
    return "Unknown error";
  }
}

export function humanizeError(raw: unknown): string {
  const text = normalize(raw);
  for (const { match, message } of PATTERNS) {
    const m = text.match(match);
    if (m) return typeof message === "string" ? message : message(m);
  }
  // Fallback: hand back the raw text. Better to show something technical
  // than to swallow it - we'd lose all diagnostic value.
  return text;
}
