import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import {
  getCheckUpdatesOnStartup,
  getLastUpdateCheck,
  setCheckUpdatesOnStartup,
} from "@/lib/updaterPrefs";
import {
  readPendingUpdate,
  requestUpdateCheckWithPopup,
} from "@/hooks/useUpdaterCheck";

const NOTES_HISTORY_KEY = "carreterie_update_notes";

type NotesEntry = { version: string; notes: string; checkedAt: string };

export function AboutUpdatesPanel() {
  const { isDemo, resetDemo } = useAuth();
  const { showToast } = useToast();
  const [version, setVersion] = useState("…");
  const [checking, setChecking] = useState(false);
  const [pendingHint, setPendingHint] = useState<string | null>(null);
  const [lastCheck, setLastCheck] = useState<string | null>(() => getLastUpdateCheck());
  const [history, setHistory] = useState<NotesEntry[]>([]);
  const [resetting, setResetting] = useState(false);
  const [checkOnStartup, setCheckOnStartup] = useState(getCheckUpdatesOnStartup);

  useEffect(() => {
    if (!isTauri()) {
      setVersion("aperçu web");
      return;
    }
    void getVersion().then(setVersion).catch(() => setVersion("?"));
    try {
      const raw = localStorage.getItem(NOTES_HISTORY_KEY);
      if (raw) setHistory(JSON.parse(raw) as NotesEntry[]);
    } catch {
      /* ignore */
    }
    const pending = readPendingUpdate();
    if (pending?.version) setPendingHint(pending.version);
  }, []);

  useEffect(() => {
    const onAvail = (ev: Event) => {
      const detail = (ev as CustomEvent<{ version: string }>).detail;
      if (detail?.version) {
        setPendingHint(detail.version);
        setLastCheck(getLastUpdateCheck());
      }
    };
    window.addEventListener("carreterie:update-available", onAvail);
    return () => window.removeEventListener("carreterie:update-available", onAvail);
  }, []);

  function searchUpdates() {
    if (!isTauri()) {
      showToast(
        "Les mises à jour nécessitent l'application installée (setup NSIS).",
        "warning",
      );
      return;
    }
    setChecking(true);
    requestUpdateCheckWithPopup();
    window.setTimeout(() => {
      setChecking(false);
      setLastCheck(getLastUpdateCheck());
    }, 4000);
  }

  return (
    <div className="space-y-6">
      <Card title="Mises à jour">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="space-y-1 text-sm">
            <p>
              <span className="text-[var(--color-text-muted)]">Version installée :</span>{" "}
              <span className="font-semibold text-[var(--color-text)]">{version}</span>
            </p>
            <p className="text-xs text-[var(--color-text-muted)]">
              Éditeur Nexyros – Dernière vérification : {lastCheck ?? "—"}
            </p>
            {pendingHint ? (
              <p className="text-xs font-medium text-[var(--color-or)]">
                Version {pendingHint} disponible – une fenêtre d&apos;installation s&apos;affiche
                automatiquement.
              </p>
            ) : null}
          </div>
          <Button
            className="shrink-0"
            loading={checking}
            onClick={searchUpdates}
            disabled={checking}
          >
            Rechercher des mises à jour
          </Button>
        </div>

        <label className="mt-4 flex cursor-pointer items-center gap-2 text-sm text-[var(--color-text)]">
          <input
            type="checkbox"
            className="h-4 w-4 accent-[var(--color-rouge)]"
            checked={checkOnStartup}
            onChange={(e) => {
              const on = e.target.checked;
              setCheckOnStartup(on);
              setCheckUpdatesOnStartup(on);
              showToast(
                on
                  ? "Popup de mise à jour au démarrage activée."
                  : "Vérification au démarrage désactivée.",
                "info",
              );
            }}
          />
          Afficher une popup au démarrage si une nouvelle version GitHub est disponible
        </label>

        <p className="mt-3 text-[11px] text-[var(--color-text-muted)]">
          Les mises à jour sont signées. Hors ligne, la facturation reste utilisable.
        </p>
      </Card>

      {history.length > 0 ? (
        <Card title="Historique des nouveautés">
          <ul className="space-y-3 text-sm">
            {history.map((h) => (
              <li
                key={`${h.version}-${h.checkedAt}`}
                className="border-b border-[var(--color-border)] pb-2 last:border-0"
              >
                <p className="font-medium text-[var(--color-text)]">v{h.version}</p>
                <p className="text-[11px] text-[var(--color-text-muted)]">{h.checkedAt}</p>
                <p className="mt-1 whitespace-pre-wrap text-xs text-[var(--color-text-muted)]">
                  {h.notes}
                </p>
              </li>
            ))}
          </ul>
        </Card>
      ) : null}

      {isDemo ? (
        <Card title="Démonstration">
          <p className="text-sm text-[var(--color-text-muted)]">
            Réinitialise demo.db et les pièces jointes de démonstration. La base de production
            n&apos;est jamais modifiée.
          </p>
          <Button
            className="mt-3"
            variant="danger"
            loading={resetting}
            onClick={() => {
              if (!window.confirm("Réinitialiser toutes les données de démonstration ?")) return;
              setResetting(true);
              void resetDemo()
                .then(() => showToast("Démonstration réinitialisée.", "success"))
                .catch((e) =>
                  showToast(
                    e instanceof Error ? e.message : "Échec de la réinitialisation.",
                    "error",
                  ),
                )
                .finally(() => setResetting(false));
            }}
          >
            Réinitialiser la démonstration
          </Button>
        </Card>
      ) : null}
    </div>
  );
}
