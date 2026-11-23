import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { isTauri } from "@tauri-apps/api/core";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import {
  getCheckUpdatesOnStartup,
  getLastUpdateCheck,
  setCheckUpdatesOnStartup,
  setLastUpdateCheck,
} from "@/lib/updaterPrefs";
import { clearPendingUpdate, readPendingUpdate } from "@/hooks/useUpdaterCheck";

const NOTES_HISTORY_KEY = "carreterie_update_notes";

type NotesEntry = { version: string; notes: string; checkedAt: string };

export function AboutUpdatesPanel() {
  const { isDemo, resetDemo } = useAuth();
  const { showToast } = useToast();
  const [version, setVersion] = useState("…");
  const [checking, setChecking] = useState(false);
  const [update, setUpdate] = useState<Update | null>(null);
  const [pendingHint, setPendingHint] = useState<string | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [installing, setInstalling] = useState(false);
  const [lastCheck, setLastCheck] = useState<string | null>(() => getLastUpdateCheck());
  const [error, setError] = useState<string | null>(null);
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
        showToast(`Mise à jour ${detail.version} disponible — Paramètres.`, "info");
      }
    };
    window.addEventListener("carreterie:update-available", onAvail);
    return () => window.removeEventListener("carreterie:update-available", onAvail);
  }, [showToast]);

  const persistNotes = useCallback((entry: NotesEntry) => {
    setHistory((prev) => {
      const next = [entry, ...prev.filter((e) => e.version !== entry.version)].slice(0, 10);
      try {
        localStorage.setItem(NOTES_HISTORY_KEY, JSON.stringify(next));
      } catch {
        /* ignore */
      }
      return next;
    });
  }, []);

  async function searchUpdates() {
    if (!isTauri()) {
      setError(
        "Les mises à jour nécessitent l'application installée (pas le mode npm run tauri dev).",
      );
      return;
    }
    setChecking(true);
    setError(null);
    setUpdate(null);
    setProgress(null);
    try {
      const result = await check();
      const checkedAt = new Date().toLocaleString("fr-FR");
      setLastCheck(checkedAt);
      setLastUpdateCheck(checkedAt);
      if (result) {
        setUpdate(result);
        setPendingHint(result.version);
        persistNotes({
          version: result.version,
          notes: result.body ?? "Notes de version non fournies.",
          checkedAt,
        });
        showToast(`Mise à jour ${result.version} disponible.`, "info");
      } else {
        setPendingHint(null);
        clearPendingUpdate();
        showToast("Vous êtes à jour.", "success");
      }
    } catch (e) {
      const msg =
        e instanceof Error
          ? e.message
          : "Impossible de vérifier (réseau ou serveur indisponible).";
      setError(msg);
      const checkedAt = new Date().toLocaleString("fr-FR");
      setLastCheck(checkedAt);
      setLastUpdateCheck(checkedAt);
    } finally {
      setChecking(false);
    }
  }

  async function installUpdate() {
    if (!update) return;
    const ok = window.confirm(
      `Installer la version ${update.version} ?\n\nL'application redémarrera. Vos données AppData sont conservées.`,
    );
    if (!ok) return;
    setInstalling(true);
    setError(null);
    setProgress(0);
    try {
      let downloaded = 0;
      let contentLength = 0;
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            contentLength = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            if (contentLength > 0) {
              setProgress(Math.min(100, Math.round((downloaded / contentLength) * 100)));
            }
            break;
          case "Finished":
            setProgress(100);
            break;
        }
      });
      clearPendingUpdate();
      showToast("Mise à jour installée. Redémarrage…", "success");
      await relaunch();
    } catch (e) {
      setError(
        e instanceof Error
          ? e.message
          : "Échec du téléchargement ou de la vérification de signature.",
      );
      setProgress(null);
    } finally {
      setInstalling(false);
    }
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
              Éditeur Nexyros · Dernière vérification : {lastCheck ?? "—"}
            </p>
            {pendingHint && !update ? (
              <p className="text-xs font-medium text-[var(--color-or)]">
                Version {pendingHint} signalée — cliquez sur Rechercher pour confirmer.
              </p>
            ) : null}
          </div>
          <Button
            className="shrink-0"
            loading={checking}
            onClick={() => void searchUpdates()}
            disabled={checking || installing}
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
                  ? "Vérification au démarrage activée."
                  : "Vérification au démarrage désactivée.",
                "info",
              );
            }}
          />
          Vérifier automatiquement les mises à jour au démarrage
        </label>

        {update ? (
          <div className="mt-4 rounded-md border border-[var(--color-or)]/50 bg-[var(--color-surface-elevated)] px-4 py-4">
            <p className="text-base font-semibold text-[var(--color-text)]">
              Nouvelle version disponible : {update.version}
            </p>
            <p className="mt-2 whitespace-pre-wrap text-xs text-[var(--color-text-muted)]">
              {update.body || "Notes de version non fournies."}
            </p>
            {progress != null ? (
              <div className="mt-3">
                <div className="h-2 overflow-hidden rounded bg-[var(--color-border)]">
                  <div
                    className="h-full bg-[var(--color-success)] transition-all"
                    style={{ width: `${progress}%` }}
                  />
                </div>
                <p className="mt-1 text-[11px] text-[var(--color-text-muted)]">
                  Téléchargement {progress} %
                </p>
              </div>
            ) : null}
            <Button
              className="mt-4"
              variant="gold"
              loading={installing}
              onClick={() => void installUpdate()}
              disabled={installing}
            >
              Mettre à jour maintenant
            </Button>
          </div>
        ) : null}

        {error ? (
          <p className="mt-3 text-xs text-[var(--color-danger)]" role="alert">
            {error}
          </p>
        ) : null}

        <p className="mt-3 text-[11px] text-[var(--color-text-muted)]">
          Les mises à jour sont signées. Hors ligne, la facturation reste utilisable. Fonctionne
          avec l&apos;application installée via le setup NSIS.
        </p>
      </Card>

      <Card title="À propos">
        <dl className="grid gap-2 text-sm">
          <div className="flex justify-between gap-4">
            <dt className="text-[var(--color-text-muted)]">Logiciel</dt>
            <dd className="font-medium text-[var(--color-text)]">Carreterie Gestion</dd>
          </div>
          <div className="flex justify-between gap-4">
            <dt className="text-[var(--color-text-muted)]">Éditeur</dt>
            <dd className="font-medium text-[var(--color-or)]">Nexyros</dd>
          </div>
          <div className="flex justify-between gap-4">
            <dt className="text-[var(--color-text-muted)]">Mode actif</dt>
            <dd className="font-medium text-[var(--color-text)]">
              {isDemo ? "Démonstration" : "Production"}
            </dd>
          </div>
        </dl>
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
