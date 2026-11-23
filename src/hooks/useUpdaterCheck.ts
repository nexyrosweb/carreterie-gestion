const PENDING_UPDATE_KEY = "carreterie_pending_update";

/** Conservé pour AppShell : la popup UpdateAvailableModal gère le check au démarrage. */
export function useUpdaterCheck() {
  // no-op : UpdateAvailableModal est montée dans AppShell
}

function parseSemver(v: string): number[] {
  return v
    .trim()
    .replace(/^v/i, "")
    .split(".")
    .map((p) => {
      const n = Number.parseInt(p.replace(/[^0-9].*$/, ""), 10);
      return Number.isFinite(n) ? n : 0;
    });
}

/** true si `candidate` est strictement plus récente que `current`. */
export function isNewerVersion(candidate: string, current: string): boolean {
  const a = parseSemver(candidate);
  const b = parseSemver(current);
  const len = Math.max(a.length, b.length);
  for (let i = 0; i < len; i++) {
    const x = a[i] ?? 0;
    const y = b[i] ?? 0;
    if (x > y) return true;
    if (x < y) return false;
  }
  return false;
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

/** Lit la MAJ en attente ; l’efface si déjà installée (version courante ≥ pending). */
export function readActivePendingUpdate(
  currentVersion: string | null | undefined,
): { version: string; notes: string } | null {
  const pending = readPendingUpdate();
  if (!pending?.version) return null;
  if (currentVersion && !isNewerVersion(pending.version, currentVersion)) {
    clearPendingUpdate();
    return null;
  }
  return pending;
}

export function clearPendingUpdate() {
  try {
    localStorage.removeItem(PENDING_UPDATE_KEY);
  } catch {
    /* ignore */
  }
  try {
    window.dispatchEvent(new CustomEvent("carreterie:update-cleared"));
  } catch {
    /* ignore */
  }
}

/** Depuis Paramètres / sidebar : ouvre la popup si une MAJ GitHub existe. */
export function requestUpdateCheckWithPopup() {
  window.dispatchEvent(new Event("carreterie:force-update-check"));
}
