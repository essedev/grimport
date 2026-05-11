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
      className={`
        font-mono text-[12px] tabular-nums
        text-text-primary hover:text-accent-amber
        bg-transparent border-0 p-0
        cursor-pointer transition-colors duration-150
        focus:outline-none focus-visible:text-accent-amber
        ${className}
      `}
    >
      {port}
    </button>
  );
}
