import { useCallback, useEffect, useState, type FormEvent } from "react";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import { apiAuthChangePassword, apiSettingsGet, apiSettingsUpdate, ApiError } from "@/lib/api";
import { generateSecurePassword, isPasswordStrongEnough } from "@/lib/password";
import type { AppSettings, AppSettingsPatch } from "@/types";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Input } from "@/components/ui/Input";
import { Spinner } from "@/components/ui/Spinner";
import { Textarea } from "@/components/ui/Textarea";
import { TheatreLogo } from "@/components/brand/TheatreLogo";
import { AboutUpdatesPanel } from "@/components/settings/AboutUpdatesPanel";

export function SettingsPage() {
  const { sessionToken, isDemo } = useAuth();
  const { showToast } = useToast();
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [pwd, setPwd] = useState({ current: "", next: "", confirm: "" });

  const load = useCallback(async () => {
    if (!sessionToken) return;
    setLoading(true);
    try {
      setSettings(await apiSettingsGet(sessionToken));
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Chargement paramètres impossible.", "error");
    } finally {
      setLoading(false);
    }
  }, [sessionToken, showToast]);

  useEffect(() => {
    void load();
  }, [load]);

  async function saveSettings(e: FormEvent) {
    e.preventDefault();
    if (!sessionToken || !settings) return;
    const patch: AppSettingsPatch = {
      theatre_name: settings.theatre_name,
      theatre_address: settings.theatre_address,
      theatre_siret: settings.theatre_siret,
      theatre_vat_number: settings.theatre_vat_number,
      logo_path: "brand:logo-theatre-carreterie.jpg",
      invoice_prefix: settings.invoice_prefix,
      credit_note_prefix: settings.credit_note_prefix,
      legal_mentions: settings.legal_mentions,
      default_tax_rate: settings.default_tax_rate,
      export_folder: settings.export_folder,
    };
    setSaving(true);
    try {
      setSettings(await apiSettingsUpdate(sessionToken, patch));
      showToast("Paramètres enregistrés.", "success");
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Enregistrement impossible.", "error");
    } finally {
      setSaving(false);
    }
  }

  function regenerateNewPassword() {
    const generated = generateSecurePassword(12);
    setPwd((prev) => ({ ...prev, next: generated, confirm: generated }));
    showToast("Nouveau mot de passe généré.", "info");
  }

  async function changePassword(e: FormEvent) {
    e.preventDefault();
    if (!sessionToken) return;
    if (!isPasswordStrongEnough(pwd.next, 12)) {
      showToast(
        "Nouveau mot de passe trop faible (12 caractères min., majuscule, minuscule et chiffre).",
        "warning",
      );
      return;
    }
    if (pwd.next !== pwd.confirm) {
      showToast("Les nouveaux mots de passe ne correspondent pas.", "warning");
      return;
    }
    try {
      await apiAuthChangePassword(sessionToken, pwd.current, pwd.next);
      showToast("Mot de passe modifié.", "success");
      setPwd({ current: "", next: "", confirm: "" });
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Modification impossible.", "error");
    }
  }

  if (loading || !settings) {
    return <Spinner className="mx-auto py-20 text-[var(--color-or)]" />;
  }

  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <form onSubmit={(e) => void saveSettings(e)}>
        <Card title="Théâtre et identité">
          <TheatreIdentityFields settings={settings} setSettings={setSettings} />
          <Button type="submit" className="mt-4" loading={saving}>
            Enregistrer
          </Button>
        </Card>
      </form>

      <Card title="Numérotation et fiscalité">
        <div className="grid gap-4 sm:grid-cols-2">
          <Input label="Préfixe factures" value={settings.invoice_prefix} onChange={(e) => setSettings({ ...settings, invoice_prefix: e.target.value })} />
          <Input label="Année factures" value={settings.invoice_year} readOnly />
          <Input label="Dernière séquence facture" type="number" value={String(settings.invoice_last_sequence)} readOnly />
          <Input label="Préfixe avoirs" value={settings.credit_note_prefix} onChange={(e) => setSettings({ ...settings, credit_note_prefix: e.target.value })} />
          <Input
            label="TVA par défaut (%)"
            value={String(settings.default_tax_rate)}
            onChange={(e) => setSettings({ ...settings, default_tax_rate: e.target.value })}
          />
          <Input
            label="Dossier d'export"
            value={settings.export_folder}
            onChange={(e) => setSettings({ ...settings, export_folder: e.target.value })}
            className="sm:col-span-2"
          />
          <Textarea
            label="Mentions légales"
            value={settings.legal_mentions}
            onChange={(e) => setSettings({ ...settings, legal_mentions: e.target.value })}
            className="sm:col-span-2"
          />
        </div>
      </Card>

      {!isDemo ? (
        <form onSubmit={(e) => void changePassword(e)}>
          <Card title="Mot de passe">
            <div className="grid gap-4">
              <Input
                type="password"
                label="Mot de passe actuel"
                value={pwd.current}
                onChange={(e) => setPwd({ ...pwd, current: e.target.value })}
                autoComplete="current-password"
              />
              <Input
                type="text"
                label="Nouveau mot de passe"
                value={pwd.next}
                onChange={(e) => setPwd({ ...pwd, next: e.target.value })}
                autoComplete="new-password"
                hint="12 caractères minimum · modifiable"
              />
              <div className="flex flex-wrap gap-2">
                <Button type="button" variant="ghost" size="sm" onClick={regenerateNewPassword}>
                  Régénérer
                </Button>
              </div>
              <Input
                type="text"
                label="Confirmer"
                value={pwd.confirm}
                onChange={(e) => setPwd({ ...pwd, confirm: e.target.value })}
                autoComplete="new-password"
              />
              <Button type="submit" variant="secondary">
                Changer le mot de passe
              </Button>
            </div>
          </Card>
        </form>
      ) : null}

      <AboutUpdatesPanel />

      <footer className="border-t border-[var(--color-border)] pt-4 text-center text-xs text-[var(--color-text-muted)]">
        <p>© Nexyros · Théâtre La Carreterie</p>
        <p className="mt-1 text-[var(--color-or)]">Développé par Nexyros</p>
      </footer>
    </div>
  );
}

function TheatreIdentityFields({
  settings,
  setSettings,
}: {
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
}) {
  return (
    <div className="grid gap-4">
      <Input label="Nom du théâtre" value={settings.theatre_name} onChange={(e) => setSettings({ ...settings, theatre_name: e.target.value })} />
      <Input label="Adresse" value={settings.theatre_address} onChange={(e) => setSettings({ ...settings, theatre_address: e.target.value })} />
      <div className="grid gap-4 sm:grid-cols-2">
        <Input label="SIRET" value={settings.theatre_siret} onChange={(e) => setSettings({ ...settings, theatre_siret: e.target.value })} />
        <Input label="N° TVA intracommunautaire" value={settings.theatre_vat_number} onChange={(e) => setSettings({ ...settings, theatre_vat_number: e.target.value })} />
      </div>
      <div>
        <p className="mb-2 text-sm font-medium text-[var(--color-text)]">Logo</p>
        <TheatreLogo className="max-w-[280px]" />
        <p className="mt-2 text-xs text-[var(--color-text-muted)]">
          Logo officiel Théâtre La Carreterie — utilisé automatiquement sur les PDF.
        </p>
      </div>
    </div>
  );
}