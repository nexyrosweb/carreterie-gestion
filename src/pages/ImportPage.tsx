import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import {
  apiImportAnalyzePdfs,
  apiImportList,
  apiImportMatchContacts,
  apiImportPdfData,
  apiImportReject,
  apiImportValidate,
  ApiError,
} from "@/lib/api";
import { parseMoney } from "@/lib/money";
import type {
  ContactMatchCandidate,
  ImportDirection,
  ImportDocumentType,
  ImportedDocument,
  ImportValidateInput,
} from "@/types";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { Input } from "@/components/ui/Input";
import { Select } from "@/components/ui/Select";
import { Spinner } from "@/components/ui/Spinner";
import { Textarea } from "@/components/ui/Textarea";
import { TBody, THead, Table, Td, Th, Tr } from "@/components/ui/Table";

function statusBadge(status: string): { tone: "default" | "gold" | "red" | "green" | "muted"; label: string } {
  switch (status) {
    case "en_attente":
      return { tone: "gold", label: "En attente" };
    case "valide":
      return { tone: "green", label: "Validé" };
    case "rejete":
      return { tone: "red", label: "Rejeté" };
    case "doublon":
      return { tone: "muted", label: "Doublon" };
    default:
      return { tone: "default", label: status };
  }
}

function docFromForm(doc: ImportedDocument): ImportValidateInput {
  return {
    id: doc.id,
    direction: doc.direction,
    document_type: doc.document_type,
    issuer_name: doc.issuer_name,
    party_name: doc.party_name,
    party_address: doc.party_address,
    siren: doc.siren,
    siret: doc.siret,
    invoice_number: doc.invoice_number,
    issue_date: doc.issue_date,
    payment_date: doc.payment_date,
    payment_method: doc.payment_method,
    payment_status: doc.payment_status,
    description: doc.description,
    total_ht: doc.total_ht,
    tax_rate: doc.tax_rate,
    total_tva: doc.total_tva,
    total_ttc: doc.total_ttc,
    related_invoice_ref: doc.related_invoice_ref,
    contact_id: doc.contact_id,
    create_new_contact: null,
    create_linked_record: false,
    notes: doc.notes,
  };
}

function checkAmounts(ht?: string | null, tva?: string | null, ttc?: string | null): boolean | null {
  if (!ht || !ttc) return null;
  const a = parseMoney(ht);
  const b = tva ? parseMoney(tva) : 0;
  const c = parseMoney(ttc);
  if (!Number.isFinite(a) || !Number.isFinite(c)) return null;
  return Math.abs(a + b - c) <= 0.02;
}

