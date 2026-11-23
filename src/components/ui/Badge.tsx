import { cn } from "@/lib/cn";
import type { ReactNode } from "react";

type Tone = "default" | "gold" | "red" | "green" | "muted";

const tones: Record<Tone, string> = {
  default: "bg-[var(--color-surface-elevated)] text-[var(--color-text)] border-[var(--color-border)]",
  gold: "bg-[var(--color-or)]/15 text-[var(--color-or)] border-[var(--color-or)]/35",
  red: "bg-[var(--color-rouge)]/20 text-[var(--color-blanc)] border-[var(--color-rouge)]/40",
  green: "bg-[var(--color-success)]/15 text-emerald-300 border-[var(--color-success)]/35",
  muted: "bg-transparent text-[var(--color-text-muted)] border-[var(--color-border)]",
};

export function Badge({
  children,
  tone = "default",
  className,
}: {
  children: ReactNode;
  tone?: Tone;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium",
        tones[tone],
        className,
      )}
    >
      {children}
    </span>
  );
}

export function invoiceStatusBadge(status: string): { label: string; tone: Tone } {
  const map: Record<string, { label: string; tone: Tone }> = {
    brouillon: { label: "Brouillon", tone: "muted" },
    emise: { label: "Émis", tone: "gold" },
    avoir: { label: "Avoir", tone: "red" },
  };
  return map[status] ?? { label: status, tone: "default" };
}