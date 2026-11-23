const PENDING_UPDATE_KEY = "carreterie_pending_update";

/** Conservé pour AppShell : la popup UpdateAvailableModal gère le check au démarrage. */
export function useUpdaterCheck() {
  // no-op : UpdateAvailableModal est montée dans AppShell
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

/** Depuis Paramètres : ouvre la popup si une MAJ GitHub existe. */
export function requestUpdateCheckWithPopup() {
  window.dispatchEvent(new Event("carreterie:force-update-check"));
}
