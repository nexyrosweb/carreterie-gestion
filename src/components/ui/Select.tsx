import { cn } from "@/lib/cn";
import type { SelectHTMLAttributes } from "react";

interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  error?: string;
  options: Array<{ value: string; label: string }>;
}

export function Select({ label, error, options, className, id, ...props }: SelectProps) {
  const selectId = id ?? label?.toLowerCase().replace(/\s+/g, "-");
  return (
    <div className="flex flex-col gap-1">
      {label ? (
        <label htmlFor={selectId} className="text-xs font-medium text-[var(--color-text-muted)]">
          {label}
        </label>
      ) : null}
      <select
        id={selectId}
        className={cn(
          "rounded-md border bg-[var(--color-surface)] px-3 py-2 text-sm text-[var(--color-text)]",
          "border-[var(--color-border)] focus:border-[var(--color-or)] focus:outline-none",
          error && "border-[var(--color-danger)]",
          className,
        )}
        {...props}
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
      {error ? <p className="text-xs text-[var(--color-danger)]">{error}</p> : null}
    </div>
  );
}