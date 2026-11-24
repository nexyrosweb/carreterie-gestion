import { useCallback, useEffect, useMemo, useState } from "react";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import {
  apiContactsCreate,
  apiContactsDelete,
  apiContactsList,
  apiContactsUpdate,
  ApiError,
} from "@/lib/api";
import {
  digitsOnly,
  formatSiretDisplay,
  frenchVatFromSiret,
  isValidSiret,
  siretFeedback,
} from "@/lib/siret";
import type { Contact, ContactInput, ContactKind } from "@/types";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { EmptyState } from "@/components/ui/EmptyState";
import { Input } from "@/components/ui/Input";
import { Modal } from "@/components/ui/Modal";
import { Spinner } from "@/components/ui/Spinner";
import { Textarea } from "@/components/ui/Textarea";
import { TBody, THead, Table, Td, Th, Tr } from "@/components/ui/Table";

type ContactTab = "client" | "fournisseur";

function matchesTab(contact: Contact, tab: ContactTab): boolean {
  if (tab === "client") {
    return contact.kind === "client" || contact.kind === "les_deux";
  }
  return contact.kind === "fournisseur" || contact.kind === "les_deux";
}

const emptyForm = (kind: ContactKind): ContactInput => ({
  kind,
  name: "",
  email: "",
  phone: "",
  address_line1: "",
  postal_code: "",
  city: "",
  country: "France",
  siret: "",
  vat_number: "",
  notes: "",
});

