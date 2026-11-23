import { useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { isTauri } from "@tauri-apps/api/core";
import { getCheckUpdatesOnStartup, setLastUpdateCheck } from "@/lib/updaterPrefs";

const PENDING_UPDATE_KEY = "carreterie_pending_update";

/** Vérifie les mises à jour au démarrage sans bloquer l'UI (si activé dans Paramètres). */
export function useUpdaterCheck() {
  useEffect(() => {
    if (!isTauri()) return;
    if (!getCheckUpdatesOnStartup()) return;

    const t = window.setTimeout(() => {
      void check()
        .then((update) => {
          setLastUpdateCheck(new Date().toLocaleString("fr-FR"));
          if (update) {
            try {
              localStorage.setItem(
                PENDING_UPDATE_KEY,
                JSON.stringify({
                  version: update.version,
                  notes: update.body ?? "",
                }),
              );
            } catch {
              /* ignore */
            }
            window.dispatchEvent(
              new CustomEvent("carreterie:update-available", {
                detail: { version: update.version },
              }),
            );
          }
        })
        .catch(() => {
          /* hors ligne ou endpoint indisponible — silencieux */
        });
    }, 2500);

    return () => window.clearTimeout(t);
  }, []);
}

export function readPendingUpdate(): { version: string; notes: string } | null {
  try {
    const raw = localStorage.getItem(PENDING_UPDATE_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as { version: string; notes: string };
  } catch {
    return null;
  }
}

export function clearPendingUpdate() {
  try {
    localStorage.removeItem(PENDING_UPDATE_KEY);
  } catch {
    /* ignore */
  }
}
