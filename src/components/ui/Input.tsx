import { cn } from "@/lib/cn";
import type { InputHTMLAttributes } from "react";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  hint?: string;
}

export function Input({ label, error, hint, className, id, ...props }: InputProps) {
  const inputId = id ?? label?.toLowerCase().replace(/\s+/g, "-");
  return (
    <div className="flex flex-col gap-1">
      {label ? (
        <label htmlFor={inputId} className="text-xs font-medium text-[var(--color-text-muted)]">
          {label}
        </label>
      ) : null}
      <input
        id={inputId}
        className={cn(
          "rounded-md border bg-[var(--color-surface)] px-3 py-2 text-sm text-[var(--color-text)]",
          "border-[var(--color-border)] placeholder:text-[var(--color-text-muted)]",
          "focus:border-[var(--color-or)] focus:outline-none focus:ring-1 focus:ring-[var(--color-or)]/40",
          error && "border-[var(--color-danger)]",
          className,
        )}
        {...props}
      />
      {error ? <p className="text-xs text-[var(--color-danger)]">{error}</p> : null}
      {hint && !error ? <p className="text-xs text-[var(--color-text-muted)]">{hint}</p> : null}
    </div>
  );
}