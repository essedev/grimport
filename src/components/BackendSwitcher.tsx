import { useMemo } from "react";
import { UISelect } from "@/components/ui/UISelect";
import { UIText } from "@/components/ui/UIText";
import type { BackendTarget, RemoteBackend, TunnelState } from "@/lib/types";

/**
 * Sentinel option value for "+ Add remote backend...". Kept as a constant so
 * the parent can distinguish "user picked a backend" from "user wants to add
 * one" with strict equality and no magic-string drift.
 */
export const ADD_BACKEND_OPTION = "__add__";

/**
 * Encode a `BackendTarget` as a string the `UISelect` can store as a value.
 * Inverse: {@link decodeOptionValue}. Kept inline so the encoding is a single
 * concern; we deliberately do not push it into a shared utility because the
 * lifetime is "as long as the dropdown is mounted".
 */
function encodeTarget(target: BackendTarget): string {
  return target.kind === "local" ? "local" : `remote:${target.name}`;
}

function decodeOptionValue(value: string): BackendTarget | "add" | null {
  if (value === ADD_BACKEND_OPTION) return "add";
  if (value === "local") return { kind: "local" };
  if (value.startsWith("remote:")) {
    return { kind: "remote", name: value.slice("remote:".length) };
  }
  return null;
}

interface BackendSwitcherProps {
  target: BackendTarget | null;
  remotes: RemoteBackend[];
  tunnels: Record<string, TunnelState>;
  onSelectTarget: (target: BackendTarget) => void;
  onAddBackend: () => void;
  className?: string;
}

/**
 * Header element that lets the user switch between the local backend and any
 * configured remote backend. Sits above the search box in the sidebar. The
 * status dot reflects the tunnel state of the *selected* backend; selecting
 * a different backend does not change the existing dots for other tunnels.
 */
export function BackendSwitcher({
  target,
  remotes,
  tunnels,
  onSelectTarget,
  onAddBackend,
  className = "",
}: BackendSwitcherProps) {
  const options = useMemo(() => {
    const remoteOptions = remotes.map((r) => ({
      value: encodeTarget({ kind: "remote", name: r.name }),
      label: `Remote: ${r.name}`,
    }));
    return [
      { value: "local", label: "Local" },
      ...remoteOptions,
      { value: ADD_BACKEND_OPTION, label: "+ Add remote backend…" },
    ];
  }, [remotes]);

  const currentValue = target ? encodeTarget(target) : "local";

  const handleChange = (value: string) => {
    const decoded = decodeOptionValue(value);
    if (decoded === "add") {
      onAddBackend();
      return;
    }
    if (decoded) {
      onSelectTarget(decoded);
    }
  };

  const indicator = currentIndicator(target, tunnels);

  return (
    <div className={`flex flex-col gap-[var(--spacing-1)] ${className}`}>
      <UISelect
        label="Backend"
        options={options}
        value={currentValue}
        onChange={handleChange}
      />
      {indicator && (
        <div
          className="flex items-center gap-[var(--spacing-1)] px-[var(--spacing-1)]"
          title={indicator.tooltip}
        >
          <span
            className={`
              inline-block w-2 h-2 rounded-full shrink-0
              ${indicator.dotClass}
            `}
            aria-label={indicator.label}
          />
          <UIText
            variant="body"
            className="text-[11px]! text-text-muted"
          >
            {indicator.label}
          </UIText>
        </div>
      )}
    </div>
  );
}

/**
 * Resolve the dot color + label + tooltip for the currently-selected backend.
 * Returns null when Local is selected: there is no tunnel to report on, so
 * showing a permanent "Connected" indicator would be visual noise.
 */
function currentIndicator(
  target: BackendTarget | null,
  tunnels: Record<string, TunnelState>,
): { dotClass: string; label: string; tooltip?: string } | null {
  if (!target || target.kind === "local") return null;
  const state = tunnels[target.name];
  if (!state || state.state === "disconnected") {
    return {
      dotClass: "bg-status-inactive",
      label: "Disconnected",
      tooltip: "Tunnel not opened yet. Use Test in Settings to connect.",
    };
  }
  if (state.state === "connecting") {
    return {
      dotClass: "bg-accent-amber animate-pulse",
      label: "Connecting…",
    };
  }
  if (state.state === "connected") {
    return {
      dotClass: "bg-accent-success",
      label: "Connected",
    };
  }
  // failed
  return {
    dotClass: "bg-accent-danger",
    label: "Failed",
    tooltip: state.reason,
  };
}
