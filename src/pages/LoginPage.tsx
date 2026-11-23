import { useEffect, useState, type FormEvent } from "react";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import { ApiError } from "@/lib/api";
import { generateSecurePassword, isPasswordStrongEnough } from "@/lib/password";
import { TheatreLogo } from "@/components/brand/TheatreLogo";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { ThemeToggle } from "@/components/ui/ThemeToggle";

const ADMIN_PASSWORD_LENGTH = 12;

export function LoginPage() {
  const { needsSetup, login, setupAdmin, enterDemo, disclaimer } = useAuth();
  const { showToast } = useToast();
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [errors, setErrors] = useState<{
    username?: string;
    password?: string;
    confirm?: string;
    form?: string;
  }>({});
  const [loading, setLoading] = useState(false);
  const [demoLoading, setDemoLoading] = useState(false);

  useEffect(() => {
    if (!needsSetup) return;
    const generated = generateSecurePassword(ADMIN_PASSWORD_LENGTH);
    setPassword(generated);
    setConfirm(generated);
    setShowPassword(true);
  }, [needsSetup]);

  function regeneratePassword() {
    const generated = generateSecurePassword(ADMIN_PASSWORD_LENGTH);
    setPassword(generated);
    setConfirm(generated);
    setShowPassword(true);
    setErrors((prev) => ({ ...prev, password: undefined, confirm: undefined }));
    showToast("Nouveau mot de passe généré.", "info");
  }

  async function copyPassword() {
    if (!password) return;
    try {
      await navigator.clipboard.writeText(password);
      showToast("Mot de passe copié dans le presse-papiers.", "success");
    } catch {
      showToast("Impossible de copier automatiquement. Sélectionnez-le manuellement.", "warning");
    }
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    const next: typeof errors = {};
    if (!username.trim()) next.username = "L'identifiant est obligatoire.";

    if (needsSetup) {
      if (!isPasswordStrongEnough(password, ADMIN_PASSWORD_LENGTH)) {
        next.password =
          "Mot de passe trop faible (12 caractères min., majuscule, minuscule et chiffre).";
      }
      if (password !== confirm) {
        next.confirm = "Les mots de passe ne correspondent pas.";
      }
    } else if (password.length < 8) {
      next.password = "Mot de passe requis.";
    }

    setErrors(next);
    if (Object.keys(next).length > 0) return;

    setLoading(true);
    setErrors({});
    try {
      if (needsSetup) {
        await setupAdmin(username.trim(), password, confirm);
        showToast("Compte administrateur créé. Conservez bien ce mot de passe.", "success");
      } else {
        await login(username.trim(), password);
        showToast("Connexion réussie.", "success");
      }
      setPassword("");
      setConfirm("");
    } catch (err) {
      const message =
        err instanceof ApiError ? err.message : "Échec de l'authentification.";
      setErrors({ form: message });
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="relative flex min-h-screen items-center justify-center bg-[var(--color-page)] p-6">
      <div className="absolute right-4 top-4">
        <ThemeToggle />
      </div>
      <div className="w-full max-w-md">
        <div className="mb-6 flex flex-col items-center text-center">
          <TheatreLogo className="mb-4 max-w-[280px]" />
          <h1 className="text-2xl font-bold text-[var(--color-text)]">Carreterie Gestion</h1>
          <p className="mt-2 text-sm text-[var(--color-text-muted)]">
            {needsSetup
              ? "Installation : créez d'abord le compte administrateur local."
              : "Connectez-vous pour accéder à la gestion."}
          </p>
        </div>

        <form
          onSubmit={(e) => void handleSubmit(e)}
          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-6 shadow-xl"
        >
          {needsSetup ? (
            <div className="mb-4 rounded-md border border-[var(--color-or)]/40 bg-[var(--color-surface-elevated)] px-3 py-2 text-xs text-[var(--color-text)]">
              Un mot de passe sécurisé de {ADMIN_PASSWORD_LENGTH} caractères a été généré.
              Vous pouvez le modifier ou en régénérer un autre avant de valider.
              Notez-le dans un endroit sûr.
            </div>
          ) : null}

          {errors.form ? (
            <p className="mb-4 text-sm text-[var(--color-danger)]" role="alert">
              {errors.form}
            </p>
          ) : null}

          <div className="space-y-4">
            <Input
              label="Identifiant administrateur"
              autoComplete="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              error={errors.username}
              required
            />

            <div className="space-y-2">
              <Input
                type={showPassword || needsSetup ? "text" : "password"}
                label={needsSetup ? "Mot de passe administrateur" : "Mot de passe"}
                autoComplete={needsSetup ? "new-password" : "current-password"}
                value={password}
                onChange={(e) => {
                  setPassword(e.target.value);
                  if (needsSetup) setConfirm(e.target.value);
                }}
                error={errors.password}
                hint={
                  needsSetup
                    ? `${password.length}/${ADMIN_PASSWORD_LENGTH}+ caractères – modifiable`
                    : undefined
                }
                required
              />

              {needsSetup ? (
                <div className="flex flex-wrap gap-2">
                  <Button type="button" variant="secondary" size="sm" onClick={regeneratePassword}>
                    Régénérer
                  </Button>
                  <Button type="button" variant="ghost" size="sm" onClick={() => void copyPassword()}>
                    Copier
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => setShowPassword((v) => !v)}
                  >
                    {showPassword ? "Masquer" : "Afficher"}
                  </Button>
                </div>
              ) : null}
            </div>

            {needsSetup ? (
              <Input
                type={showPassword ? "text" : "password"}
                label="Confirmer le mot de passe"
                autoComplete="new-password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                error={errors.confirm}
                required
              />
            ) : null}
          </div>

          <Button type="submit" className="mt-6 w-full" loading={loading}>
            {needsSetup ? "Créer le compte administrateur" : "Connexion administrateur"}
          </Button>

          {!needsSetup ? (
            <div className="mt-4 border-t border-[var(--color-border)] pt-4">
              <Button
                type="button"
                variant="secondary"
                className="w-full"
                loading={demoLoading}
                onClick={() => {
                  setDemoLoading(true);
                  void enterDemo()
                    .then(() => showToast("Mode démonstration activé.", "info"))
                    .catch((err) =>
                      setErrors({
                        form:
                          err instanceof ApiError
                            ? err.message
                            : "Impossible de démarrer la démonstration.",
                      }),
                    )
                    .finally(() => setDemoLoading(false));
                }}
              >
                Essayer la démonstration
              </Button>
              <p className="mt-2 text-center text-[11px] text-[var(--color-text-muted)]">
                Base isolée – données fictives – sans valeur comptable
              </p>
            </div>
          ) : null}
        </form>

        <aside className="mt-6 space-y-2 text-xs text-[var(--color-text-muted)]">
          <p>
            Les données restent sur cet ordinateur. Activez{" "}
            <strong className="text-[var(--color-text)]">BitLocker</strong> (ou un chiffrement
            disque équivalent) pour protéger les sauvegardes en cas de vol ou de perte du matériel.
          </p>
          <p>{disclaimer}</p>
          <p className="pt-2">© Nexyros – Théâtre La Carreterie</p>
        </aside>
      </div>
    </div>
  );
}
