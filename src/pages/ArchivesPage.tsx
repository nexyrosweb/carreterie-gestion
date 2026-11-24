import { useCallback, useEffect, useState } from "react";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import {
  apiAccountingExportFecLite,
  apiArchivesExportCsv,
  apiAuditList,
  apiInvoicesList,
  ApiError,
} from "@/lib/api";
import { formatMoneyDecimal } from "@/lib/money";
import type { AuditEntry, Invoice } from "@/types";
import { Badge, invoiceStatusBadge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { Input } from "@/components/ui/Input";
import { Select } from "@/components/ui/Select";
import { Spinner } from "@/components/ui/Spinner";
import { TBody, THead, Table, Td, Th, Tr } from "@/components/ui/Table";
import { format } from "date-fns";
import { fr } from "date-fns/locale";

export function ArchivesPage() {
  const { sessionToken } = useAuth();
  const { showToast } = useToast();
  const [statusFilter, setStatusFilter] = useState("");
  const [query, setQuery] = useState("");
  const [invoices, setInvoices] = useState<Invoice[]>([]);
  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [fecFrom, setFecFrom] = useState("");
  const [fecTo, setFecTo] = useState("");
  const [exportingFec, setExportingFec] = useState(false);

  const load = useCallback(async () => {
    if (!sessionToken) return;
    setLoading(true);
    try {
      const [inv, log] = await Promise.all([
        apiInvoicesList(sessionToken, statusFilter || undefined),
        apiAuditList(sessionToken, 100),
      ]);
      setInvoices(inv);
      setAudit(log);
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Chargement impossible.", "error");
      setInvoices([]);
      setAudit([]);
    } finally {
      setLoading(false);
    }
  }, [sessionToken, statusFilter, showToast]);

  useEffect(() => {
    void load();
  }, [load]);

  const filteredInvoices = (() => {
    const q = query.trim().toLowerCase();
    if (!q) return invoices;
    return invoices.filter(
      (inv) =>
        (inv.number ?? "").toLowerCase().includes(q) ||
        inv.id.toLowerCase().includes(q) ||
        (inv.customer_notes ?? "").toLowerCase().includes(q),
    );
  })();

  async function exportCsv() {
    if (!sessionToken) return;
    setExporting(true);
    try {
      const path = await apiArchivesExportCsv(sessionToken);
      showToast(`Export CSV : ${path}`, "success");
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Export impossible.", "error");
    } finally {
      setExporting(false);
    }
  }

  async function exportFec() {
    if (!sessionToken) return;
    setExportingFec(true);
    try {
      const path = await apiAccountingExportFecLite(sessionToken, {
        dateFrom: fecFrom || undefined,
        dateTo: fecTo || undefined,
      });
      showToast(`Export FEC-lite : ${path}`, "success");
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Export comptable impossible.", "error");
    } finally {
      setExportingFec(false);
    }
  }

  return (
    <div className="space-y-6">
      <Card title="Factures archivées">
        <div className="mb-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <Input
            label="Recherche"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="N°, notes…"
          />
          <Select
            label="Statut"
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            options={[
              { value: "", label: "Tous" },
              { value: "brouillon", label: "Brouillons" },
              { value: "emise", label: "Émises" },
              { value: "avoir", label: "Avoirs" },
            ]}
          />
        </div>
        <div className="flex flex-wrap gap-3">
          <Button variant="gold" loading={exporting} onClick={() => void exportCsv()}>
            Exporter toutes les factures (CSV)
          </Button>
        </div>
        <div className="mt-4 grid gap-3 border-t border-[var(--color-border)] pt-4 sm:grid-cols-3">
          <Input
            label="FEC — du"
            type="date"
            value={fecFrom}
            onChange={(e) => setFecFrom(e.target.value)}
          />
          <Input
            label="FEC — au"
            type="date"
            value={fecTo}
            onChange={(e) => setFecTo(e.target.value)}
          />
          <div className="flex items-end">
            <Button
              className="w-full"
              variant="secondary"
              loading={exportingFec}
              onClick={() => void exportFec()}
            >
              Export comptable (FEC-lite)
            </Button>
          </div>
        </div>
      </Card>

      <Card title="Liste des factures">
        {loading ? (
          <Spinner className="mx-auto py-10 text-[var(--color-or)]" />
        ) : filteredInvoices.length === 0 ? (
          <EmptyState title="Aucune facture" />
        ) : (
          <Table>
            <THead>
              <tr>
                <Th>N°</Th>
                <Th>Statut</Th>
                <Th>Date</Th>
                <Th className="text-right">TTC</Th>
              </tr>
            </THead>
            <TBody>
              {filteredInvoices.map((inv) => {
                const st = invoiceStatusBadge(inv.status);
                return (
                  <Tr key={inv.id}>
                    <Td>{inv.number ?? "—"}</Td>
                    <Td>
                      <Badge tone={st.tone}>{st.label}</Badge>
                    </Td>
                    <Td>
                      {inv.issue_date
                        ? format(new Date(inv.issue_date), "dd/MM/yyyy", { locale: fr })
                        : "—"}
                    </Td>
                    <Td className="text-right">{formatMoneyDecimal(inv.total_ttc)}</Td>
                  </Tr>
                );
              })}
            </TBody>
          </Table>
        )}
      </Card>

      <Card title="Journal d'audit">
        {audit.length === 0 ? (
          <EmptyState title="Journal vide" />
        ) : (
          <Table>
            <THead>
              <tr>
                <Th>Date</Th>
                <Th>Action</Th>
                <Th>Entité</Th>
                <Th>Détails</Th>
              </tr>
            </THead>
            <TBody>
              {audit.map((a) => (
                <Tr key={a.id}>
                  <Td>{format(new Date(a.created_at), "dd/MM/yyyy HH:mm", { locale: fr })}</Td>
                  <Td>{a.action}</Td>
                  <Td className="text-xs text-[var(--color-text-muted)]">
                    {a.entity_type ?? "—"}
                    {a.entity_id ? ` #${a.entity_id.slice(0, 8)}` : ""}
                  </Td>
                  <Td className="text-xs text-[var(--color-text-muted)]">{a.details ?? "—"}</Td>
                </Tr>
              ))}
            </TBody>
          </Table>
        )}
      </Card>
    </div>
  );
}