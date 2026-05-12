import { useCallback, useEffect, useMemo, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as cmd from "@/lib/commands";
import { humanizeError } from "@/lib/errors";
import { useToast } from "@/lib/toast";
import type {
  BackendTarget,
  RemoteBackend,
  TunnelState,
  TunnelStatus,
} from "@/lib/types";

/**
 * Shape returned by `useBackends`. Encapsulates the active backend target,
 * the list of remote backends, and the live tunnel status map. Components
 * use this hook instead of poking `commands.ts` directly so we can swap the
 * implementation (e.g. add polling, optimistic updates) without rippling
 * changes through every consumer.
 */
export interface BackendsApi {
  /** Active backend target. `null` while the initial fetch is in flight. */
  target: BackendTarget | null;
  /** Catalogue of configured remote backends. */
  remotes: RemoteBackend[];
  /** Tunnel state keyed by backend name. */
  tunnels: Record<string, TunnelState>;
  loading: boolean;
  /** Refetch the target + remotes + tunnel statuses. */
  refresh: () => Promise<void>;
  /** Persist the new target. No-op + error toast on failure. */
  setTarget: (target: BackendTarget) => Promise<boolean>;
  /** Run the round-trip probe and return the remote project count. */
  testBackend: (name: string) => Promise<number | null>;
  /** Close an open tunnel. Idempotent for unknown names. */
  closeTunnel: (name: string) => Promise<void>;
}

/**
 * Subscribe to backend state. The hook keeps `target` and the tunnel status
 * map in sync with the backend by combining an initial fetch + a Tauri
 * event listener for `tunnel://state-changed`. Mutations go through
 * `setTarget` / `closeTunnel` / `testBackend`; the hook refetches as needed.
 */
export function useBackends(): BackendsApi {
  const [target, setTargetState] = useState<BackendTarget | null>(null);
  const [remotes, setRemotes] = useState<RemoteBackend[]>([]);
  const [tunnels, setTunnels] = useState<Record<string, TunnelState>>({});
  const [loading, setLoading] = useState(true);
  const { showError } = useToast();

  const refresh = useCallback(async () => {
    try {
      const [t, rs, statuses] = await Promise.all([
        cmd.getCurrentBackend(),
        cmd.listRemoteBackends(),
        cmd.getTunnelStatuses(),
      ]);
      setTargetState(t);
      setRemotes(rs);
      const next: Record<string, TunnelState> = {};
      for (const s of statuses) {
        next[s.backend_name] = s.state;
      }
      setTunnels(next);
    } catch (err) {
      showError(humanizeError(err));
    } finally {
      setLoading(false);
    }
  }, [showError]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    void (async () => {
      await refresh();
      unlisten = await listen<TunnelStatus>(cmd.TUNNEL_EVENT, (event) => {
        const status = event.payload;
        setTunnels((prev) => ({ ...prev, [status.backend_name]: status.state }));
      });
    })();
    return () => {
      unlisten?.();
    };
  }, [refresh]);

  const setTarget = useCallback(
    async (next: BackendTarget): Promise<boolean> => {
      try {
        await cmd.setCurrentBackend(next);
        setTargetState(next);
        return true;
      } catch (err) {
        showError(humanizeError(err));
        return false;
      }
    },
    [showError],
  );

  const testBackend = useCallback(
    async (name: string): Promise<number | null> => {
      try {
        const count = await cmd.testRemoteBackend(name);
        return count;
      } catch (err) {
        showError(humanizeError(err));
        return null;
      } finally {
        // The Rust side emits a TUNNEL_EVENT during test, but emit a refetch
        // too so the table picks up the new state even if the listener race
        // misses the event (e.g. on first connect before listen() finished).
        void refresh();
      }
    },
    [refresh, showError],
  );

  const closeTunnel = useCallback(
    async (name: string): Promise<void> => {
      try {
        await cmd.closeTunnel(name);
      } catch (err) {
        showError(humanizeError(err));
      } finally {
        void refresh();
      }
    },
    [refresh, showError],
  );

  return useMemo(
    () => ({
      target,
      remotes,
      tunnels,
      loading,
      refresh,
      setTarget,
      testBackend,
      closeTunnel,
    }),
    [target, remotes, tunnels, loading, refresh, setTarget, testBackend, closeTunnel],
  );
}
