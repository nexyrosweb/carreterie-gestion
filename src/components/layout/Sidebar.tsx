import { useEffect, useState, type ReactNode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { NavLink } from "react-router-dom";
import { TheatreLogo } from "@/components/brand/TheatreLogo";
import { useAuth } from "@/context/AuthContext";
import {
  readPendingUpdate,
  requestUpdateCheckWithPopup,
} from "@/hooks/useUpdaterCheck";
import { apiDbPing } from "@/lib/api";
import { cn } from "@/lib/cn";

const modules = [
  { to: "/", label: "Tableau de bord", end: true, icon: IconDashboard },
  { to: "/factures-clients", label: "Factures clients", icon: IconInvoice },
  { to: "/factures-fournisseurs", label: "Factures fournisseurs", icon: IconSupplier },
  { to: "/import-pdf", label: "Import PDF", icon: IconImport },
  { to: "/contacts", label: "Contacts", icon: IconContacts },
  { to: "/archives", label: "Archives", icon: IconArchives },
  { to: "/sauvegardes", label: "Sauvegardes", icon: IconBackup },
  { to: "/parametres", label: "Paramètres", icon: IconSettings },
  { to: "/a-propos", label: "À propos", icon: IconAbout },
] as const;

export function Sidebar() {
  return (
    <aside
      className="flex h-full w-[var(--sidebar-width)] shrink-0 flex-col border-r border-[var(--color-border)] bg-[var(--color-surface)]"
      aria-label="Navigation principale"
    >
      <div className="border-b border-[var(--color-border)] px-4 py-4">
        <div className="flex items-center gap-3">
          <TheatreLogo variant="mark" className="h-11 w-11 shrink-0" />
          <div className="min-w-0">
            <p className="text-[10px] font-semibold uppercase tracking-[0.18em] text-[var(--color-or)]">
              Théâtre La Carreterie
            </p>
            <h1 className="truncate text-base font-bold text-[var(--color-text)]">
              Carreterie Gestion
            </h1>
          </div>
        </div>
      </div>
      <nav className="flex flex-1 flex-col gap-0.5 p-2">
        {modules.map((item) => {
          const Icon = item.icon;
          return (
            <NavLink
              key={item.to}
              to={item.to}
              end={"end" in item ? item.end : false}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-2.5 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                  isActive
                    ? "border-l-2 border-[var(--color-or)] bg-[var(--color-rouge)]/20 text-[var(--color-text)]"
                    : "text-[var(--color-text-muted)] hover:bg-[var(--color-surface-elevated)] hover:text-[var(--color-text)]",
                )
              }
            >
              <Icon className="h-4 w-4 shrink-0 opacity-90" />
              <span className="truncate">{item.label}</span>
            </NavLink>
          );
        })}
      </nav>
      <DbStatusFooter />
    </aside>
  );
}

