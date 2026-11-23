import { cn } from "@/lib/cn";
import { Spinner } from "./Spinner";
import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger" | "gold";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  loading?: boolean;
  size?: "sm" | "md" | "lg";
}

const variants: Record<Variant, string> = {
  primary:
    "bg-[var(--color-rouge)] text-[var(--color-blanc)] hover:bg-[var(--color-bordeaux)] border border-transparent",
  secondary:
    "bg-[var(--color-surface-elevated)] text-[var(--color-text)] border border-[var(--color-border)] hover:border-[var(--color-or)]/50",
  ghost:
    "bg-transparent text-[var(--color-text)] border border-transparent hover:bg-[var(--color-surface-elevated)]",
  danger:
    "bg-[var(--color-danger)]/20 text-[var(--color-blanc)] border border-[var(--color-danger)]/40 hover:bg-[var(--color-danger)]/35",
  gold:
    "bg-[var(--color-or)]/15 text-[var(--color-or)] border border-[var(--color-or)]/40 hover:bg-[var(--color-or)]/25",
};

const sizes = {
  sm: "px-2.5 py-1 text-xs",
  md: "px-4 py-2 text-sm",
  lg: "px-5 py-2.5 text-sm",
};

export function Button({
  className,
  variant = "primary",
  size = "md",
  loading,
  disabled,
  children,
  ...props
}: ButtonProps) {
  return (
    <button
      type="button"
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-md font-medium transition-colors",
        "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-or)]",
        "disabled:pointer-events-none disabled:opacity-50",
        variants[variant],
        sizes[size],
        className,
      )}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? <Spinner className="h-4 w-4" /> : null}
      {children}
    </button>
  );
}