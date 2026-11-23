import { useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { isTauri } from "@tauri-apps/api/core";

/** Vérifie les mises à jour au démarrage sans bloquer l'UI. */
export function useUpdaterCheck() {
  useEffect(() => {
    if (!isTauri()) return;
    const t = window.setTimeout(() => {
      void check().catch(() => {
        /* hors ligne ou endpoint indisponible — silencieux */
      });
    }, 2500);
    return () => window.clearTimeout(t);
  }, []);
}
