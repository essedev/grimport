import { useEffect, useState } from "react";
import { Trash2, Play, Plus, X, ChevronDown, ChevronRight } from "lucide-react";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIInput } from "@/components/ui/UIInput";
import { UIDivider } from "@/components/ui/UIDivider";
import { useConfirm } from "@/lib/dialog";
import { useToast } from "@/lib/toast";
import { humanizeError } from "@/lib/errors";
import * as cmd from "@/lib/commands";
import type {
  ForwardExclusion,
  RemoteBackend,
  RemoteBackendForm,
  TunnelState,
  BackendTarget,
} from "@/lib/types";

interface RemoteBackendsPanelProps {
  remotes: RemoteBackend[];
  tunnels: Record<string, TunnelState>;
  /** Current backend target - so we can warn before deleting the active one. */
  currentTarget: BackendTarget | null;
  /** Run after any mutation so the parent's hook re-fetches state. */
  onChanged: () => void;
}

const EMPTY_FORM: RemoteBackendForm = {
  name: "",
  ssh_alias: "",
  remote_socket_path: "/run/portsage/portsage.sock",
  local_socket_path: "",
  auto_forward_enabled: false,
};

/**
 * Default for `local_socket_path` derived from the backend's name. Kept in
 * sync with what the plan suggests as the convention. The user can override
 * it in the form before saving.
 */
function defaultLocalSocket(name: string): string {
  const safe = name.trim().replace(/[^a-zA-Z0-9_.-]/g, "-");
  return safe ? `/tmp/portsage-${safe}.sock` : "";
}

