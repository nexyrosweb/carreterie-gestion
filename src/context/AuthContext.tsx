import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  apiAuthEnterDemo,
  apiAuthFirstLaunch,
  apiAuthLogin,
  apiAuthLogout,
  apiAuthSessionPing,
  apiAuthSetupAdmin,
  apiDemoReset,
  ApiError,
} from "@/lib/api";

const SESSION_KEY = "carreterie_session_token";
const USERNAME_KEY = "carreterie_username";
const DEMO_KEY = "carreterie_is_demo";
const DEFAULT_IDLE_MS = 15 * 60 * 1000;
const TOUCH_INTERVAL_MS = 60 * 1000;

interface AuthContextValue {
  loading: boolean;
  authenticated: boolean;
  locked: boolean;
  needsSetup: boolean;
  disclaimer: string;
  sessionToken: string | null;
  username: string | null;
  isDemo: boolean;
  login: (username: string, password: string) => Promise<void>;
  enterDemo: () => Promise<void>;
  resetDemo: () => Promise<void>;
  setupAdmin: (username: string, password: string, confirm: string) => Promise<void>;
  unlock: (password: string) => Promise<void>;
  logout: () => Promise<void>;
  markActivity: () => void;
  setLocked: (value: boolean) => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);

function readStoredSession(): {
  token: string | null;
  username: string | null;
  isDemo: boolean;
} {
  try {
    return {
      token: localStorage.getItem(SESSION_KEY),
      username: localStorage.getItem(USERNAME_KEY),
      isDemo: localStorage.getItem(DEMO_KEY) === "1",
    };
  } catch {
    return { token: null, username: null, isDemo: false };
  }
}

function persistSession(token: string, username: string, isDemo: boolean) {
  try {
    localStorage.setItem(SESSION_KEY, token);
    localStorage.setItem(USERNAME_KEY, username);
    localStorage.setItem(DEMO_KEY, isDemo ? "1" : "0");
  } catch {
    /* ignore */
  }
}

