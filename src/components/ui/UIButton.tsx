import { type ReactNode, type ButtonHTMLAttributes } from "react";

type ButtonVariant = "primary" | "ghost" | "warning" | "danger";
type ButtonSize = "default" | "icon" | "icon-sm";

interface UIButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  children: ReactNode;
}

// Hover backgrounds use translucent overlays (white/10, danger/20) instead of
// solid tokens so the contrast is visible regardless of whether the underlying
// row is on bg-deep, bg-surface, or bg-elevated. A solid bg-elevated hover
// disappears when the row itself already paints bg-elevated on hover.
// `warning` (amber) sits between ghost and danger: stopping a process is
// destructive against the *target* but not against portsage data, so it
// warrants an alert tint without the full red of `danger`.
const variantClasses: Record<ButtonVariant, string> = {
  primary:
    "bg-accent-amber text-bg-deep hover:bg-accent-amber/90 border-transparent",
  ghost:
    "bg-transparent text-text-secondary hover:text-text-primary hover:bg-white/10 border-transparent",
  warning:
    "bg-transparent text-accent-amber hover:bg-accent-amber/15 hover:text-accent-amber border-transparent",
  danger:
    "bg-transparent text-accent-danger hover:bg-accent-danger/20 hover:text-accent-danger border-transparent",
};

// Square icon-only sizes keep toolbars visually grid-aligned. The 28px size
// matches the existing 32px row height with a 2px inset; 24px is for inline
// row actions inside a 32px PortRow.
const sizeClasses: Record<ButtonSize, string> = {
  default: "px-[var(--spacing-3)] py-[var(--spacing-1)]",
  icon: "w-7 h-7 p-0",
  "icon-sm": "w-6 h-6 p-0",
};

export function UIButton({
  variant = "ghost",
  size = "default",
  children,
  className = "",
  ...props
}: UIButtonProps) {
  return (
    <button
      className={`
        inline-flex items-center justify-center gap-[var(--spacing-1)]
        ${sizeClasses[size]}
        rounded-[var(--radius-sm)] border
        font-sans text-[13px] font-medium
        transition-colors duration-150
        cursor-pointer
        disabled:opacity-40 disabled:pointer-events-none
        focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber
        ${variantClasses[variant]} ${className}
      `}
      {...props}
    >
      {children}
    </button>
  );
}
