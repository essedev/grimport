import { openInBrowser } from "@/lib/commands";

interface UIPortLinkProps {
  port: number;
  className?: string;
}

export function UIPortLink({ port, className = "" }: UIPortLinkProps) {
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        openInBrowser(port);
      }}
      title={`Open http://localhost:${port} in browser`}
      aria-label={`Open http://localhost:${port} in browser`}
      className={`
        font-mono text-[12px] tabular-nums
        text-text-primary hover:text-accent-amber
        bg-transparent border-0 px-[var(--spacing-1)] rounded-[var(--radius-sm)]
        cursor-pointer transition-colors duration-150
        focus:outline-none focus-visible:text-accent-amber
        focus-visible:ring-2 focus-visible:ring-accent-amber/60
        ${className}
      `}
    >
      {port}
    </button>
  );
}