export function ContactsPage() {
  const { sessionToken } = useAuth();
  const { showToast } = useToast();
  const [tab, setTab] = useState<ContactTab>("client");
  const [allContacts, setAllContacts] = useState<Contact[]>([]);
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Contact | null>(null);
  const [form, setForm] = useState<ContactInput>(emptyForm("client"));
  const [formErrors, setFormErrors] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Contact | null>(null);

  const contacts = useMemo(
    () => allContacts.filter((c) => matchesTab(c, tab)),
    [allContacts, tab],
  );

  const load = useCallback(async () => {
    if (!sessionToken) return;
    setLoading(true);
    try {
      setAllContacts(await apiContactsList(sessionToken));
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Erreur chargement contacts.", "error");
      setAllContacts([]);
    } finally {
      setLoading(false);
    }
  }, [sessionToken, showToast]);

  useEffect(() => {
    void load();
  }, [load]);

  function openCreate() {
    setEditing(null);
    setForm(emptyForm(tab));
    setFormErrors({});
    setModalOpen(true);
  }

  function openEdit(contact: Contact) {
    setEditing(contact);
    setForm({
      kind: contact.kind,
      name: contact.name,
      email: contact.email ?? "",
      phone: contact.phone ?? "",
      address_line1: contact.address_line1 ?? "",
      address_line2: contact.address_line2 ?? "",
      postal_code: contact.postal_code ?? "",
      city: contact.city ?? "",
      country: contact.country ?? "France",
      siret: contact.siret ? formatSiretDisplay(contact.siret) : "",
      vat_number: contact.vat_number ?? "",
      notes: contact.notes ?? "",
    });
    setFormErrors({});
    setModalOpen(true);
  }

  function findSiretDuplicate(siretDigits: string): Contact | undefined {
    if (!siretDigits) return undefined;
    return allContacts.find(
      (c) =>
        digitsOnly(c.siret ?? "") === siretDigits &&
        (!editing || c.id !== editing.id),
    );
  }

  function onSiretChange(raw: string) {
    const display = formatSiretDisplay(raw);
    const digits = digitsOnly(raw);
    let vat = form.vat_number ?? "";
    if (digits.length === 14 && isValidSiret(digits)) {
      vat = frenchVatFromSiret(digits) ?? vat;
    }
    const next = { ...form, siret: display, vat_number: vat };
    setForm(next);
    const fb = siretFeedback(digits);
    const dup = digits.length === 14 && fb.ok ? findSiretDuplicate(digits) : undefined;
    setFormErrors((prev) => {
      const nextErr = { ...prev };
      if (!digits) delete nextErr.siret;
      else if (!fb.ok) nextErr.siret = fb.message ?? "SIRET invalide";
      else if (dup) nextErr.siret = `Doublon : déjà utilisé par « ${dup.name} »`;
      else delete nextErr.siret;
      return nextErr;
    });
  }

  async function handleSave() {
    if (!sessionToken) return;
    const errors: Record<string, string> = {};
    if (!form.name.trim()) errors.name = "Le nom est obligatoire.";
    const siretDigits = digitsOnly(form.siret ?? "");
    if (siretDigits) {
      const fb = siretFeedback(siretDigits);
      if (!fb.ok) errors.siret = fb.message ?? "SIRET invalide";
      else {
        const dup = findSiretDuplicate(siretDigits);
        if (dup) errors.siret = `Un contact avec ce SIRET existe déjà (« ${dup.name} »).`;
      }
    }
    setFormErrors(errors);
    if (Object.keys(errors).length > 0) return;

    const input: ContactInput = {
      ...form,
      siret: siretDigits || "",
      kind: editing ? form.kind : tab === "client" ? "client" : "fournisseur",
    };

    setSaving(true);
    try {
      if (editing) {
        await apiContactsUpdate(sessionToken, editing.id, input);
        showToast("Contact mis à jour.", "success");
      } else {
        await apiContactsCreate(sessionToken, input);
        showToast("Contact créé.", "success");
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
      await apiContactsDelete(sessionToken, deleteTarget.id);
      showToast("Contact supprimé.", "success");
      setDeleteTarget(null);
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Suppression impossible.", "error");
    }
  }

  return (
    <div className="space-y-4">
      <nav className="flex gap-2" aria-label="Type de contacts">
        {(["client", "fournisseur"] as const).map((k) => (
          <Button key={k} variant={tab === k ? "gold" : "secondary"} onClick={() => setTab(k)}>
            {k === "client" ? "Clients" : "Fournisseurs"}
          </Button>
        ))}
        <div className="flex-1" />
        <Button onClick={openCreate}>Nouveau {tab === "client" ? "client" : "fournisseur"}</Button>
      </nav>

      <Card>
        {loading ? (
          <div className="flex justify-center py-12">
            <Spinner className="text-[var(--color-or)]" />
          </div>
        ) : contacts.length === 0 ? (
          <EmptyState
            title={`Aucun ${tab === "client" ? "client" : "fournisseur"}`}
            description="Ajoutez un contact pour commencer."
            action={<Button onClick={openCreate}>Créer</Button>}
          />
        ) : (
          <Table>
            <THead>
              <tr>
                <Th>Nom</Th>
                <Th>Type</Th>
                <Th>Coordonnées</Th>
                <Th>Ville</Th>
                <Th className="text-right">Actions</Th>
              </tr>
            </THead>
            <TBody>
              {contacts.map((c) => (
                <Tr key={c.id}>
                  <Td className="font-medium">{c.name}</Td>
                  <Td className="text-xs text-[var(--color-text-muted)]">{c.kind}</Td>
                  <Td>
                    <div className="text-xs text-[var(--color-text-muted)]">
                      {c.email || "—"}
                      {c.phone ? ` – ${c.phone}` : ""}
                    </div>
                  </Td>
                  <Td>{c.city || "—"}</Td>
                  <Td className="text-right">
                    <p className="flex justify-end gap-1">
                      <Button size="sm" variant="secondary" onClick={() => openEdit(c)}>
                        Modifier
                      </Button>
                      <Button size="sm" variant="danger" onClick={() => setDeleteTarget(c)}>
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
        title={editing ? "Modifier le contact" : "Nouveau contact"}
        size="lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setModalOpen(false)}>
              Annuler
            </Button>
            <Button onClick={() => void handleSave()} loading={saving}>
              Enregistrer
            </Button>
          </>
        }
      >
        <div className="grid gap-4 sm:grid-cols-2">
          <Input
            label="Nom *"
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
            error={formErrors.name}
            className="sm:col-span-2"
          />
          <Input label="E-mail" type="email" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} />
          <Input label="Téléphone" value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} />
          <Input label="Adresse" value={form.address_line1} onChange={(e) => setForm({ ...form, address_line1: e.target.value })} className="sm:col-span-2" />
          <Input label="Code postal" value={form.postal_code} onChange={(e) => setForm({ ...form, postal_code: e.target.value })} />
          <Input label="Ville" value={form.city} onChange={(e) => setForm({ ...form, city: e.target.value })} />
          <Input
            label="SIRET"
            value={form.siret}
            onChange={(e) => onSiretChange(e.target.value)}
            error={formErrors.siret}
            hint={
              !formErrors.siret && digitsOnly(form.siret ?? "").length === 14
                ? `SIREN : ${digitsOnly(form.siret ?? "").slice(0, 9)}`
                : "14 chiffres — formatage et contrôle auto"
            }
          />
          <Input
            label="N° TVA"
            value={form.vat_number}
            onChange={(e) => setForm({ ...form, vat_number: e.target.value.toUpperCase() })}
            hint="Calculé depuis le SIRET si laissé vide"
          />
          <Textarea label="Notes" value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} className="sm:col-span-2" />
        </div>
      </Modal>

      <ConfirmDialog
        open={deleteTarget != null}
        title="Supprimer le contact"
        message={`Confirmer la suppression de « ${deleteTarget?.name ?? ""} » ? Cette action est irréversible.`}
        destructive
        onConfirm={() => void handleDelete()}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  );
}