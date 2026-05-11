import { Trash2, Power } from "lucide-react";
import { UIStatus } from "@/components/ui/UIStatus";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIPortLink } from "@/components/ui/UIPortLink";
import type { PortStatus } from "@/lib/types";

interface PortRowProps {
  port: PortStatus;
  onRemove?: (id: number) => void;
  onKill?: (port: PortStatus) => void;
}

export function PortRow({ port, onRemove, onKill }: PortRowProps) {
  return (
    <div className="flex items-center gap-[var(--spacing-2)] h-8 group">
      <UIStatus active={port.active} />
      <UIText variant="body" className="flex-1 truncate">
        {port.service}
      </UIText>
      {port.active && port.process && (
        <UIText variant="mono" className="text-text-muted text-[10px]! truncate max-w-40">
          {port.process}
          {port.pid !== null && (
            <span className="text-text-muted/70"> · {port.pid}</span>
          )}
        </UIText>
      )}
      <UIPortLink port={port.port} />
      {onKill && port.active && (
        <UIButton
          variant="ghost"
          className="opacity-0 group-hover:opacity-100 p-1 text-accent-danger hover:bg-accent-danger-soft hover:text-accent-danger"
          title="Stop process on this port"
          onClick={() => onKill(port)}
        >
          <Power size={14} />
        </UIButton>
      )}
      {onRemove && (
        <UIButton
          variant="danger"
          className="opacity-0 group-hover:opacity-100 p-1"
          title="Remove port from project"
          onClick={() => onRemove(port.id)}
        >
          <Trash2 size={14} />
        </UIButton>
      )}
    </div>
  );
}