export function ImportPage() {
  const { sessionToken } = useAuth();
  const { showToast } = useToast();
  const [docs, setDocs] = useState<ImportedDocument[]>([]);
  const [loading, setLoading] = useState(true);
  const [importing, setImporting] = useState(false);
  const [progress, setProgress] = useState(0);
  const [filterStatus, setFilterStatus] = useState("");
  const [filterDirection, setFilterDirection] = useState("");
  const [filterType, setFilterType] = useState("");
  const [defaultDirection, setDefaultDirection] = useState<ImportDirection>("client");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [form, setForm] = useState<ImportValidateInput | null>(null);
  const [matches, setMatches] = useState<ContactMatchCandidate[]>([]);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [pdfUrl, setPdfUrl] = useState<string | null>(null);
  const [createContact, setCreateContact] = useState(false);
  const [newContactName, setNewContactName] = useState("");
  const [createLinked, setCreateLinked] = useState(false);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    if (!sessionToken) return;
    setLoading(true);
    try {
      const list = await apiImportList(sessionToken, {
        status: filterStatus || undefined,
        direction: filterDirection || undefined,
        documentType: filterType || undefined,
      });
      setDocs(list);
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Chargement impossible.", "error");
    } finally {
      setLoading(false);
    }
  }, [sessionToken, filterStatus, filterDirection, filterType, showToast]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    return () => {
      if (pdfUrl) URL.revokeObjectURL(pdfUrl);
    };
  }, [pdfUrl]);

  const selected = useMemo(
    () => docs.find((d) => d.id === selectedId) ?? null,
    [docs, selectedId],
  );

  async function openReview(doc: ImportedDocument, extraMatches?: ContactMatchCandidate[], extraWarnings?: string[]) {
    if (!sessionToken) return;
    setSelectedId(doc.id);
    setForm(docFromForm(doc));
    setCreateContact(false);
    setCreateLinked(false);
    setNewContactName(doc.party_name ?? "");
    setWarnings(extraWarnings ?? []);
    if (extraMatches) {
      setMatches(extraMatches);
    } else {
      try {
        setMatches(
          await apiImportMatchContacts(sessionToken, {
            siret: doc.siret ?? undefined,
            siren: doc.siren ?? undefined,
            name: doc.party_name ?? undefined,
          }),
        );
      } catch {
        setMatches([]);
      }
    }
    try {
      const b64 = await apiImportPdfData(sessionToken, doc.id);
      const binary = atob(b64);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
      const blob = new Blob([bytes], { type: "application/pdf" });
      if (pdfUrl) URL.revokeObjectURL(pdfUrl);
      setPdfUrl(URL.createObjectURL(blob));
    } catch (err) {
      setPdfUrl(null);
      showToast(err instanceof ApiError ? err.message : "Aperçu PDF impossible.", "error");
    }
  }

  async function pickAndAnalyze() {
    if (!sessionToken) return;
    const selectedPaths = await open({
      multiple: true,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      title: "Importer des factures PDF",
    });
    if (!selectedPaths) return;
    const paths = Array.isArray(selectedPaths) ? selectedPaths : [selectedPaths];
    if (paths.length === 0) return;

    setImporting(true);
    setProgress(10);
    try {
      const results = await apiImportAnalyzePdfs(sessionToken, paths, defaultDirection);
      setProgress(100);
      const warnCount = results.reduce((n, r) => n + r.warnings.length, 0);
      showToast(
        `${results.length} document(s) analysé(s)${warnCount ? ` – ${warnCount} avertissement(s)` : ""}.`,
        "success",
      );
      await load();
      const firstPending = results.find((r) => r.document.status === "en_attente");
      if (firstPending) {
        await openReview(firstPending.document, firstPending.matches, firstPending.warnings);
      }
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Import impossible.", "error");
    } finally {
      setImporting(false);
      setProgress(0);
    }
  }

  function updateField<K extends keyof ImportValidateInput>(key: K, value: ImportValidateInput[K]) {
    setForm((prev) => (prev ? { ...prev, [key]: value } : prev));
  }

  const coherent = form
    ? checkAmounts(form.total_ht, form.total_tva, form.total_ttc)
    : null;

  async function validate() {
    if (!sessionToken || !form) return;
    if (createContact && !newContactName.trim()) {
      showToast("Indiquez le nom du nouveau contact.", "warning");
      return;
    }
    setSaving(true);
    try {
      const input: ImportValidateInput = {
        ...form,
        create_linked_record: createLinked,
        create_new_contact: createContact
          ? {
              kind: form.direction === "fournisseur" ? "fournisseur" : "client",
              name: newContactName.trim(),
              siret: form.siret ?? undefined,
              address_line1: form.party_address ?? undefined,
            }
          : null,
        contact_id: createContact ? null : form.contact_id,
      };
      const updated = await apiImportValidate(sessionToken, input);
      showToast("Document validé et archivé (sans numérotation officielle).", "success");
      setSelectedId(null);
      setForm(null);
      setPdfUrl(null);
      await load();
      if (updated.linked_draft_invoice_id) {
        showToast("Brouillon client créé (non émis).", "info");
      }
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Validation impossible.", "error");
    } finally {
      setSaving(false);
    }
  }

  async function reject() {
    if (!sessionToken || !selectedId) return;
    setSaving(true);
    try {
      await apiImportReject(sessionToken, selectedId, "Rejeté après vérification");
      showToast("Document rejeté.", "success");
      setSelectedId(null);
      setForm(null);
      setPdfUrl(null);
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Rejet impossible.", "error");
    } finally {
      setSaving(false);
    }
  }

  if (form && selected) {
    return (
      <div className="flex h-[calc(100vh-7rem)] flex-col gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div>
            <Button variant="ghost" size="sm" onClick={() => { setForm(null); setSelectedId(null); setPdfUrl(null); }}>
              ← Retour à la liste
            </Button>
            <p className="mt-1 text-sm text-[var(--color-text-muted)]">
              {selected.original_filename} – extraction : {selected.extraction_source}
            </p>
          </div>
          <div className="flex gap-2">
            <Button variant="danger" disabled={saving} onClick={() => void reject()}>
              Rejeter
            </Button>
            <Button loading={saving} onClick={() => void validate()}>
              Valider l&apos;archivage
            </Button>
          </div>
        </div>

        {(warnings.length > 0 || selected.uncertain_fields.length > 0 || selected.missing_fields.length > 0) && (
          <div className="rounded-md border border-[var(--color-or)]/40 bg-[var(--color-surface-elevated)] px-3 py-2 text-xs text-[var(--color-text)]">
            {warnings.map((w) => (
              <p key={w}>⚠ {w}</p>
            ))}
            {selected.uncertain_fields.length > 0 ? (
              <p>Champs incertains : {selected.uncertain_fields.join(", ")}</p>
            ) : null}
            {selected.missing_fields.length > 0 ? (
              <p>Champs manquants : {selected.missing_fields.join(", ")}</p>
            ) : null}
            {coherent === false ? (
              <p className="text-[var(--color-danger)]">Incohérence HT + TVA ≠ TTC – corrigez avant validation.</p>
            ) : null}
            {coherent === true ? <p className="text-[var(--color-success)]">Montants HT / TVA / TTC cohérents.</p> : null}
          </div>
        )}

        <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-2">
          <Card className="flex min-h-[420px] flex-col !p-0 overflow-hidden">
            <div className="border-b border-[var(--color-border)] px-3 py-2 text-xs font-medium text-[var(--color-text-muted)]">
              Aperçu PDF original
            </div>
            {pdfUrl ? (
              <iframe title="Aperçu PDF" src={pdfUrl} className="h-full min-h-[400px] w-full flex-1 bg-[var(--color-surface)]" />
            ) : (
              <div className="flex flex-1 items-center justify-center p-6">
                <EmptyState title="Aperçu indisponible" description="Le PDF n'a pas pu être chargé." />
              </div>
            )}
          </Card>

          <Card className="overflow-y-auto">
            <div className="space-y-3">
              <div className="grid gap-3 sm:grid-cols-2">
                <Select
                  label="Direction"
                  value={form.direction}
                  onChange={(e) => updateField("direction", e.target.value as ImportDirection)}
                  options={[
                    { value: "client", label: "Facture client (émise)" },
                    { value: "fournisseur", label: "Facture fournisseur (reçue)" },
                  ]}
                />
                <Select
                  label="Type de document"
                  value={form.document_type}
                  onChange={(e) => updateField("document_type", e.target.value as ImportDocumentType)}
                  options={[
                    { value: "facture", label: "Facture" },
                    { value: "acompte", label: "Acompte" },
                    { value: "avoir", label: "Avoir" },
                    { value: "inconnu", label: "Inconnu" },
                  ]}
                />
              </div>
              <Input label="Émetteur" value={form.issuer_name ?? ""} onChange={(e) => updateField("issuer_name", e.target.value || null)} />
              <Input label="Client / fournisseur" value={form.party_name ?? ""} onChange={(e) => updateField("party_name", e.target.value || null)} />
              <Textarea label="Adresse" value={form.party_address ?? ""} onChange={(e) => updateField("party_address", e.target.value || null)} rows={2} />
              <div className="grid gap-3 sm:grid-cols-2">
                <Input label="SIREN" value={form.siren ?? ""} onChange={(e) => updateField("siren", e.target.value || null)} />
                <Input label="SIRET" value={form.siret ?? ""} onChange={(e) => updateField("siret", e.target.value || null)} />
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <Input label="N° facture" value={form.invoice_number ?? ""} onChange={(e) => updateField("invoice_number", e.target.value || null)} />
                <Input label="Date d'émission" type="date" value={form.issue_date ?? ""} onChange={(e) => updateField("issue_date", e.target.value || null)} />
              </div>
              <Textarea label="Description / prestations" value={form.description ?? ""} onChange={(e) => updateField("description", e.target.value || null)} rows={3} />
              <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                <Input label="Total HT" value={form.total_ht ?? ""} onChange={(e) => updateField("total_ht", e.target.value || null)} />
                <Input label="Taux TVA %" value={form.tax_rate ?? ""} onChange={(e) => updateField("tax_rate", e.target.value || null)} />
                <Input label="TVA" value={form.total_tva ?? ""} onChange={(e) => updateField("total_tva", e.target.value || null)} />
                <Input label="Total TTC" value={form.total_ttc ?? ""} onChange={(e) => updateField("total_ttc", e.target.value || null)} />
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <Input label="Date règlement" type="date" value={form.payment_date ?? ""} onChange={(e) => updateField("payment_date", e.target.value || null)} />
                <Input label="Mode règlement" value={form.payment_method ?? ""} onChange={(e) => updateField("payment_method", e.target.value || null)} />
              </div>
              <Input
                label="Réf. facture d'origine (acompte / avoir)"
                value={form.related_invoice_ref ?? ""}
                onChange={(e) => updateField("related_invoice_ref", e.target.value || null)}
              />

              <div className="rounded-md border border-[var(--color-border)] p-3">
                <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-[var(--color-or)]">
                  Contact
                </p>
                {matches.length > 0 && !createContact ? (
                  <Select
                    label="Correspondance proposée"
                    value={form.contact_id ?? ""}
                    onChange={(e) => updateField("contact_id", e.target.value || null)}
                    options={[
                      { value: "", label: "— Aucune —" },
                      ...matches.map((m) => ({
                        value: m.id,
                        label: `${m.name}${m.siret ? ` (${m.siret})` : ""} – ${m.match_reason}`,
                      })),
                    ]}
                  />
                ) : null}
                <label className="mt-2 flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={createContact}
                    onChange={(e) => {
                      setCreateContact(e.target.checked);
                      if (e.target.checked) updateField("contact_id", null);
                    }}
                  />
                  Créer un nouveau contact (pas de fusion auto)
                </label>
                {createContact ? (
                  <Input
                    className="mt-2"
                    label="Nom du nouveau contact"
                    value={newContactName}
                    onChange={(e) => setNewContactName(e.target.value)}
                  />
                ) : null}
              </div>

              <label className="flex items-start gap-2 text-sm">
                <input
                  type="checkbox"
                  className="mt-1"
                  checked={createLinked}
                  onChange={(e) => setCreateLinked(e.target.checked)}
                />
                <span>
                  Créer aussi un enregistrement lié (brouillon client non émis, ou facture fournisseur).
                  <span className="block text-xs text-[var(--color-text-muted)]">
                    Aucune facture officielle numérotée n&apos;est créée automatiquement.
                  </span>
                </span>
              </label>

              <Textarea
                label="Notes"
                value={form.notes ?? ""}
                onChange={(e) => updateField("notes", e.target.value || null)}
                rows={2}
              />
            </div>
          </Card>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <Card>
        <div className="flex flex-wrap items-end gap-3">
          <Select
            label="Direction par défaut à l'import"
            value={defaultDirection}
            onChange={(e) => setDefaultDirection(e.target.value as ImportDirection)}
            options={[
              { value: "client", label: "Clients (factures émises)" },
              { value: "fournisseur", label: "Fournisseurs (reçues)" },
            ]}
          />
          <Button onClick={() => void pickAndAnalyze()} loading={importing}>
            Importer des PDF…
          </Button>
          <p className="text-xs text-[var(--color-text-muted)]">
            Hors ligne – max 25 Mo – doublons détectés par empreinte SHA-256. OCR local via Tesseract si installé.
          </p>
        </div>
        {importing ? (
          <div className="mt-3">
            <div className="h-2 overflow-hidden rounded bg-[var(--color-surface-elevated)]">
              <div
                className="h-full bg-[var(--color-rouge)] transition-all"
                style={{ width: `${Math.max(progress, 15)}%` }}
              />
            </div>
            <p className="mt-1 text-xs text-[var(--color-text-muted)]">Analyse en cours…</p>
          </div>
        ) : null}
      </Card>

      <div className="flex flex-wrap gap-3">
        <Select
          label="Statut"
          value={filterStatus}
          onChange={(e) => setFilterStatus(e.target.value)}
          options={[
            { value: "", label: "Tous" },
            { value: "en_attente", label: "En attente" },
            { value: "valide", label: "Validés" },
            { value: "doublon", label: "Doublons" },
            { value: "rejete", label: "Rejetés" },
          ]}
        />
        <Select
          label="Direction"
          value={filterDirection}
          onChange={(e) => setFilterDirection(e.target.value)}
          options={[
            { value: "", label: "Toutes" },
            { value: "client", label: "Client" },
            { value: "fournisseur", label: "Fournisseur" },
          ]}
        />
        <Select
          label="Type"
          value={filterType}
          onChange={(e) => setFilterType(e.target.value)}
          options={[
            { value: "", label: "Tous" },
            { value: "facture", label: "Facture" },
            { value: "acompte", label: "Acompte" },
            { value: "avoir", label: "Avoir" },
          ]}
        />
      </div>

      {loading ? (
        <div className="flex justify-center py-16">
          <Spinner className="h-8 w-8 text-[var(--color-or)]" />
        </div>
      ) : docs.length === 0 ? (
        <EmptyState
          title="Aucun document importé"
          description="Sélectionnez un ou plusieurs PDF pour démarrer l'extraction."
        />
      ) : (
        <Card className="!p-0 overflow-hidden">
          <Table>
            <THead>
              <Tr>
                <Th>Fichier</Th>
                <Th>N°</Th>
                <Th>Type</Th>
                <Th>Direction</Th>
                <Th>Statut</Th>
                <Th>TTC</Th>
                <Th />
              </Tr>
            </THead>
            <TBody>
              {docs.map((doc) => {
                const badge = statusBadge(doc.status);
                return (
                  <Tr key={doc.id}>
                    <Td className="max-w-[14rem] truncate">{doc.original_filename}</Td>
                    <Td>{doc.invoice_number ?? "—"}</Td>
                    <Td>{doc.document_type}</Td>
                    <Td>{doc.direction}</Td>
                    <Td>
                      <Badge tone={badge.tone}>{badge.label}</Badge>
                    </Td>
                    <Td>{doc.total_ttc ?? "—"}</Td>
                    <Td>
                      <Button size="sm" variant="secondary" onClick={() => void openReview(doc)}>
                        {doc.status === "en_attente" ? "Vérifier" : "Ouvrir"}
                      </Button>
                    </Td>
                  </Tr>
                );
              })}
            </TBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
