import { type ReactNode } from "react";

export interface UITabOption<T extends string = string> {
  value: T;
  label: string;
  count?: number;
}

interface UITabsProps<T extends string = string> {
  options: UITabOption<T>[];
  value: T;
  onChange: (value: T) => void;
  className?: string;
  ariaLabel?: string;
}

export function UITabs<T extends string = string>({
  options,
  value,
  onChange,
  className = "",
  ariaLabel,
}: UITabsProps<T>) {
  return (
    <div
      role="tablist"
      aria-label={ariaLabel}
      className={`
        flex items-end gap-[var(--spacing-3)]
        border-b border-border-subtle
        ${className}
      `}
    >
      {options.map((opt) => {
        const active = opt.value === value;
        return (
          <button
            key={opt.value}
            role="tab"
            aria-selected={active}
            aria-controls={`tabpanel-${opt.value}`}
            id={`tab-${opt.value}`}
            tabIndex={active ? 0 : -1}
            type="button"
            onClick={() => onChange(opt.value)}
            className={`
              relative -mb-px flex items-center gap-[var(--spacing-1)]
              px-[var(--spacing-1)] pb-[var(--spacing-2)] pt-[var(--spacing-1)]
              font-mono text-[12px] cursor-pointer
              border-b-2 transition-colors duration-150
              focus:outline-none focus-visible:text-accent-amber
              ${active
                ? "text-accent-amber border-accent-amber"
                : "text-text-secondary border-transparent hover:text-text-primary"
              }
            `}
          >
            <span>{opt.label}</span>
            {typeof opt.count === "number" && (
              <span
                className={`
                  font-sans text-[10px] font-medium
                  px-[var(--spacing-1)] rounded-[var(--radius-sm)]
                  ${active
                    ? "bg-accent-amber-soft text-accent-amber"
                    : "bg-bg-elevated text-text-muted"
                  }
                `}
              >
                {opt.count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}

interface UITabPanelProps {
  value: string;
  active: string;
  children: ReactNode;
  className?: string;
}

export function UITabPanel({ value, active, children, className = "" }: UITabPanelProps) {
  if (value !== active) return null;
  return (
    <div
      role="tabpanel"
      id={`tabpanel-${value}`}
      aria-labelledby={`tab-${value}`}
      className={className}
    >
      {children}
    </div>
  );
}
