const CHECK_ON_STARTUP_KEY = "carreterie_check_updates_on_startup";
const LAST_CHECK_KEY = "carreterie_last_update_check";

export function getCheckUpdatesOnStartup(): boolean {
  try {
    const v = localStorage.getItem(CHECK_ON_STARTUP_KEY);
    return v === null ? true : v === "1";
  } catch {
    return true;
  }
}

export function setCheckUpdatesOnStartup(enabled: boolean) {
  try {
    localStorage.setItem(CHECK_ON_STARTUP_KEY, enabled ? "1" : "0");
  } catch {
    /* ignore */
  }
}

export function getLastUpdateCheck(): string | null {
  try {
    return localStorage.getItem(LAST_CHECK_KEY);
  } catch {
    return null;
  }
}

export function setLastUpdateCheck(isoOrLocale: string) {
  try {
    localStorage.setItem(LAST_CHECK_KEY, isoOrLocale);
  } catch {
    /* ignore */
  }
}