function DbStatusFooter() {
  const { sessionToken, isDemo } = useAuth();
  const [connected, setConnected] = useState<boolean | null>(null);
  const [version, setVersion] = useState("…");
  const [pendingVersion, setPendingVersion] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      setVersion("dev");
      return;
    }
    void getVersion().then(setVersion).catch(() => setVersion("?"));
  }, []);

  useEffect(() => {
    const syncPending = () => {
      setPendingVersion(readPendingUpdate()?.version ?? null);
    };
    syncPending();
    const onAvail = (ev: Event) => {
      const detail = (ev as CustomEvent<{ version: string }>).detail;
      setPendingVersion(detail?.version ?? readPendingUpdate()?.version ?? null);
    };
    window.addEventListener("carreterie:update-available", onAvail);
    window.addEventListener("storage", syncPending);
    return () => {
      window.removeEventListener("carreterie:update-available", onAvail);
      window.removeEventListener("storage", syncPending);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    const ping = () => {
      void apiDbPing(sessionToken)
        .then((r) => {
          if (!cancelled) setConnected(r.connected);
        })
        .catch(() => {
          if (!cancelled) setConnected(false);
        });
    };

    ping();
    const id = window.setInterval(ping, 15_000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [sessionToken]);

  const label =
    connected === true ? "Connecté" : connected === false ? "Déconnecté" : "Vérification…";

  return (
    <div className="border-t border-[var(--color-border)] px-3 py-3">
      <button
        type="button"
        onClick={() => requestUpdateCheckWithPopup()}
        className={cn(
          "mb-2 flex w-full items-center justify-between gap-2 rounded-md border px-2.5 py-2 text-left transition",
          pendingVersion
            ? "border-[var(--color-or)]/50 bg-[var(--color-or)]/10 hover:bg-[var(--color-or)]/20"
            : "border-[var(--color-border)] bg-[var(--color-surface-elevated)] hover:border-[var(--color-or)]/40",
        )}
        title={
          pendingVersion
            ? `Mise à jour ${pendingVersion} disponible — cliquer pour installer`
            : "Rechercher des mises à jour"
        }
      >
        <span className="min-w-0">
          <span className="block text-[10px] uppercase tracking-wide text-[var(--color-text-muted)]">
            Version
          </span>
          <span className="block truncate text-xs font-semibold text-[var(--color-text)]">
            v{version}
          </span>
        </span>
        {pendingVersion ? (
          <span className="shrink-0 rounded bg-[var(--color-or)]/20 px-1.5 py-0.5 text-[10px] font-semibold text-[var(--color-or)]">
            → {pendingVersion}
          </span>
        ) : (
          <span className="shrink-0 text-[10px] text-[var(--color-text-muted)]">MAJ</span>
        )}
      </button>

      <p
        className="flex items-center gap-2 px-0.5 text-[10px] text-[var(--color-text-muted)]"
        role="status"
        aria-live="polite"
      >
        <span
          className={cn(
            "inline-block h-2 w-2 shrink-0 rounded-full",
            connected === true && "bg-[var(--color-success)]",
            connected === false && "bg-[var(--color-danger)]",
            connected === null && "bg-[var(--color-text-muted)]",
          )}
          aria-hidden
        />
        <span>{label}</span>
        {connected === true ? (
          <span className="text-[var(--color-text-muted)]">
            · {isDemo ? "demo.db" : "Base locale"}
          </span>
        ) : null}
      </p>
    </div>
  );
}

type IconProps = { className?: string };

function iconShell(children: ReactNode, className?: string) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden
    >
      {children}
    </svg>
  );
}

function IconDashboard({ className }: IconProps) {
  return iconShell(
    <>
      <rect x="3" y="3" width="7" height="9" rx="1" />
      <rect x="14" y="3" width="7" height="5" rx="1" />
      <rect x="14" y="12" width="7" height="9" rx="1" />
      <rect x="3" y="16" width="7" height="5" rx="1" />
    </>,
    className,
  );
}

function IconInvoice({ className }: IconProps) {
  return iconShell(
    <>
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <path d="M14 2v6h6" />
      <path d="M8 13h8M8 17h5" />
    </>,
    className,
  );
}

function IconSupplier({ className }: IconProps) {
  return iconShell(
    <>
      <path d="M3 9h18v11a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V9z" />
      <path d="M3 9l2.5-5h13L21 9" />
      <path d="M9 20v-6h6v6" />
    </>,
    className,
  );
}

function IconImport({ className }: IconProps) {
  return iconShell(
    <>
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <path d="M14 2v6h6" />
      <path d="M12 18V11M9 14l3 3 3-3" />
    </>,
    className,
  );
}

function IconContacts({ className }: IconProps) {
  return iconShell(
    <>
      <circle cx="9" cy="8" r="3.5" />
      <path d="M2.5 19c0-3.2 2.9-5.5 6.5-5.5s6.5 2.3 6.5 5.5" />
      <circle cx="17" cy="9" r="2.5" />
      <path d="M21.5 18.5c0-2.2-1.8-4-4.5-4" />
    </>,
    className,
  );
}

function IconArchives({ className }: IconProps) {
  return iconShell(
    <>
      <path d="M4 7h16v12a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7z" />
      <path d="M3 4h18v3H3z" />
      <path d="M10 12h4" />
    </>,
    className,
  );
}

function IconBackup({ className }: IconProps) {
  return iconShell(
    <>
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <path d="M7 10l5 5 5-5" />
      <path d="M12 15V3" />
    </>,
    className,
  );
}

function IconSettings({ className }: IconProps) {
  return iconShell(
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
    </>,
    className,
  );
}

function IconAbout({ className }: IconProps) {
  return iconShell(
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 10v6M12 7h.01" />
    </>,
    className,
  );
}