function clearStoredSession() {
  try {
    localStorage.removeItem(SESSION_KEY);
    localStorage.removeItem(USERNAME_KEY);
    localStorage.removeItem(DEMO_KEY);
  } catch {
    /* ignore */
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [loading, setLoading] = useState(true);
  const [sessionToken, setSessionToken] = useState<string | null>(null);
  const [username, setUsername] = useState<string | null>(null);
  const [isDemo, setIsDemo] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);
  const [locked, setLocked] = useState(false);
  const [needsSetup, setNeedsSetup] = useState(false);
  const [disclaimer, setDisclaimer] = useState("");

  const lastActivityRef = useRef(Date.now());

  const applyLogin = useCallback((token: string, name: string, demo: boolean) => {
    setSessionToken(token);
    setUsername(name);
    setIsDemo(demo);
    setAuthenticated(true);
    setLocked(false);
    persistSession(token, name, demo);
    lastActivityRef.current = Date.now();
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        const first = await apiAuthFirstLaunch();
        setNeedsSetup(first.needs_setup);
        setDisclaimer(first.disclaimer);

        const stored = readStoredSession();
        if (stored.token) {
          try {
            const ping = await apiAuthSessionPing(stored.token);
            applyLogin(ping.token, ping.username, Boolean(ping.is_demo));
          } catch {
            clearStoredSession();
            setSessionToken(null);
            setUsername(stored.username);
            setIsDemo(false);
            setAuthenticated(false);
          }
        }
      } catch {
        setNeedsSetup(false);
      } finally {
        setLoading(false);
      }
    })();
  }, [applyLogin]);

  const markActivity = useCallback(() => {
    lastActivityRef.current = Date.now();
    if (locked) return;
  }, [locked]);

  useEffect(() => {
    const onActivity = () => markActivity();
    window.addEventListener("mousemove", onActivity);
    window.addEventListener("keydown", onActivity);
    window.addEventListener("mousedown", onActivity);
    window.addEventListener("scroll", onActivity, true);
    return () => {
      window.removeEventListener("mousemove", onActivity);
      window.removeEventListener("keydown", onActivity);
      window.removeEventListener("mousedown", onActivity);
      window.removeEventListener("scroll", onActivity, true);
    };
  }, [markActivity]);

  useEffect(() => {
    if (!authenticated || locked || !sessionToken) return;

    const idleCheck = window.setInterval(() => {
      if (Date.now() - lastActivityRef.current >= DEFAULT_IDLE_MS) {
        setLocked(true);
      }
    }, 5000);

    const touchTimer = window.setInterval(() => {
      if (Date.now() - lastActivityRef.current < DEFAULT_IDLE_MS) {
        void apiAuthSessionPing(sessionToken)
          .then((r) => {
            if (r.token !== sessionToken || Boolean(r.is_demo) !== isDemo) {
              applyLogin(r.token, r.username, Boolean(r.is_demo));
            }
          })
          .catch(() => undefined);
      }
    }, TOUCH_INTERVAL_MS);

    return () => {
      window.clearInterval(idleCheck);
      window.clearInterval(touchTimer);
    };
  }, [authenticated, locked, sessionToken, isDemo, applyLogin]);

  const login = useCallback(
    async (user: string, password: string) => {
      const result = await apiAuthLogin(user, password);
      applyLogin(result.token, result.username, false);
      setDisclaimer(result.disclaimer);
      setNeedsSetup(false);
    },
    [applyLogin],
  );

  const enterDemo = useCallback(async () => {
    const result = await apiAuthEnterDemo();
    applyLogin(result.token, result.username, true);
    setDisclaimer(result.disclaimer);
  }, [applyLogin]);

  const resetDemo = useCallback(async () => {
    if (!sessionToken || !isDemo) {
      throw new ApiError("Réinitialisation réservée au mode démonstration.", "INTERDIT");
    }
    await apiDemoReset(sessionToken);
    const result = await apiAuthEnterDemo();
    applyLogin(result.token, result.username, true);
  }, [sessionToken, isDemo, applyLogin]);

  const setupAdmin = useCallback(
    async (user: string, password: string, confirm: string) => {
      if (password !== confirm) {
        throw new ApiError("Les mots de passe ne correspondent pas.", "VALIDATION");
      }
      await apiAuthSetupAdmin(user, password);
      const result = await apiAuthLogin(user, password);
      applyLogin(result.token, result.username, false);
      setDisclaimer(result.disclaimer);
      setNeedsSetup(false);
    },
    [applyLogin],
  );

  const unlock = useCallback(
    async (password: string) => {
      if (isDemo) {
        const result = await apiAuthEnterDemo();
        applyLogin(result.token, result.username, true);
        return;
      }
      const name = username ?? readStoredSession().username;
      if (!name) {
        throw new ApiError("Identifiant manquant. Reconnectez-vous.", "SESSION_INVALIDE");
      }
      const result = await apiAuthLogin(name, password);
      applyLogin(result.token, result.username, false);
    },
    [username, applyLogin, isDemo],
  );

  const logout = useCallback(async () => {
    const token = sessionToken;
    if (token) {
      try {
        await apiAuthLogout(token);
      } catch {
        /* session déjà expirée */
      }
    }
    clearStoredSession();
    setSessionToken(null);
    setUsername(null);
    setIsDemo(false);
    setAuthenticated(false);
    setLocked(false);
  }, [sessionToken]);

  const value = useMemo<AuthContextValue>(
    () => ({
      loading,
      authenticated,
      locked,
      needsSetup,
      disclaimer,
      sessionToken,
      username,
      isDemo,
      login,
      enterDemo,
      resetDemo,
      setupAdmin,
      unlock,
      logout,
      markActivity,
      setLocked,
    }),
    [
      loading,
      authenticated,
      locked,
      needsSetup,
      disclaimer,
      sessionToken,
      username,
      isDemo,
      login,
      enterDemo,
      resetDemo,
      setupAdmin,
      unlock,
      logout,
      markActivity,
    ],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth doit être utilisé dans AuthProvider");
  }
  return ctx;
}