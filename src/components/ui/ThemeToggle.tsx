import { useTheme } from "@/context/ThemeContext";
import { cn } from "@/lib/cn";

export function ThemeToggle({ className }: { className?: string }) {
  const { isDark, toggleTheme } = useTheme();

  return (
    <button
      type="button"
      onClick={toggleTheme}
      className={cn(
        "inline-flex h-8 items-center gap-2 rounded-md border border-[var(--color-border)] bg-[var(--color-surface-elevated)] px-2.5 text-xs font-medium text-[var(--color-text)] transition hover:border-[var(--color-or)] hover:text-[var(--color-or)]",
        className,
      )}
      title={isDark ? "Passer en thème clair" : "Passer en thème sombre"}
      aria-label={isDark ? "Passer en thème clair" : "Passer en thème sombre"}
    >
      <span aria-hidden className="text-sm leading-none">
        {isDark ? "☀" : "☾"}
      </span>
      <span>{isDark ? "Clair" : "Sombre"}</span>
    </button>
  );
}
