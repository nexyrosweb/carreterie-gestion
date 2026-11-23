import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import {
  apiContactsList,
  apiSuppliersCreate,
  apiSuppliersDelete,
  apiSuppliersList,
  apiSuppliersUpdate,
  ApiError,
} from "@/lib/api";
import { formatMoneyDecimal, parseAmountInput, parseMoney } from "@/lib/money";
import type { Contact, SupplierInvoice, SupplierInvoiceInput } from "@/types";
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

function isSupplierContact(c: Contact): boolean {
  return c.kind === "fournisseur" || c.kind === "les_deux";
}

const emptyInput = (): SupplierInvoiceInput => ({
  supplier_contact_id: "",
  reference: "",
  invoice_date: "",
  amount_ht: undefined,
  amount_ttc: undefined,
  tax_rate_percent: 20,
  notes: "",
  attachment_path: "",
});

function backupFileName(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] ?? path;
}

export function SupplierInvoicesPage() {
  const { sessionToken } = useAuth();
  const { showToast } = useToast();
  const [items, setItems] = useState<SupplierInvoice[]>([]);
  const [suppliers, setSuppliers] = useState<Contact[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<SupplierInvoice | null>(null);
  const [form, setForm] = useState<SupplierInvoiceInput>(emptyInput());
  const [saving, setSaving] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<SupplierInvoice | null>(null);

  const supplierNames = useMemo(() => {
    const map = new Map<string, string>();
    for (const s of suppliers) map.set(s.id, s.name);
    return map;
  }, [suppliers]);

  const filteredItems = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return items;
    return items.filter((row) => {
      const name = row.supplier_contact_id
        ? (supplierNames.get(row.supplier_contact_id) ?? "")
        : "";
      return (
        (row.reference ?? "").toLowerCase().includes(q) ||
        name.toLowerCase().includes(q)
      );
    });
  }, [items, search, supplierNames]);

  const load = useCallback(async () => {
    if (!sessionToken) return;
    setLoading(true);
    try {
      const [list, allContacts] = await Promise.all([
        apiSuppliersList(sessionToken),
        apiContactsList(sessionToken),
      ]);
      setItems(list);
      setSuppliers(allContacts.filter(isSupplierContact));
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Erreur chargement.", "error");
      setItems([]);
    } finally {
      setLoading(false);
    }
  }, [sessionToken, showToast]);

  useEffect(() => {
    void load();
  }, [load]);

  function openCreate() {
    setEditing(null);
    setForm({ ...emptyInput(), supplier_contact_id: suppliers[0]?.id ?? "" });
    setModalOpen(true);
  }

  function openEdit(row: SupplierInvoice) {
    setEditing(row);
    setForm({
      supplier_contact_id: row.supplier_contact_id ?? "",
      reference: row.reference ?? "",
      invoice_date: row.invoice_date ?? "",
      amount_ht: row.amount_ht ?? undefined,
      amount_ttc: row.amount_ttc ?? undefined,
      tax_rate_percent: row.tax_rate_percent ?? 20,
      notes: row.notes ?? "",
      attachment_path: row.attachment_path ?? "",
    });
    setModalOpen(true);
  }

  async function pickAttachment() {
    const selected = await open({ multiple: false, title: "Pièce jointe fournisseur" });
    if (!selected || Array.isArray(selected)) return;
    setForm((f) => ({ ...f, attachment_path: selected }));
  }

  async function save() {
    if (!sessionToken || !form.supplier_contact_id) {
      showToast("Sélectionnez un fournisseur.", "warning");
      return;
    }
    setSaving(true);
    try {
      const input: SupplierInvoiceInput = {
        ...form,
        reference: form.reference || undefined,
        invoice_date: form.invoice_date || undefined,
        notes: form.notes || undefined,
        attachment_path: form.attachment_path || undefined,
      };
      if (editing) {
        await apiSuppliersUpdate(sessionToken, editing.id, input);
        showToast("Facture fournisseur mise à jour.", "success");
      } else {
        await apiSuppliersCreate(sessionToken, input);
        showToast("Facture fournisseur enregistrée.", "success");
      }
      setModalOpen(false);
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Enregistrement impossible.", "error");
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete() {
    if (!sessionToken || !deleteTarget) return;
    try {
      await apiSuppliersDelete(sessionToken, deleteTarget.id);
      showToast("Facture supprimée.", "success");
      setDeleteTarget(null);
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Suppression impossible.", "error");
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap gap-3">
        <Input
          label="Recherche"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Référence, fournisseur…"
          className="min-w-[16rem] flex-1"
        />
        <div className="flex items-end">
          <Button onClick={openCreate} disabled={suppliers.length === 0}>
            Nouvelle facture
          </Button>
        </div>
      </div>

      <Card>
        {loading ? (
          <Spinner className="mx-auto py-12 text-[var(--color-or)]" />
        ) : filteredItems.length === 0 ? (
          <EmptyState title="Aucune facture fournisseur" description="Enregistrez une facture reçue." />
        ) : (
          <Table>
            <THead>
              <tr>
                <Th>Réf.</Th>
                <Th>Fournisseur</Th>
                <Th>Date</Th>
                <Th className="text-right">TTC</Th>
                <Th>Pièce jointe</Th>
                <Th className="text-right">Actions</Th>
              </tr>
            </THead>
            <TBody>
              {filteredItems.map((row) => (
                <Tr key={row.id}>
                  <Td>{row.reference || "—"}</Td>
                  <Td>
                    {row.supplier_contact_id
                      ? (supplierNames.get(row.supplier_contact_id) ?? row.supplier_contact_id.slice(0, 8))
                      : "—"}
                  </Td>
                  <Td>
                    {row.invoice_date
                      ? format(new Date(row.invoice_date), "dd/MM/yyyy", { locale: fr })
                      : "—"}
                  </Td>
                  <Td className="text-right">
                    {row.amount_ttc != null ? formatMoneyDecimal(row.amount_ttc) : "—"}
                  </Td>
                  <Td className="text-xs text-[var(--color-text-muted)]">
                    {row.attachment_path ? backupFileName(row.attachment_path) : "—"}
                  </Td>
                  <Td className="text-right">
                    <p className="flex justify-end gap-1">
                      <Button size="sm" variant="secondary" onClick={() => openEdit(row)}>
                        Modifier
                      </Button>
                      <Button size="sm" variant="danger" onClick={() => setDeleteTarget(row)}>
                        Supprimer
                      </Button>
                    </p>
                  </Td>
                </Tr>
              ))}
            </TBody>
          </Table>
        )}
      </Card>

      <Modal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title={editing ? "Modifier la facture" : "Nouvelle facture fournisseur"}
        size="lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setModalOpen(false)}>
              Annuler
            </Button>
            <Button onClick={() => void save()} loading={saving}>
              Enregistrer
            </Button>
          </>
        }
      >
        <div className="grid gap-4 sm:grid-cols-2">
          <Select
            label="Fournisseur"
            value={form.supplier_contact_id ?? ""}
            onChange={(e) => setForm({ ...form, supplier_contact_id: e.target.value })}
            options={suppliers.map((s) => ({ value: s.id, label: s.name }))}
            className="sm:col-span-2"
          />
          <Input
            label="Référence"
            value={form.reference ?? ""}
            onChange={(e) => setForm({ ...form, reference: e.target.value })}
          />
          <Input
            label="Date facture"
            type="date"
            value={form.invoice_date ?? ""}
            onChange={(e) => setForm({ ...form, invoice_date: e.target.value })}
          />
          <MoneyField
            label="Montant HT"
            value={form.amount_ht}
            onChange={(v) => setForm({ ...form, amount_ht: v })}
          />
          <MoneyField
            label="Montant TTC"
            value={form.amount_ttc}
            onChange={(v) => setForm({ ...form, amount_ttc: v })}
          />
          <MoneyField
            label="TVA (%)"
            value={form.tax_rate_percent}
            onChange={(v) => setForm({ ...form, tax_rate_percent: v })}
          />
          <AttachmentField
            path={form.attachment_path ?? ""}
            onPick={() => void pickAttachment()}
          />
          <Textarea
            label="Notes"
            value={form.notes ?? ""}
            onChange={(e) => setForm({ ...form, notes: e.target.value })}
            className="sm:col-span-2"
          />
        </div>
      </Modal>

      <ConfirmDialog
        open={deleteTarget != null}
        title="Supprimer la facture"
        message="Confirmer la suppression de cette facture fournisseur ?"
        destructive
        onConfirm={() => void handleDelete()}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  );
}

function MoneyField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string | number | undefined;
  onChange: (v: number | undefined) => void;
}) {
  return (
    <Input
      label={label}
      value={value != null ? String(parseMoney(value)) : ""}
      onChange={(e) => {
        const parsed = parseAmountInput(e.target.value);
        onChange(parsed ?? undefined);
      }}
    />
  );
}

function AttachmentField({ path, onPick }: { path: string; onPick: () => void }) {
  return (
    <div className="sm:col-span-2 flex flex-wrap items-end gap-2">
      <Input
        label="Pièce jointe"
        value={path ? backupFileName(path) : ""}
        readOnly
        className="min-w-[12rem] flex-1"
      />
      <Button type="button" variant="secondary" onClick={onPick}>
        Parcourir…
      </Button>
    </div>
  );
}