import { useCallback, useEffect, useMemo, useState } from "react";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import {
  apiContactsList,
  apiContractsGenerateFromDevis,
  apiInvoicesAcceptDevis,
  apiInvoicesCreateCreditNote,
  apiInvoicesCreateDraft,
  apiInvoicesCreateFromDevis,
  apiInvoicesDeleteDraft,
  apiInvoicesGeneratePdf,
  apiInvoicesGet,
  apiInvoicesIssue,
  apiInvoicesList,
  apiInvoicesListPayments,
  apiInvoicesPreviewPdf,
  apiInvoicesRecordPayment,
  apiInvoicesUpdateDraft,
  apiMailSendDocument,
  ApiError,
} from "@/lib/api";
import {
  formatEur,
  formatMoneyDecimal,
  lineAmounts,
  parseMoney,
  sumLines,
} from "@/lib/money";
import type {
  Contact,
  Invoice,
  InvoiceDraftInput,
  InvoiceKind,
  InvoiceLine,
  InvoiceLineInput,
  InvoicePayment,
  PaymentType,
} from "@/types";
import { Badge, invoiceStatusBadge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { EmptyState } from "@/components/ui/EmptyState";
import { Input } from "@/components/ui/Input";
import { Modal } from "@/components/ui/Modal";
import { Select } from "@/components/ui/Select";
import { Spinner } from "@/components/ui/Spinner";
import { Textarea } from "@/components/ui/Textarea";
import { TBody, THead, Table, Td, Th, Tr } from "@/components/ui/Table";
import { format } from "date-fns";
import { fr } from "date-fns/locale";

const defaultLine = (): InvoiceLineInput => ({
  description: "",
  quantity: 1,
  unit_price_ht: 0,
  tax_rate_percent: 20,
  line_kind: "standard",
});

const locationLine = (): InvoiceLineInput => ({
  description: "Location de salle",
  quantity: 1,
  unit_price_ht: 0,
  tax_rate_percent: 20,
  line_kind: "location_salle",
});

function lineToInput(line: InvoiceLine): InvoiceLineInput {
  return {
    description: line.description,
    quantity: parseMoney(line.quantity),
    unit_price_ht: parseMoney(line.unit_price_ht),
    tax_rate_percent: parseMoney(line.tax_rate_percent),
    line_kind: line.line_kind ?? "standard",
  };
}

function isClientContact(c: Contact): boolean {
  return c.kind === "client" || c.kind === "les_deux";
}

export function ClientInvoicesPage({
  documentKind = "facture",
}: {
  documentKind?: Extract<InvoiceKind, "facture" | "devis">;
}) {
  const isDevis = documentKind === "devis";
  const docLabel = isDevis ? "devis" : "facture";
  const { sessionToken } = useAuth();
  const { showToast } = useToast();
  const [invoices, setInvoices] = useState<Invoice[]>([]);
  const [clients, setClients] = useState<Contact[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("");
  const [editorOpen, setEditorOpen] = useState(false);
  const [detail, setDetail] = useState<Invoice | null>(null);
  const [draftForm, setDraftForm] = useState<InvoiceDraftInput>({
    contact_id: "",
    lines: [defaultLine()],
    due_date: "",
    customer_notes: "",
    invoice_kind: documentKind,
    rental_start: "",
    rental_end: "",
  });
  const [editingId, setEditingId] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const [paymentAmount, setPaymentAmount] = useState("");
  const [paymentDate, setPaymentDate] = useState(
    () => new Date().toISOString().slice(0, 10),
  );
  const [paymentType, setPaymentType] = useState<PaymentType>("reglement");
  const [payments, setPayments] = useState<InvoicePayment[]>([]);
  const [emailTo, setEmailTo] = useState("");
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  const clientNames = useMemo(() => {
    const map = new Map<string, string>();
    for (const c of clients) map.set(c.id, c.name);
    return map;
  }, [clients]);

  const load = useCallback(async () => {
    if (!sessionToken) return;
    setLoading(true);
    try {
      const [inv, allContacts] = await Promise.all([
        apiInvoicesList(sessionToken, filter || undefined, documentKind),
        apiContactsList(sessionToken),
      ]);
      setInvoices(inv);
      setClients(allContacts.filter(isClientContact));
    } catch (err) {
      showToast(
        err instanceof ApiError ? err.message : `Erreur chargement ${docLabel}s.`,
        "error",
      );
      setInvoices([]);
    } finally {
      setLoading(false);
    }
  }, [sessionToken, filter, showToast, documentKind, docLabel]);

  useEffect(() => {
    void load();
  }, [load]);

  const totals = useMemo(
    () =>
      sumLines(
        draftForm.lines.map((l) => ({
          quantity: parseMoney(l.quantity),
          unit_price_ht: parseMoney(l.unit_price_ht),
          tax_rate_percent: parseMoney(l.tax_rate_percent),
        })),
      ),
    [draftForm.lines],
  );

  function openNewDraft() {
    setEditingId(null);
    setDraftForm({
      contact_id: clients[0]?.id ?? "",
      lines: [defaultLine()],
      issue_date: "",
      due_date: "",
      customer_notes: "",
      invoice_kind: documentKind,
      rental_start: "",
      rental_end: "",
    });
    setEditorOpen(true);
  }

  function addLocationLine() {
    setDraftForm((prev) => ({
      ...prev,
      lines: [...prev.lines, locationLine()],
    }));
  }

  async function openDetail(inv: Invoice) {
    if (!sessionToken) return;
    try {
      const full = await apiInvoicesGet(sessionToken, inv.id);
      setDetail(full);
      setEmailTo("");
      setPreviewUrl((prev) => {
        if (prev) URL.revokeObjectURL(prev);
        return null;
      });
      if (!isDevis && full.status === "emise") {
        try {
          setPayments(await apiInvoicesListPayments(sessionToken, full.id));
        } catch {
          setPayments([]);
        }
      } else {
        setPayments([]);
      }
    } catch (err) {
      showToast(
        err instanceof ApiError ? err.message : `Impossible d'ouvrir le ${docLabel}.`,
        "error",
      );
    }
  }

  function openEditDraft(inv: Invoice) {
    setEditingId(inv.id);
    setDraftForm({
      contact_id: inv.contact_id,
      issue_date: inv.issue_date ?? "",
      due_date: inv.due_date ?? "",
      customer_notes: inv.customer_notes ?? "",
      lines: inv.lines.length ? inv.lines.map(lineToInput) : [defaultLine()],
      invoice_kind: documentKind,
      rental_start: inv.rental_start ?? "",
      rental_end: inv.rental_end ?? "",
    });
    setEditorOpen(true);
  }

  function updateLine(index: number, patch: Partial<InvoiceLineInput>) {
    setDraftForm((prev) => {
      const lines = [...prev.lines];
      const current = lines[index];
      if (!current) return prev;
      lines[index] = { ...current, ...patch };
      return { ...prev, lines };
    });
  }

  function addLine() {
    setDraftForm((prev) => ({ ...prev, lines: [...prev.lines, defaultLine()] }));
  }

  function removeLine(index: number) {
    setDraftForm((prev) => ({
      ...prev,
      lines: prev.lines.length > 1 ? prev.lines.filter((_, i) => i !== index) : prev.lines,
    }));
  }

  async function saveDraft() {
    if (!sessionToken) return;
    if (!draftForm.contact_id) {
      showToast("Sélectionnez un client.", "warning");
      return;
    }
    if (draftForm.lines.some((l) => !l.description.trim())) {
      showToast("Chaque ligne doit avoir une description.", "warning");
      return;
    }
    setSaving(true);
    try {
      const input: InvoiceDraftInput = {
        ...draftForm,
        issue_date: draftForm.issue_date || undefined,
        due_date: draftForm.due_date || undefined,
        customer_notes: draftForm.customer_notes || undefined,
        invoice_kind: documentKind,
        rental_start: draftForm.rental_start || undefined,
        rental_end: draftForm.rental_end || undefined,
      };
      if (editingId) {
        await apiInvoicesUpdateDraft(sessionToken, editingId, input);
        showToast("Brouillon mis à jour.", "success");
      } else {
        await apiInvoicesCreateDraft(sessionToken, input);
        showToast("Brouillon créé.", "success");
      }
      setEditorOpen(false);
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Enregistrement impossible.", "error");
    } finally {
      setSaving(false);
    }
  }

  async function recordPayment() {
    if (!sessionToken || !detail || detail.status !== "emise") return;
    const amount = Number(paymentAmount.replace(",", "."));
    if (!Number.isFinite(amount) || amount <= 0) {
      showToast("Indiquez un montant de règlement valide.", "warning");
      return;
    }
    try {
      const updated = await apiInvoicesRecordPayment(sessionToken, {
        invoice_id: detail.id,
        amount,
        paid_at: paymentDate,
        method: "virement",
        payment_type: paymentType,
      });
      setDetail(updated);
      setPayments(await apiInvoicesListPayments(sessionToken, detail.id));
      setPaymentAmount("");
      showToast(
        paymentType === "acompte"
          ? "Acompte enregistré."
          : paymentType === "solde"
            ? "Solde enregistré."
            : "Règlement enregistré.",
        "success",
      );
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Règlement impossible.", "error");
    }
  }

  async function convertDevis(issueImmediately: boolean) {
    if (!sessionToken || !detail) return;
    try {
      const facture = await apiInvoicesCreateFromDevis(sessionToken, {
        devis_id: detail.id,
        issue_immediately: issueImmediately,
      });
      showToast(
        issueImmediately
          ? `Facture ${facture.number ?? ""} émise depuis le devis.`
          : "Brouillon facture créé depuis le devis.",
        "success",
      );
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Conversion impossible.", "error");
    }
  }

  async function acceptDevis() {
    if (!sessionToken || !detail) return;
    try {
      const updated = await apiInvoicesAcceptDevis(sessionToken, detail.id);
      setDetail(updated);
      showToast("Devis marqué comme accepté.", "success");
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Acceptation impossible.", "error");
    }
  }

  async function generateContract() {
    if (!sessionToken || !detail) return;
    try {
      const path = await apiContractsGenerateFromDevis(sessionToken, detail.id);
      showToast(`Contrat de cession : ${path}`, "success");
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Génération contrat impossible.", "error");
    }
  }

  async function sendEmail() {
    if (!sessionToken || !detail) return;
    try {
      await apiMailSendDocument(sessionToken, {
        invoice_id: detail.id,
        to_email: emailTo.trim() || undefined,
      });
      showToast("Document envoyé par e-mail.", "success");
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Envoi e-mail impossible.", "error");
    }
  }

  async function issueInvoice(id: string) {
    if (!sessionToken) return;
    try {
      await apiInvoicesIssue(sessionToken, id);
      showToast(isDevis ? "Devis émis." : "Facture émise.", "success");
      await load();
      if (detail?.id === id) {
        setDetail(await apiInvoicesGet(sessionToken, id));
      }
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Émission impossible.", "error");
    }
  }

  async function generatePdf(id: string) {
    if (!sessionToken) return;
    try {
      const path = await apiInvoicesGeneratePdf(sessionToken, id);
      showToast(`PDF généré : ${path}`, "success");
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Génération PDF impossible.", "error");
    }
  }

  async function showPreview(id: string) {
    if (!sessionToken) return;
    setPreviewLoading(true);
    try {
      const b64 = await apiInvoicesPreviewPdf(sessionToken, id);
      const binary = atob(b64);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
      const blob = new Blob([bytes], { type: "application/pdf" });
      setPreviewUrl((prev) => {
        if (prev) URL.revokeObjectURL(prev);
        return URL.createObjectURL(blob);
      });
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Aperçu PDF impossible.", "error");
    } finally {
      setPreviewLoading(false);
    }
  }

  async function createCreditNote(inv: Invoice) {
    if (!sessionToken) return;
    const lines = inv.lines.map(lineToInput);
    if (lines.length === 0) {
      showToast("La facture n'a pas de lignes pour créer un avoir.", "warning");
      return;
    }
    try {
      await apiInvoicesCreateCreditNote(sessionToken, inv.id, lines, inv.customer_notes ?? undefined);
      showToast("Avoir créé (brouillon).", "success");
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Création avoir impossible.", "error");
    }
  }

  async function deleteDraft(id: string) {
    if (!sessionToken) return;
    try {
      await apiInvoicesDeleteDraft(sessionToken, id);
      showToast("Brouillon supprimé.", "success");
      setDeleteTarget(null);
      if (detail?.id === id) setDetail(null);
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Suppression impossible.", "error");
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-end gap-3">
        <Select
          label="Filtrer par statut"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          options={
            isDevis
              ? [
                  { value: "", label: "Tous" },
                  { value: "brouillon", label: "Brouillons" },
                  { value: "emise", label: "Émis" },
                ]
              : [
                  { value: "", label: "Toutes" },
                  { value: "brouillon", label: "Brouillons" },
                  { value: "emise", label: "Émises" },
                  { value: "avoir", label: "Avoirs" },
                ]
          }
          className="min-w-[12rem]"
        />
        <div className="flex-1" />
        <Button onClick={openNewDraft} disabled={clients.length === 0}>
          {isDevis ? "Nouveau devis" : "Nouveau brouillon"}
        </Button>
      </div>

      <Card>
        {loading ? (
          <Spinner className="mx-auto py-12 text-[var(--color-or)]" />
        ) : invoices.length === 0 ? (
          <EmptyState
            title={isDevis ? "Aucun devis" : "Aucune facture"}
            description={
              isDevis
                ? "Créez un devis pour commencer."
                : "Créez un brouillon pour commencer."
            }
          />
        ) : (
          <Table>
            <THead>
              <tr>
                <Th>N°</Th>
                <Th>Client</Th>
                <Th>Statut</Th>
                <Th>Date</Th>
                <Th className="text-right">TTC</Th>
                <Th className="text-right">Actions</Th>
              </tr>
            </THead>
            <TBody>
              {invoices.map((inv) => {
                const st = invoiceStatusBadge(inv.status);
                return (
                  <Tr key={inv.id} onClick={() => void openDetail(inv)}>
                    <Td>{inv.number ?? `Brouillon`}</Td>
                    <Td>{clientNames.get(inv.contact_id) ?? inv.contact_id.slice(0, 8)}</Td>
                    <Td>
                      <Badge tone={st.tone}>{st.label}</Badge>
                    </Td>
                    <Td>
                      {inv.issue_date
                        ? format(new Date(inv.issue_date), "dd/MM/yyyy", { locale: fr })
                        : "—"}
                    </Td>
                    <Td className="text-right">{formatMoneyDecimal(inv.total_ttc)}</Td>
                    <Td className="text-right">
                      {inv.status === "brouillon" ? (
                        <Button
                          size="sm"
                          variant="secondary"
                          onClick={(e) => {
                            e.stopPropagation();
                            openEditDraft(inv);
                          }}
                        >
                          Éditer
                        </Button>
                      ) : null}
                    </Td>
                  </Tr>
                );
              })}
            </TBody>
          </Table>
        )}
      </Card>

      <Modal
        open={editorOpen}
        onClose={() => setEditorOpen(false)}
        title={
          editingId
            ? isDevis
              ? "Modifier le devis"
              : "Modifier le brouillon"
            : isDevis
              ? "Nouveau devis"
              : "Nouveau brouillon"
        }
        size="xl"
        footer={
          <>
            <Button variant="secondary" onClick={() => setEditorOpen(false)}>
              Annuler
            </Button>
            <Button onClick={() => void saveDraft()} loading={saving}>
              Enregistrer
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <Select
            label="Client"
            value={draftForm.contact_id}
            onChange={(e) => setDraftForm({ ...draftForm, contact_id: e.target.value })}
            options={clients.map((c) => ({ value: c.id, label: c.name }))}
          />
          <div className="grid gap-3 sm:grid-cols-2">
            <Input
              label="Date d'émission"
              type="date"
              value={draftForm.issue_date ?? ""}
              onChange={(e) => setDraftForm({ ...draftForm, issue_date: e.target.value })}
            />
            <Input
              label={isDevis ? "Validité" : "Échéance"}
              type="date"
              value={draftForm.due_date ?? ""}
              onChange={(e) => setDraftForm({ ...draftForm, due_date: e.target.value })}
            />
          </div>
          <div>
            <Textarea
              label={isDevis ? "Prestation / notes" : "Notes client / prestation"}
              value={draftForm.customer_notes ?? ""}
              onChange={(e) => setDraftForm({ ...draftForm, customer_notes: e.target.value })}
            />
            <p className="mt-1 text-[11px] text-[var(--color-text-muted)]">
              Affiché sur le PDF (ex. Location salle, juillet 2026)
            </p>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <Input
              label="Location — début"
              type="datetime-local"
              value={draftForm.rental_start ?? ""}
              onChange={(e) => setDraftForm({ ...draftForm, rental_start: e.target.value })}
            />
            <Input
              label="Location — fin"
              type="datetime-local"
              value={draftForm.rental_end ?? ""}
              onChange={(e) => setDraftForm({ ...draftForm, rental_end: e.target.value })}
            />
          </div>

          <Table>
            <THead>
              <tr>
                <Th>Type</Th>
                <Th>Description</Th>
                <Th>Qté</Th>
                <Th>PU HT</Th>
                <Th>TVA %</Th>
                <Th className="text-right">TTC ligne</Th>
                <Th />
              </tr>
            </THead>
            <TBody>
              {draftForm.lines.map((line, index) => {
                const amounts = lineAmounts(
                  parseMoney(line.quantity),
                  parseMoney(line.unit_price_ht),
                  parseMoney(line.tax_rate_percent),
                );
                return (
                  <Tr key={index}>
                    <Td>
                      <select
                        className="rounded border border-[var(--color-border)] bg-transparent px-1 py-1 text-xs"
                        value={line.line_kind ?? "standard"}
                        onChange={(e) =>
                          updateLine(index, {
                            line_kind: e.target.value as "standard" | "location_salle",
                            description:
                              e.target.value === "location_salle" && !line.description
                                ? "Location de salle"
                                : line.description,
                          })
                        }
                      >
                        <option value="standard">Standard</option>
                        <option value="location_salle">Location salle</option>
                      </select>
                    </Td>
                    <Td>
                      <input
                        className="w-full rounded border border-[var(--color-border)] bg-transparent px-2 py-1 text-sm"
                        value={line.description}
                        onChange={(e) => updateLine(index, { description: e.target.value })}
                      />
                    </Td>
                    <Td>
                      <input
                        type="number"
                        min={0}
                        step="0.01"
                        className="w-16 rounded border border-[var(--color-border)] bg-transparent px-2 py-1 text-sm"
                        value={line.quantity}
                        onChange={(e) => updateLine(index, { quantity: Number(e.target.value) })}
                      />
                    </Td>
                    <Td>
                      <input
                        type="number"
                        min={0}
                        step="0.01"
                        className="w-24 rounded border border-[var(--color-border)] bg-transparent px-2 py-1 text-sm"
                        value={line.unit_price_ht}
                        onChange={(e) => updateLine(index, { unit_price_ht: Number(e.target.value) })}
                      />
                    </Td>
                    <Td>
                      <input
                        type="number"
                        min={0}
                        className="w-16 rounded border border-[var(--color-border)] bg-transparent px-2 py-1 text-sm"
                        value={line.tax_rate_percent}
                        onChange={(e) => updateLine(index, { tax_rate_percent: Number(e.target.value) })}
                      />
                    </Td>
                    <Td className="text-right">{formatEur(amounts.ttc)}</Td>
                    <Td>
                      <Button size="sm" variant="ghost" onClick={() => removeLine(index)}>
                        Retirer
                      </Button>
                    </Td>
                  </Tr>
                );
              })}
            </TBody>
          </Table>
          <div className="flex flex-wrap gap-2">
            <Button variant="secondary" size="sm" onClick={addLine}>
              Ajouter une ligne
            </Button>
            <Button variant="ghost" size="sm" onClick={addLocationLine}>
              Ligne location de salle
            </Button>
          </div>
          <p className="text-right text-sm text-[var(--color-text-muted)]">
            Total HT {formatEur(totals.totalHt)} – TVA {formatEur(totals.totalTva)} – TTC{" "}
            <span className="font-semibold text-[var(--color-or)]">{formatEur(totals.totalTtc)}</span>
          </p>
        </div>
      </Modal>

      <Modal
        open={detail != null}
        onClose={() => {
          setDetail(null);
          setPreviewUrl((prev) => {
            if (prev) URL.revokeObjectURL(prev);
            return null;
          });
        }}
        title={
          detail?.number
            ? `${isDevis ? "Devis" : detail.invoice_kind === "avoir" ? "Avoir" : "Facture"} ${detail.number}`
            : isDevis
              ? "Devis"
              : "Facture"
        }
        size="xl"
        footer={
          detail ? (
            <>
              <Button
                variant="secondary"
                loading={previewLoading}
                onClick={() => void showPreview(detail.id)}
              >
                Aperçu
              </Button>
              {detail.status === "brouillon" ? (
                <>
                  <Button variant="danger" onClick={() => setDeleteTarget(detail.id)}>
                    Supprimer
                  </Button>
                  <Button variant="secondary" onClick={() => openEditDraft(detail)}>
                    Éditer
                  </Button>
                  <Button onClick={() => void issueInvoice(detail.id)}>Émettre</Button>
                </>
              ) : null}
              {detail.status === "emise" || detail.status === "avoir" ? (
                <>
                  <Button variant="gold" onClick={() => void generatePdf(detail.id)}>
                    Enregistrer PDF
                  </Button>
                  <Button variant="secondary" onClick={() => void sendEmail()}>
                    Envoyer e-mail
                  </Button>
                  {!isDevis && detail.status === "emise" && detail.invoice_kind !== "devis" ? (
                    <Button variant="danger" onClick={() => void createCreditNote(detail)}>
                      Avoir
                    </Button>
                  ) : null}
                  {isDevis && detail.status === "emise" ? (
                    <>
                      {!detail.accepted_at ? (
                        <Button variant="secondary" onClick={() => void acceptDevis()}>
                          Marquer accepté
                        </Button>
                      ) : (
                        <Button variant="gold" onClick={() => void generateContract()}>
                          Contrat de cession
                        </Button>
                      )}
                      <Button onClick={() => void convertDevis(false)}>→ Facture (brouillon)</Button>
                      <Button variant="secondary" onClick={() => void convertDevis(true)}>
                        → Facture émise
                      </Button>
                    </>
                  ) : null}
                </>
              ) : null}
            </>
          ) : null
        }
      >
        {detail ? (
          <div className="space-y-3 text-sm">
            <p>
              Client : <strong>{clientNames.get(detail.contact_id) ?? detail.contact_id}</strong>
            </p>
            <p>
              Statut :{" "}
              <Badge tone={invoiceStatusBadge(detail.status).tone}>
                {invoiceStatusBadge(detail.status).label}
              </Badge>
              {detail.accepted_at ? (
                <span className="ml-2 text-xs text-[var(--color-or)]">Accepté</span>
              ) : null}
            </p>
            <p>
              Totaux : HT {formatMoneyDecimal(detail.total_ht)} – TVA{" "}
              {formatMoneyDecimal(detail.total_tva)} – TTC {formatMoneyDecimal(detail.total_ttc)}
            </p>
            {!isDevis && detail.status === "emise" ? (
              <p className="text-xs text-[var(--color-text-muted)]">
                Payé : {formatMoneyDecimal(detail.amount_paid ?? 0)} – Statut paiement :{" "}
                {detail.payment_status ?? "impayee"}
              </p>
            ) : null}
            {detail.customer_notes ? (
              <p className="text-xs text-[var(--color-text-muted)]">{detail.customer_notes}</p>
            ) : null}
            {(detail.rental_start || detail.rental_end) && (
              <p className="text-xs text-[var(--color-text-muted)]">
                Location : {detail.rental_start ?? "—"} → {detail.rental_end ?? "—"}
              </p>
            )}

            {(detail.status === "emise" || detail.status === "avoir") && (
              <Input
                label="E-mail destinataire (optionnel)"
                value={emailTo}
                onChange={(e) => setEmailTo(e.target.value)}
                placeholder="Sinon e-mail du contact"
              />
            )}

            {previewUrl ? (
              <iframe
                title="Aperçu PDF"
                src={previewUrl}
                className="mt-2 h-[28rem] w-full rounded border border-[var(--color-border)] bg-white"
              />
            ) : (
              <p className="text-xs text-[var(--color-text-muted)]">
                Cliquez sur « Aperçu » pour visualiser le document au format du modèle.
              </p>
            )}

            {!isDevis && payments.length > 0 ? (
              <div className="border-t border-[var(--color-border)] pt-3">
                <p className="mb-2 text-xs font-medium">Règlements / acomptes</p>
                <ul className="space-y-1 text-xs text-[var(--color-text-muted)]">
                  {payments.map((p) => (
                    <li key={p.id}>
                      {p.paid_at} — {formatMoneyDecimal(p.amount)} — {p.payment_type}
                      {p.method ? ` (${p.method})` : ""}
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

            {!isDevis && detail.status === "emise" && detail.payment_status !== "payee" ? (
              <div className="mt-3 grid gap-2 border-t border-[var(--color-border)] pt-3 sm:grid-cols-2 lg:grid-cols-4">
                <Input
                  label="Montant"
                  type="number"
                  step="0.01"
                  value={paymentAmount}
                  onChange={(e) => setPaymentAmount(e.target.value)}
                />
                <Input
                  label="Date"
                  type="date"
                  value={paymentDate}
                  onChange={(e) => setPaymentDate(e.target.value)}
                />
                <Select
                  label="Type"
                  value={paymentType}
                  onChange={(e) => setPaymentType(e.target.value as PaymentType)}
                  options={[
                    { value: "reglement", label: "Règlement" },
                    { value: "acompte", label: "Acompte" },
                    { value: "solde", label: "Solde" },
                  ]}
                />
                <div className="flex items-end">
                  <Button className="w-full" variant="secondary" onClick={() => void recordPayment()}>
                    Enregistrer
                  </Button>
                </div>
              </div>
            ) : null}
          </div>
        ) : null}
      </Modal>

      <ConfirmDialog
        open={deleteTarget != null}
        title="Supprimer le brouillon"
        message="Confirmer la suppression de ce brouillon ?"
        destructive
        onConfirm={() => deleteTarget && void deleteDraft(deleteTarget)}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  );
}