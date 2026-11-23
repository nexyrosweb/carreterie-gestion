import { cn } from "@/lib/cn";
import type { TextareaHTMLAttributes } from "react";

interface TextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: string;
}

export function Textarea({ label, error, className, id, ...props }: TextareaProps) {
  const areaId = id ?? label?.toLowerCase().replace(/\s+/g, "-");
  return (
    <div className="flex flex-col gap-1">
      {label ? (
        <label htmlFor={areaId} className="text-xs font-medium text-[var(--color-text-muted)]">
          {label}
        </label>
      ) : null}
      <textarea
        id={areaId}
        className={cn(
          "min-h-[5rem] rounded-md border bg-[var(--color-surface)] px-3 py-2 text-sm",
          "border-[var(--color-border)] text-[var(--color-text)]",
          "focus:border-[var(--color-or)] focus:outline-none",
          error && "border-[var(--color-danger)]",
          className,
        )}
        {...props}
      />
      {error ? <p className="text-xs text-[var(--color-danger)]">{error}</p> : null}
    </div>
  );
}