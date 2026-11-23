import { useCallback, useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { Modal } from "@/components/ui/Modal";
import { Button } from "@/components/ui/Button";
import { useToast } from "@/context/ToastContext";
import {
  getCheckUpdatesOnStartup,
  setLastUpdateCheck,
} from "@/lib/updaterPrefs";
import { clearPendingUpdate } from "@/hooks/useUpdaterCheck";

const DISMISS_KEY = "carreterie_update_dismissed_version";

function wasDismissed(version: string): boolean {
  try {
    return localStorage.getItem(DISMISS_KEY) === version;
  } catch {
    return false;
  }
}

function markDismissed(version: string) {
  try {
    localStorage.setItem(DISMISS_KEY, version);
  } catch {
    /* ignore */
  }
}

export function UpdateAvailableModal() {
  const { showToast } = useToast();
  const [update, setUpdate] = useState<Update | null>(null);
  const [open, setOpen] = useState(false);
  const [currentVersion, setCurrentVersion] = useState("");
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const presentUpdate = useCallback(
    (u: Update, force = false) => {
      setUpdate(u);
      try {
        localStorage.setItem(
          "carreterie_pending_update",
          JSON.stringify({ version: u.version, notes: u.body ?? "" }),
        );
      } catch {
        /* ignore */
      }
      if (force || !wasDismissed(u.version)) {
        setOpen(true);
        setError(null);
        setProgress(null);
      }
      window.dispatchEvent(
        new CustomEvent("carreterie:update-available", {
          detail: { version: u.version },
        }),
      );
    },
    [],
  );

  const runCheck = useCallback(
    async (opts?: { forcePopup?: boolean }) => {
      if (!isTauri()) return null;
      try {
        const u = await check();
        setLastUpdateCheck(new Date().toLocaleString("fr-FR"));
        if (u) {
          presentUpdate(u, opts?.forcePopup === true);
          return u;
        }
        clearPendingUpdate();
        return null;
      } catch {
        return null;
      }
    },
    [presentUpdate],
  );

  useEffect(() => {
    if (!isTauri()) return;
    void getVersion().then(setCurrentVersion).catch(() => undefined);

    if (getCheckUpdatesOnStartup()) {
      const t = window.setTimeout(() => {
        void runCheck({ forcePopup: false });
      }, 2000);
      return () => window.clearTimeout(t);
    }
    return undefined;
  }, [runCheck]);

  useEffect(() => {
    const onForce = () => {
      void runCheck({ forcePopup: true }).then((u) => {
        if (!u) showToast("Vous êtes à jour.", "success");
      });
    };
    const onSilent = () => {
      void runCheck({ forcePopup: false });
    };
    window.addEventListener("carreterie:force-update-check", onForce);
    window.addEventListener("carreterie:force-update-check-silent", onSilent);
    return () => {
      window.removeEventListener("carreterie:force-update-check", onForce);
      window.removeEventListener("carreterie:force-update-check-silent", onSilent);
    };
  }, [runCheck, showToast]);

  async function install() {
    if (!update) return;
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

  function dismiss() {
    if (update) markDismissed(update.version);
    setOpen(false);
  }

  return (
    <Modal
      open={open && !!update}
      onClose={() => {
        if (!installing) dismiss();
      }}
      title="Nouvelle mise à jour disponible"
      size="sm"
      footer={
        <>
          <Button variant="secondary" onClick={dismiss} disabled={installing}>
            Plus tard
          </Button>
          <Button variant="gold" loading={installing} onClick={() => void install()}>
            Installer maintenant
          </Button>
        </>
      }
    >
      {update ? (
        <div className="space-y-3 text-sm text-[var(--color-text)]">
          <p>
            Une nouvelle version de <strong>Carreterie Gestion</strong> est disponible.
            Installez-la pour bénéficier des dernières corrections Nexyros.
          </p>
          <dl className="grid gap-1 rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs">
            <div className="flex justify-between gap-3">
              <dt className="text-[var(--color-text-muted)]">Version actuelle</dt>
              <dd className="font-medium">{currentVersion || "—"}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-[var(--color-text-muted)]">Nouvelle version</dt>
              <dd className="font-semibold text-[var(--color-or)]">{update.version}</dd>
            </div>
          </dl>
          {update.body ? (
            <div>
              <p className="mb-1 text-xs font-medium text-[var(--color-text-muted)]">
                Notes de version
              </p>
              <p className="max-h-32 overflow-y-auto whitespace-pre-wrap text-xs text-[var(--color-text-muted)]">
                {update.body}
              </p>
            </div>
          ) : null}
          {progress != null ? (
            <div>
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
          {error ? (
            <p className="text-xs text-[var(--color-danger)]" role="alert">
              {error}
            </p>
          ) : null}
          <p className="text-[11px] text-[var(--color-text-muted)]">
            Mise à jour signée. Vos données (clients, factures, sauvegardes) sont conservées.
          </p>
        </div>
      ) : null}
    </Modal>
  );
}
