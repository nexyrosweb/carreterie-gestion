import { useState, type FormEvent } from "react";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import { ApiError } from "@/lib/api";
import { TheatreLogo } from "@/components/brand/TheatreLogo";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";

export function LockScreen() {
  const { username, unlock, markActivity, isDemo } = useAuth();
  const { showToast } = useToast();
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | undefined>();
  const [loading, setLoading] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError(undefined);
    setLoading(true);
    try {
      await unlock(password);
      markActivity();
      setPassword("");
      showToast("Session déverrouillée.", "success");
    } catch (err) {
      const message =
        err instanceof ApiError ? err.message : "Impossible de déverrouiller la session.";
      setError(message);
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="fixed inset-0 z-[80] flex items-center justify-center bg-[var(--color-page)]/95 backdrop-blur-sm">
      <style>
        {`.lock-logo{border-left:4px solid var(--color-or);padding-left:12px}`}
      </style>
      <form
        onSubmit={(e) => void handleSubmit(e)}
        className="w-full max-w-sm rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-6 shadow-2xl"
      >
        <LockFormCardHeader username={username} isDemo={isDemo} />
        <div className="mt-6 space-y-4">
          {!isDemo ? (
            <Input
              type="password"
              label="Mot de passe"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              error={error}
              required
            />
          ) : error ? (
            <p className="text-sm text-[var(--color-danger)]" role="alert">
              {error}
            </p>
          ) : null}
          <Button type="submit" className="w-full" loading={loading}>
            Déverrouiller
          </Button>
        </div>
      </form>
    </div>
  );
}

function LockFormCardHeader({
  username,
  isDemo,
}: {
  username: string | null;
  isDemo: boolean;
}) {
  return (
    <>
      <div className="flex items-center gap-3">
        <TheatreLogo variant="mark" className="h-12 w-12" />
        <div>
          <p className="text-xs uppercase tracking-widest text-[var(--color-or)]">Session verrouillée</p>
          <h2 className="text-lg font-semibold text-[var(--color-text)]">Carreterie Gestion</h2>
        </div>
      </div>
      <p className="mt-3 text-xs text-[var(--color-text-muted)]">
        Inactivité détectée{username ? ` (${username})` : ""}
        {isDemo ? " – mode démonstration" : ""}.{" "}
        {isDemo
          ? "Cliquez sur Déverrouiller pour reprendre la démo."
          : "Saisissez votre mot de passe pour reprendre."}
      </p>
    </>
  );
}