export function RemoteBackendsPanel({
  remotes,
  tunnels,
  currentTarget,
  onChanged,
}: RemoteBackendsPanelProps) {
  const [showAdd, setShowAdd] = useState(false);
  const [form, setForm] = useState<RemoteBackendForm>(EMPTY_FORM);
  const [busy, setBusy] = useState<string | null>(null);
  const { showError, showSuccess } = useToast();
  const confirm = useConfirm();

  const updateField = <K extends keyof RemoteBackendForm>(
    key: K,
    value: RemoteBackendForm[K],
  ) => {
    setForm((prev) => {
      const next = { ...prev, [key]: value };
      // Auto-derive local_socket_path from name unless the user typed one.
      if (key === "name" && (!prev.local_socket_path || prev.local_socket_path === defaultLocalSocket(prev.name))) {
        next.local_socket_path = defaultLocalSocket(value as string);
      }
      return next;
    });
  };

  const handleSubmit = async () => {
    setBusy("add");
    try {
      await cmd.addRemoteBackend(form);
      showSuccess(`Added remote backend "${form.name}"`);
      setShowAdd(false);
      setForm(EMPTY_FORM);
      onChanged();
    } catch (err) {
      showError(humanizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleTest = async (name: string) => {
    setBusy(`test:${name}`);
    try {
      const count = await cmd.testRemoteBackend(name);
      showSuccess(`"${name}" reachable - ${count} project${count === 1 ? "" : "s"}`);
    } catch (err) {
      showError(humanizeError(err));
    } finally {
      setBusy(null);
      onChanged();
    }
  };

  const handleRemove = async (b: RemoteBackend) => {
    const isCurrent =
      currentTarget?.kind === "remote" && currentTarget.name === b.name;
    const ok = await confirm({
      title: `Remove "${b.name}"?`,
      message: isCurrent
        ? `This backend is currently active. Removing it will switch back to Local.`
        : `This will close any open tunnel and forget the configuration. Continue?`,
      kind: "warning",
      okLabel: "Remove",
    });
    if (!ok) return;
    setBusy(`remove:${b.name}`);
    try {
      await cmd.removeRemoteBackend(b.id);
      showSuccess(`Removed "${b.name}"`);
      onChanged();
    } catch (err) {
      showError(humanizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleToggleAutoForward = async (b: RemoteBackend, next: boolean) => {
    setBusy(`af:${b.name}`);
    try {
      await cmd.setRemoteBackendAutoForward(b.id, next);
      onChanged();
    } catch (err) {
      showError(humanizeError(err));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="flex flex-col gap-[var(--spacing-4)]">
      <section className="flex flex-col gap-[var(--spacing-3)]">
        <UIText variant="section" as="h3">
          Remote backends
        </UIText>
        <UIText variant="body" className="text-text-secondary">
          Catalogue of remote Portsage servers reachable via SSH. The sidebar
          backend switcher lists everything you add here. Tunnels open on
          demand; the dot on each row mirrors the current tunnel state.
        </UIText>

        {remotes.length === 0 && !showAdd && (
          <div className="bg-bg-input border border-border-subtle rounded-[var(--radius-md)] p-[var(--spacing-3)]">
            <UIText variant="body" className="text-text-muted text-[12px]!">
              No remote backends configured yet.
            </UIText>
          </div>
        )}

        {remotes.length > 0 && (
          <div className="flex flex-col gap-[var(--spacing-1)] bg-bg-input border border-border-subtle rounded-[var(--radius-md)] p-[var(--spacing-2)]">
            {remotes.map((b) => (
              <RemoteBackendRow
                key={b.id}
                backend={b}
                tunnel={tunnels[b.name]}
                busy={busy}
                onTest={() => handleTest(b.name)}
                onRemove={() => handleRemove(b)}
                onToggleAutoForward={(v) => handleToggleAutoForward(b, v)}
              />
            ))}
          </div>
        )}

        {!showAdd ? (
          <div>
            <UIButton
              variant="ghost"
              onClick={() => setShowAdd(true)}
            >
              <Plus size={14} aria-hidden="true" />
              Add remote backend
            </UIButton>
          </div>
        ) : (
          <AddRemoteBackendForm
            form={form}
            onField={updateField}
            onSubmit={handleSubmit}
            onCancel={() => {
              setShowAdd(false);
              setForm(EMPTY_FORM);
            }}
            busy={busy === "add"}
          />
        )}
      </section>
    </div>
  );
}

interface RemoteBackendRowProps {
  backend: RemoteBackend;
  tunnel: TunnelState | undefined;
  busy: string | null;
  onTest: () => void;
  onRemove: () => void;
  onToggleAutoForward: (next: boolean) => void;
}

function RemoteBackendRow({
  backend,
  tunnel,
  busy,
  onTest,
  onRemove,
  onToggleAutoForward,
}: RemoteBackendRowProps) {
  const ind = describeTunnel(tunnel);
  const isTesting = busy === `test:${backend.name}`;
  const isRemoving = busy === `remove:${backend.name}`;
  const isToggling = busy === `af:${backend.name}`;

  return (
    <div className="flex flex-col gap-[var(--spacing-1)] p-[var(--spacing-2)] rounded-[var(--radius-sm)] hover:bg-bg-elevated">
      <div className="flex items-center justify-between gap-[var(--spacing-2)]">
        <div className="flex items-center gap-[var(--spacing-2)] min-w-0 flex-1">
          <span
            className={`inline-block w-2 h-2 rounded-full shrink-0 ${ind.dotClass}`}
            aria-label={ind.label}
            title={ind.tooltip ?? ind.label}
          />
          <UIText variant="section" className="text-[13px] truncate">
            {backend.name}
          </UIText>
          <UIText variant="mono" className="text-[11px]! text-text-muted truncate">
            {backend.ssh_alias}
          </UIText>
        </div>
        <div className="flex items-center gap-[var(--spacing-1)]">
          <UIButton
            variant="ghost"
            onClick={onTest}
            disabled={isTesting}
          >
            <Play size={12} aria-hidden="true" />
            {isTesting ? "Testing…" : "Test"}
          </UIButton>
          <UIButton
            variant="danger"
            onClick={onRemove}
            disabled={isRemoving}
          >
            <Trash2 size={12} aria-hidden="true" />
            {isRemoving ? "Removing…" : "Remove"}
          </UIButton>
        </div>
      </div>
      <div className="flex items-center gap-[var(--spacing-3)] pl-[var(--spacing-3)]">
        <UIText variant="mono" className="text-[11px]! text-text-muted truncate">
          remote: {backend.remote_socket_path}
        </UIText>
        <UIText variant="mono" className="text-[11px]! text-text-muted truncate">
          local: {backend.local_socket_path}
        </UIText>
      </div>
      <label className="flex items-center gap-[var(--spacing-2)] pl-[var(--spacing-3)] cursor-pointer select-none">
        <input
          type="checkbox"
          checked={backend.auto_forward_enabled}
          disabled={isToggling}
          onChange={(e) => onToggleAutoForward(e.target.checked)}
        />
        <UIText variant="body" className="text-[11px]! text-text-muted">
          Auto-forward ports
        </UIText>
      </label>
      <ExcludedPortsSection backend={backend} />
    </div>
  );
}

/**
 * Collapsible "Excluded ports" sub-section for one remote backend.
 * Lists ports the user has explicitly blocked from being auto-forwarded
 * (typically because a local process already owns that port and the user
 * doesn't want Portsage to fight it). Fetches on expand, refreshes after
 * each add/remove.
 */
function ExcludedPortsSection({ backend }: { backend: RemoteBackend }) {
  const [open, setOpen] = useState(false);
  const [list, setList] = useState<ForwardExclusion[]>([]);
  const [newPort, setNewPort] = useState("");
  const [busy, setBusy] = useState(false);
  const { showError } = useToast();

  const refresh = async () => {
    try {
      const rows = await cmd.listForwardExclusions(backend.id);
      setList(rows);
    } catch (err) {
      showError(humanizeError(err));
    }
  };

  useEffect(() => {
    if (open) void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const handleAdd = async () => {
    const portNum = parseInt(newPort, 10);
    if (Number.isNaN(portNum) || portNum < 1 || portNum > 65535) {
      showError("Port must be a number between 1 and 65535.");
      return;
    }
    setBusy(true);
    try {
      await cmd.addForwardExclusion(backend.id, portNum);
      setNewPort("");
      await refresh();
    } catch (err) {
      showError(humanizeError(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = async (id: number) => {
    setBusy(true);
    try {
      await cmd.removeForwardExclusion(id);
      await refresh();
    } catch (err) {
      showError(humanizeError(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="pl-[var(--spacing-3)] flex flex-col gap-[var(--spacing-1)]">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-[var(--spacing-1)] text-text-muted hover:text-text-primary cursor-pointer w-fit"
      >
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <UIText variant="body" className="text-[11px]!">
          Excluded ports {list.length > 0 && open && `(${list.length})`}
        </UIText>
      </button>
      {open && (
        <div className="pl-[var(--spacing-3)] flex flex-col gap-[var(--spacing-1)]">
          {list.length === 0 && (
            <UIText variant="body" className="text-[11px]! text-text-muted">
              No exclusions. Ports listed here are skipped by auto-forward.
            </UIText>
          )}
          {list.map((ex) => (
            <div
              key={ex.id}
              className="flex items-center justify-between gap-[var(--spacing-2)]"
            >
              <UIText variant="mono" className="text-[11px]!">
                {ex.port}
              </UIText>
              <button
                type="button"
                onClick={() => handleRemove(ex.id)}
                disabled={busy}
                className="text-text-muted hover:text-accent-danger cursor-pointer disabled:opacity-50"
                aria-label={`Remove exclusion for port ${ex.port}`}
              >
                <X size={12} />
              </button>
            </div>
          ))}
          <div className="flex items-center gap-[var(--spacing-1)]">
            <UIInput
              value={newPort}
              onChange={(e) => setNewPort(e.target.value)}
              placeholder="Port to exclude"
              type="number"
              min={1}
              max={65535}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleAdd();
              }}
            />
            <UIButton variant="ghost" onClick={handleAdd} disabled={busy || !newPort}>
              <Plus size={12} aria-hidden="true" />
              Add
            </UIButton>
          </div>
        </div>
      )}
    </div>
  );
}

interface AddFormProps {
  form: RemoteBackendForm;
  onField: <K extends keyof RemoteBackendForm>(
    key: K,
    value: RemoteBackendForm[K],
  ) => void;
  onSubmit: () => void;
  onCancel: () => void;
  busy: boolean;
}

function AddRemoteBackendForm({
  form,
  onField,
  onSubmit,
  onCancel,
  busy,
}: AddFormProps) {
  const canSubmit =
    form.name.trim() !== "" &&
    form.ssh_alias.trim() !== "" &&
    form.remote_socket_path.trim() !== "" &&
    form.local_socket_path.trim() !== "" &&
    !busy;

  return (
    <div className="bg-bg-input border border-border-subtle rounded-[var(--radius-md)] p-[var(--spacing-3)] flex flex-col gap-[var(--spacing-2)]">
      <div className="flex items-center justify-between">
        <UIText variant="section" className="text-[13px]">
          Add remote backend
        </UIText>
        <button
          type="button"
          onClick={onCancel}
          className="text-text-muted hover:text-text-primary cursor-pointer"
          aria-label="Cancel"
        >
          <X size={14} />
        </button>
      </div>

      <FormField label="Name" hint="Short label. Used in the sidebar (e.g. dev, staging).">
        <UIInput
          value={form.name}
          onChange={(e) => onField("name", e.target.value)}
          placeholder="dev"
          autoFocus
        />
      </FormField>

      <FormField label="SSH alias" hint="Must match a Host entry in your ~/.ssh/config.">
        <UIInput
          value={form.ssh_alias}
          onChange={(e) => onField("ssh_alias", e.target.value)}
          placeholder="dev-server"
        />
      </FormField>

      <FormField label="Remote socket" hint="Path on the remote box (default for systemd installs).">
        <UIInput
          value={form.remote_socket_path}
          onChange={(e) => onField("remote_socket_path", e.target.value)}
          placeholder="/run/portsage/portsage.sock"
        />
      </FormField>

      <FormField label="Local socket" hint="SSH-forward target on this Mac. The default keeps backends from colliding.">
        <UIInput
          value={form.local_socket_path}
          onChange={(e) => onField("local_socket_path", e.target.value)}
          placeholder="/tmp/portsage-dev.sock"
        />
      </FormField>

      <UIDivider />

      <div className="flex justify-end gap-[var(--spacing-2)]">
        <UIButton variant="ghost" onClick={onCancel} disabled={busy}>
          Cancel
        </UIButton>
        <UIButton
          variant="primary"
          onClick={onSubmit}
          disabled={!canSubmit}
        >
          {busy ? "Saving…" : "Save"}
        </UIButton>
      </div>
    </div>
  );
}

function FormField({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-[var(--spacing-1)]">
      <UIText variant="label">{label}</UIText>
      {children}
      {hint && (
        <UIText variant="body" className="text-[11px]! text-text-muted">
          {hint}
        </UIText>
      )}
    </div>
  );
}

function describeTunnel(state: TunnelState | undefined): {
  dotClass: string;
  label: string;
  tooltip?: string;
} {
  if (!state || state.state === "disconnected") {
    return { dotClass: "bg-status-inactive", label: "Disconnected" };
  }
  if (state.state === "connecting") {
    return { dotClass: "bg-accent-amber animate-pulse", label: "Connecting" };
  }
  if (state.state === "connected") {
    return { dotClass: "bg-accent-success", label: "Connected" };
  }
  return { dotClass: "bg-accent-danger", label: "Failed", tooltip: state.reason };
}
