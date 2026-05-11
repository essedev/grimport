import { useState } from "react";
import { Plus, Settings, AlertTriangle, ArrowLeft } from "lucide-react";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIBadge } from "@/components/ui/UIBadge";
import { AddProjectForm } from "@/components/AddProjectForm";
import type { ProjectStatus, UnmanagedPort } from "@/lib/types";

type SettingsTab = "general" | "integrations" | "data";

interface WelcomePanelProps {
  projects: ProjectStatus[];
  unmanagedPorts: UnmanagedPort[];
  onCreate: (name: string, path?: string) => void;
  onShowSettings: (tab?: SettingsTab) => void;
  onShowUnmanaged: () => void;
}

export function WelcomePanel({
  projects,
  unmanagedPorts,
  onCreate,
  onShowSettings,
  onShowUnmanaged,
}: WelcomePanelProps) {
  const [showAdd, setShowAdd] = useState(false);

  const totalProjects = projects.length;
  const totalPorts = projects.reduce((sum, p) => sum + p.ports.length, 0);
  const totalActive = projects
    .flatMap((p) => p.ports)
    .filter((p) => p.active).length;

  const isFirstRun = totalProjects === 0;

  return (
    <div className="flex flex-col items-center justify-center h-full w-full px-[var(--spacing-5)]">
      <div className="flex flex-col gap-[var(--spacing-6)] w-full max-w-[480px]">
        <header className="flex flex-col gap-[var(--spacing-2)]">
          <UIText
            variant="title"
            as="h2"
            className="text-[22px]!"
            style={{ textShadow: "0 0 12px var(--color-accent-amber-glow)" }}
          >
            {isFirstRun ? "Welcome to portsage" : "ports under control"}
          </UIText>
          <UIText variant="body" className="text-text-secondary">
            {isFirstRun
              ? "Reserve port ranges for each project, register the services that run inside them, and stop runaway processes without grepping lsof."
              : "Select a project from the sidebar to manage its ports, or create a new one."}
          </UIText>
        </header>

        {!isFirstRun && (
          <div className="grid grid-cols-3 gap-[var(--spacing-2)]">
            <Stat label="Projects" value={totalProjects} />
            <Stat label="Registered ports" value={totalPorts} />
            <Stat label="Active now" value={totalActive} highlight={totalActive > 0} />
          </div>
        )}

        {!isFirstRun && !showAdd && (
          <div className="flex items-center gap-[var(--spacing-2)] text-text-muted">
            <ArrowLeft size={14} aria-hidden="true" />
            <UIText variant="body" className="text-text-muted">
              Pick a project from the sidebar to see its ports.
            </UIText>
          </div>
        )}

        {showAdd ? (
          <div className="bg-bg-surface border border-border-subtle rounded-[var(--radius-md)]">
            <AddProjectForm
              onSubmit={(name, path) => {
                onCreate(name, path);
                setShowAdd(false);
              }}
              onCancel={() => setShowAdd(false)}
            />
          </div>
        ) : (
          <div className="flex flex-wrap items-center gap-[var(--spacing-2)]">
            <UIButton variant="primary" onClick={() => setShowAdd(true)}>
              <Plus size={14} aria-hidden="true" />
              {isFirstRun ? "Create your first project" : "New project"}
            </UIButton>
            {unmanagedPorts.length > 0 && (
              <UIButton
                variant="warning"
                onClick={onShowUnmanaged}
                aria-label={`Review ${unmanagedPorts.length} unmanaged ports`}
              >
                <AlertTriangle size={14} aria-hidden="true" />
                {unmanagedPorts.length} unmanaged
              </UIButton>
            )}
            <UIButton variant="ghost" onClick={() => onShowSettings()}>
              <Settings size={14} aria-hidden="true" />
              Settings
            </UIButton>
          </div>
        )}

        {isFirstRun && (
          <div className="flex flex-col gap-[var(--spacing-2)] pt-[var(--spacing-2)]">
            <UIText variant="label" className="text-text-muted">
              NEXT STEPS
            </UIText>
            <ul className="flex flex-col gap-[var(--spacing-1)] text-text-secondary text-[12px] font-sans pl-[var(--spacing-3)] list-disc">
              <li>Reserve a port range for the project.</li>
              <li>Register each service running locally (vite, postgres, redis...).</li>
              <li>
                Optional: connect your AI editor in{" "}
                <button
                  type="button"
                  onClick={() => onShowSettings("integrations")}
                  className="text-accent-amber hover:underline cursor-pointer focus:outline-none focus-visible:underline"
                >
                  Settings &rarr; Integrations
                </button>{" "}
                so it can reserve ports for you.
              </li>
            </ul>
          </div>
        )}
      </div>
    </div>
  );
}

interface StatProps {
  label: string;
  value: number;
  highlight?: boolean;
}

function Stat({ label, value, highlight }: StatProps) {
  return (
    <div className="flex flex-col gap-[var(--spacing-1)] bg-bg-surface border border-border-subtle rounded-[var(--radius-md)] px-[var(--spacing-3)] py-[var(--spacing-3)]">
      <UIText variant="label">{label}</UIText>
      <div className="flex items-center gap-[var(--spacing-2)]">
        <UIText
          variant="mono"
          className={`text-[20px]! tabular-nums ${highlight ? "text-accent-amber!" : ""}`}
        >
          {value}
        </UIText>
        {highlight && value > 0 && (
          <UIBadge variant="active" className="text-[10px]!">
            live
          </UIBadge>
        )}
      </div>
    </div>
  );
}
