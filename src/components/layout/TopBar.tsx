import { Link } from "react-router-dom";
import { useAuth } from "@/context/AuthContext";
import { Button } from "@/components/ui/Button";
import { ThemeToggle } from "@/components/ui/ThemeToggle";

export function TopBar({ title }: { title: string }) {
  const { logout, isDemo } = useAuth();

  return (
    <header className="flex h-14 shrink-0 items-center justify-between border-b border-[var(--color-border)] bg-[var(--color-surface)] px-6">
      <div className="min-w-0">
        <p className="truncate text-[10px] font-semibold uppercase tracking-[0.16em] text-[var(--color-or)]">
          Carreterie Gestion
          {isDemo ? " – Démo" : ""}
        </p>
        <h1 className="truncate text-base font-semibold text-[var(--color-text)]">{title}</h1>
      </div>
      <div className="flex shrink-0 items-center gap-3">
        <ThemeToggle />
        <Link
          to="/a-propos"
          className="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-or)]"
        >
          À propos
        </Link>
        <Button variant="secondary" size="sm" onClick={() => void logout()}>
          Déconnexion
        </Button>
      </div>
    </header>
  );
}
