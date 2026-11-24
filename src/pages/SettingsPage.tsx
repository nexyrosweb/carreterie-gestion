import { useCallback, useEffect, useState, type FormEvent } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import {
  apiAuthChangePassword,
  apiAuthProfileGet,
  apiAuthProfileSetAvatar,
  apiAuthProfileUpdate,
  apiMailTestConnection,
  apiSettingsGet,
  apiSettingsSetLogo,
  apiSettingsUpdate,
  ApiError,
} from "@/lib/api";
import { generateSecurePassword, isPasswordStrongEnough } from "@/lib/password";
import {
  digitsOnly,
  formatSiretDisplay,
  frenchVatFromSiret,
  isValidSiret,
  siretFeedback,
} from "@/lib/siret";
import type { AppSettings, AppSettingsPatch, UserProfile } from "@/types";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Input } from "@/components/ui/Input";
import { Select } from "@/components/ui/Select";
import { Spinner } from "@/components/ui/Spinner";
import { Textarea } from "@/components/ui/Textarea";
import { TheatreLogo } from "@/components/brand/TheatreLogo";

export function SettingsPage() {
  const { sessionToken, isDemo } = useAuth();
  const { showToast } = useToast();
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [profile, setProfile] = useState<UserProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [savingProfile, setSavingProfile] = useState(false);
  const [pwd, setPwd] = useState({ current: "", next: "", confirm: "" });

  const load = useCallback(async () => {
    if (!sessionToken) return;
    setLoading(true);
    try {
      const [s, p] = await Promise.all([
        apiSettingsGet(sessionToken),
        apiAuthProfileGet(sessionToken),
      ]);
      setSettings(s);
      setProfile(p);
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
      logo_path: settings.logo_path || "brand:logo-theatre-carreterie.jpg",
      invoice_prefix: settings.invoice_prefix,
      credit_note_prefix: settings.credit_note_prefix,
      legal_mentions: settings.legal_mentions,
      default_tax_rate: settings.default_tax_rate,
      export_folder: settings.export_folder,
      smtp_enabled: settings.smtp_enabled,
      smtp_host: settings.smtp_host,
      smtp_port: settings.smtp_port,
      smtp_encryption: settings.smtp_encryption,
      smtp_username: settings.smtp_username,
      smtp_password: settings.smtp_password,
      smtp_from_name: settings.smtp_from_name,
      smtp_from_email: settings.smtp_from_email,
      smtp_reply_to: settings.smtp_reply_to,
      smtp_cc: settings.smtp_cc,
      smtp_bcc: settings.smtp_bcc,
      smtp_timeout_secs: settings.smtp_timeout_secs,
      devis_prefix: settings.devis_prefix,
      bank_iban: settings.bank_iban,
      bank_account: settings.bank_account,
      bank_bic: settings.bank_bic,
      bank_name: settings.bank_name,
      bank_holder: settings.bank_holder,
      pdf_primary_color: settings.pdf_primary_color,
      pdf_header_subtitle: settings.pdf_header_subtitle,
      pdf_use_custom_logo: settings.pdf_use_custom_logo,
      pdf_template: settings.pdf_template,
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

  async function saveProfile(e: FormEvent) {
    e.preventDefault();
    if (!sessionToken || !profile || isDemo) return;
    setSavingProfile(true);
    try {
      setProfile(
        await apiAuthProfileUpdate(sessionToken, {
          email: profile.email,
          first_name: profile.first_name,
          last_name: profile.last_name,
        }),
      );
      showToast("Profil enregistré.", "success");
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Profil non enregistré.", "error");
    } finally {
      setSavingProfile(false);
    }
  }

  async function pickAvatar() {
    if (!sessionToken || isDemo) return;
    const selected = await open({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
      title: "Choisir un logo de profil",
    });
    if (!selected || Array.isArray(selected)) return;
    try {
      setProfile(await apiAuthProfileSetAvatar(sessionToken, selected));
      showToast("Logo de profil mis à jour.", "success");
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Logo non mis à jour.", "error");
    }
  }

  async function pickPdfLogo() {
    if (!sessionToken) return;
    const selected = await open({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }],
      title: "Choisir un logo PDF",
    });
    if (!selected || Array.isArray(selected)) return;
    try {
      setSettings(await apiSettingsSetLogo(sessionToken, selected));
      showToast("Logo PDF mis à jour.", "success");
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Logo PDF non mis à jour.", "error");
    }
  }

  async function testSmtp() {
    if (!sessionToken) return;
    try {
      const msg = await apiMailTestConnection(sessionToken);
      showToast(msg, "success");
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Test SMTP échoué.", "error");
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
      {profile && !isDemo ? (
        <form onSubmit={(e) => void saveProfile(e)}>
          <Card title="Profil">
            <div className="grid gap-4">
              <div className="flex flex-wrap items-center gap-4">
                <ProfileAvatar profile={profile} />
                <div className="space-y-2">
                  <p className="text-sm text-[var(--color-text-muted)]">
                    Identifiant :{" "}
                    <span className="font-medium text-[var(--color-text)]">{profile.username}</span>
                  </p>
                  <Button type="button" variant="secondary" size="sm" onClick={() => void pickAvatar()}>
                    Changer le logo
                  </Button>
                </div>
              </div>
              <div className="grid gap-4 sm:grid-cols-2">
                <Input
                  label="Prénom"
                  value={profile.first_name}
                  onChange={(e) => setProfile({ ...profile, first_name: e.target.value })}
                />
                <Input
                  label="Nom"
                  value={profile.last_name}
                  onChange={(e) => setProfile({ ...profile, last_name: e.target.value })}
                />
              </div>
              <Input
                label="E-mail"
                type="email"
                value={profile.email}
                onChange={(e) => setProfile({ ...profile, email: e.target.value })}
              />
              <Button type="submit" loading={savingProfile}>
                Enregistrer le profil
              </Button>
            </div>
          </Card>
        </form>
      ) : null}

      <form onSubmit={(e) => void saveSettings(e)}>
        <Card title="Théâtre et identité">
          <TheatreIdentityFields settings={settings} setSettings={setSettings} />
          <Button type="submit" className="mt-4" loading={saving}>
            Enregistrer
          </Button>
        </Card>
      </form>

      <form onSubmit={(e) => void saveSettings(e)}>
        <Card title="Serveur SMTP">
          <div className="grid gap-4 sm:grid-cols-2">
            <label className="flex items-center gap-2 text-sm text-[var(--color-text)] sm:col-span-2">
              <input
                type="checkbox"
                className="h-4 w-4 accent-[var(--color-rouge)]"
                checked={settings.smtp_enabled}
                onChange={(e) => setSettings({ ...settings, smtp_enabled: e.target.checked })}
              />
              Activer l&apos;envoi SMTP
            </label>
            <Input
              label="Hôte SMTP"
              value={settings.smtp_host}
              onChange={(e) => setSettings({ ...settings, smtp_host: e.target.value })}
              placeholder="smtp.exemple.fr"
            />
            <Input
              label="Port"
              type="number"
              value={String(settings.smtp_port)}
              onChange={(e) =>
                setSettings({
                  ...settings,
                  smtp_port: Number.parseInt(e.target.value || "587", 10) || 587,
                })
              }
            />
            <Select
              label="Chiffrement"
              value={settings.smtp_encryption}
              onChange={(e) => setSettings({ ...settings, smtp_encryption: e.target.value })}
              options={[
                { value: "none", label: "Aucun" },
                { value: "starttls", label: "STARTTLS" },
                { value: "ssl", label: "SSL / TLS" },
              ]}
            />
            <Input
              label="Délai d'attente (s)"
              type="number"
              value={String(settings.smtp_timeout_secs)}
              onChange={(e) =>
                setSettings({
                  ...settings,
                  smtp_timeout_secs: Number.parseInt(e.target.value || "30", 10) || 30,
                })
              }
            />
            <Input
              label="Identifiant SMTP"
              value={settings.smtp_username}
              onChange={(e) => setSettings({ ...settings, smtp_username: e.target.value })}
              autoComplete="off"
            />
            <Input
              label="Mot de passe SMTP"
              type="password"
              value={settings.smtp_password}
              onChange={(e) => setSettings({ ...settings, smtp_password: e.target.value })}
              autoComplete="new-password"
            />
            <Input
              label="Nom de l'expéditeur"
              value={settings.smtp_from_name}
              onChange={(e) => setSettings({ ...settings, smtp_from_name: e.target.value })}
            />
            <Input
              label="E-mail de l'expéditeur"
              type="email"
              value={settings.smtp_from_email}
              onChange={(e) => setSettings({ ...settings, smtp_from_email: e.target.value })}
            />
            <Input
              label="Reply-To"
              type="email"
              value={settings.smtp_reply_to}
              onChange={(e) => setSettings({ ...settings, smtp_reply_to: e.target.value })}
            />
            <Input
              label="Cc (séparés par des virgules)"
              value={settings.smtp_cc}
              onChange={(e) => setSettings({ ...settings, smtp_cc: e.target.value })}
            />
            <Input
              label="Cci (séparés par des virgules)"
              value={settings.smtp_bcc}
              onChange={(e) => setSettings({ ...settings, smtp_bcc: e.target.value })}
              className="sm:col-span-2"
            />
          </div>
          <div className="mt-4 flex flex-wrap gap-2">
            <Button type="submit" loading={saving}>
              Enregistrer le SMTP
            </Button>
            <Button
              type="button"
              variant="secondary"
              onClick={() => void testSmtp()}
            >
              Tester la connexion
            </Button>
          </div>
        </Card>
      </form>

      <form onSubmit={(e) => void saveSettings(e)}>
        <Card title="Thème PDF">
          <div className="grid gap-4 sm:grid-cols-2">
            <Select
              label="Modèle de facture"
              value={settings.pdf_template || "classique"}
              onChange={(e) => setSettings({ ...settings, pdf_template: e.target.value })}
              options={[
                { value: "classique", label: "Classique" },
                { value: "compact", label: "Compact" },
                { value: "billetterie", label: "Billetterie" },
              ]}
            />
            <Input
              label="Couleur principale (hex)"
              value={settings.pdf_primary_color}
              onChange={(e) =>
                setSettings({ ...settings, pdf_primary_color: e.target.value })
              }
              placeholder="#8B1E1E"
            />
            <Input
              label="Sous-titre en-tête"
              value={settings.pdf_header_subtitle}
              onChange={(e) =>
                setSettings({ ...settings, pdf_header_subtitle: e.target.value })
              }
              placeholder="ex. Scène nationale"
              className="sm:col-span-2"
            />
            <label className="flex items-center gap-2 text-sm sm:col-span-2">
              <input
                type="checkbox"
                checked={settings.pdf_use_custom_logo}
                onChange={(e) =>
                  setSettings({ ...settings, pdf_use_custom_logo: e.target.checked })
                }
              />
              Utiliser le logo personnalisé (sinon logo théâtre embarqué)
            </label>
            <div className="sm:col-span-2">
              <Button type="button" variant="secondary" size="sm" onClick={() => void pickPdfLogo()}>
                Choisir un logo PDF
              </Button>
              {settings.logo_path && !settings.logo_path.startsWith("brand:") ? (
                <p className="mt-2 text-xs text-[var(--color-text-muted)]">{settings.logo_path}</p>
              ) : null}
            </div>
          </div>
          <Button type="submit" className="mt-4" loading={saving}>
            Enregistrer le thème PDF
          </Button>
        </Card>
      </form>

      <form onSubmit={(e) => void saveSettings(e)}>
        <Card title="Numérotation, banque et fiscalité">
          <div className="grid gap-4 sm:grid-cols-2">
            <Input label="Préfixe factures" value={settings.invoice_prefix} onChange={(e) => setSettings({ ...settings, invoice_prefix: e.target.value })} />
            <Input label="Préfixe devis" value={settings.devis_prefix} onChange={(e) => setSettings({ ...settings, devis_prefix: e.target.value })} />
            <Input label="Préfixe avoirs" value={settings.credit_note_prefix} onChange={(e) => setSettings({ ...settings, credit_note_prefix: e.target.value })} />
            <Input label="Année" value={settings.invoice_year} readOnly />
            <Input
              label="IBAN"
              value={settings.bank_iban}
              onChange={(e) => setSettings({ ...settings, bank_iban: e.target.value })}
            />
            <Input
              label="BIC"
              value={settings.bank_bic}
              onChange={(e) => setSettings({ ...settings, bank_bic: e.target.value.toUpperCase() })}
              placeholder="BNPAFRPPXXX"
            />
            <Input
              label="Banque"
              value={settings.bank_name}
              onChange={(e) => setSettings({ ...settings, bank_name: e.target.value })}
              placeholder="ex. Banque Populaire"
            />
            <Input
              label="Titulaire du compte"
              value={settings.bank_holder}
              onChange={(e) => setSettings({ ...settings, bank_holder: e.target.value })}
              placeholder="ex. Théâtre La Carreterie"
            />
            <Input
              label="Compte bancaire N° (optionnel)"
              value={settings.bank_account}
              onChange={(e) => setSettings({ ...settings, bank_account: e.target.value })}
            />
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
          <Button type="submit" className="mt-4" loading={saving}>
            Enregistrer
          </Button>
        </Card>
      </form>

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
                hint="12 caractères minimum – modifiable"
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

      <footer className="border-t border-[var(--color-border)] pt-4 text-center text-xs text-[var(--color-text-muted)]">
        <p>© Nexyros – Théâtre La Carreterie</p>
        <p className="mt-1 text-[var(--color-or)]">Développé par Nexyros</p>
      </footer>
    </div>
  );
}

function ProfileAvatar({ profile }: { profile: UserProfile }) {
  const initials =
    `${profile.first_name.charAt(0)}${profile.last_name.charAt(0)}`.toUpperCase() ||
    profile.username.slice(0, 2).toUpperCase();
  if (profile.avatar_path) {
    return (
      <img
        src={convertFileSrc(profile.avatar_path)}
        alt="Logo de profil"
        className="h-16 w-16 rounded-full object-cover ring-1 ring-[var(--color-border)]"
      />
    );
  }
  return (
    <div className="flex h-16 w-16 items-center justify-center rounded-full bg-[var(--color-surface-elevated)] text-sm font-semibold text-[var(--color-or)] ring-1 ring-[var(--color-border)]">
      {initials}
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
  const siretFb = siretFeedback(settings.theatre_siret);
  const sirenDigits = digitsOnly(settings.theatre_siret).slice(0, 9);

  function onSiretChange(raw: string) {
    const display = formatSiretDisplay(raw);
    const digits = digitsOnly(raw);
    let vat = settings.theatre_vat_number;
    if (digits.length === 14 && isValidSiret(digits)) {
      vat = frenchVatFromSiret(digits) ?? vat;
    }
    setSettings({ ...settings, theatre_siret: display, theatre_vat_number: vat });
  }

  return (
    <div className="grid gap-4">
      <Input label="Nom du théâtre" value={settings.theatre_name} onChange={(e) => setSettings({ ...settings, theatre_name: e.target.value })} />
      <Input label="Adresse" value={settings.theatre_address} onChange={(e) => setSettings({ ...settings, theatre_address: e.target.value })} />
      <div className="grid gap-4 sm:grid-cols-2">
        <Input
          label="SIRET"
          value={settings.theatre_siret}
          onChange={(e) => onSiretChange(e.target.value)}
          error={siretFb.ok ? undefined : siretFb.message}
          hint={
            siretFb.ok && sirenDigits.length === 9
              ? `SIREN : ${sirenDigits.slice(0, 3)} ${sirenDigits.slice(3, 6)} ${sirenDigits.slice(6, 9)}`
              : "14 chiffres — formatage automatique"
          }
        />
        <Input
          label="N° TVA intracommunautaire"
          value={settings.theatre_vat_number}
          onChange={(e) => setSettings({ ...settings, theatre_vat_number: e.target.value.toUpperCase() })}
          hint="Calculé automatiquement depuis le SIRET"
        />
      </div>
      <div>
        <p className="mb-2 text-sm font-medium text-[var(--color-text)]">Logo théâtre</p>
        <TheatreLogo className="max-w-[280px]" />
        <p className="mt-2 text-xs text-[var(--color-text-muted)]">
          Logo officiel Théâtre La Carreterie – utilisé automatiquement sur les PDF.
        </p>
      </div>
    </div>
  );
}
