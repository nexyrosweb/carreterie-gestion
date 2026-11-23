import { Outlet, useLocation } from "react-router-dom";
import { useAuth } from "@/context/AuthContext";
import { UpdateAvailableModal } from "@/components/updater/UpdateAvailableModal";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { useUpdaterCheck } from "@/hooks/useUpdaterCheck";

const titles: Record<string, string> = {
  "/": "Tableau de bord",
  "/factures-clients": "Factures clients",
  "/factures-fournisseurs": "Factures fournisseurs",
  "/import-pdf": "Import PDF",
  "/contacts": "Contacts",
  "/archives": "Archives",
  "/sauvegardes": "Sauvegardes",
  "/parametres": "Paramètres",
  "/a-propos": "À propos",
};

export function AppShell() {
  const { pathname } = useLocation();
  const title = titles[pathname] ?? "Carreterie Gestion";
  const { isDemo } = useAuth();
  useUpdaterCheck();

  return (
    <div className="flex h-screen overflow-hidden bg-[var(--color-page)]">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        {isDemo ? (
          <div
            className="flex items-center justify-center gap-2 bg-[var(--color-or)] px-4 py-1.5 text-center text-xs font-semibold tracking-wide text-[var(--color-page)]"
            role="status"
          >
            MODE DÉMONSTRATION — données fictives, sans valeur comptable
          </div>
        ) : null}
        <TopBar title={title} />
        <main className="flex-1 overflow-y-auto p-6">
          <Outlet />
        </main>
      </div>
      <UpdateAvailableModal />
    </div>
  );
}
