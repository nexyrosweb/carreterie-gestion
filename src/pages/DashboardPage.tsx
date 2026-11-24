import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import { apiDashboardStats, ApiError } from "@/lib/api";
import { formatMoneyDecimal, parseMoney } from "@/lib/money";
import type { DashboardStats } from "@/types";
import { Card } from "@/components/ui/Card";
import { Spinner } from "@/components/ui/Spinner";
import { EmptyState } from "@/components/ui/EmptyState";

export function DashboardPage() {
  const { sessionToken } = useAuth();
  const { showToast } = useToast();
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    if (!sessionToken) return;
    setLoading(true);
    try {
      setStats(await apiDashboardStats(sessionToken));
    } catch (err) {
      showToast(
        err instanceof ApiError ? err.message : "Impossible de charger le tableau de bord.",
        "error",
      );
      setStats(null);
    } finally {
      setLoading(false);
    }
  }, [sessionToken, showToast]);

  useEffect(() => {
    void load();
  }, [load]);

  const chartData = useMemo(
    () =>
      (stats?.monthly_activity ?? []).map((row) => ({
        name: row.month,
        recettes: parseMoney(row.revenue_ht),
        depenses: parseMoney(row.supplier_ttc),
      })),
    [stats],
  );

  if (loading) {
    return (
      <div className="flex justify-center py-20">
        <Spinner className="h-8 w-8 text-[var(--color-or)]" />
      </div>
    );
  }

  const kpis = [
    { label: "CA HT (factures émises)", value: formatMoneyDecimal(stats?.total_issued_ht) },
    { label: "Recettes TTC", value: formatMoneyDecimal(stats?.total_issued_ttc) },
    { label: "Impayés clients TTC", value: formatMoneyDecimal(stats?.total_unpaid_ttc) },
    { label: "Devis en attente", value: String(stats?.pending_quotes_count ?? 0) },
    { label: "Devis émis", value: String(stats?.quotes_issued_count ?? 0) },
    { label: "Factures impayées", value: String(stats?.unpaid_invoices_count ?? 0) },
    { label: "Factures émises", value: String(stats?.issued_invoices_count ?? 0) },
    { label: "Brouillons factures", value: String(stats?.drafts_count ?? 0) },
    { label: "Avoirs", value: String(stats?.credit_notes_count ?? 0) },
    { label: "Contacts", value: String(stats?.contacts_count ?? 0) },
    { label: "Dépenses fournisseurs TTC", value: formatMoneyDecimal(stats?.total_supplier_ttc) },
    { label: "Factures fournisseurs", value: String(stats?.supplier_invoices_count ?? 0) },
  ];

  return (
    <div className="space-y-6">
      <section className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {kpis.map((kpi) => (
          <Card key={kpi.label} className="!p-0">
            <div className="p-4">
              <p className="text-xs text-[var(--color-text-muted)]">{kpi.label}</p>
              <p className="mt-1 text-2xl font-semibold text-[var(--color-or)]">{kpi.value}</p>
            </div>
          </Card>
        ))}
      </section>

      <Card title="Activité mensuelle (CA HT / dépenses TTC)">
        {chartData.length === 0 ? (
          <EmptyState
            title="Aucune activité mensuelle"
            description="Émettez des factures ou enregistrez des dépenses fournisseurs pour alimenter le graphique."
          />
        ) : (
          <div className="mt-4 h-64 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={chartData}>
                <CartesianGrid strokeDasharray="3 3" stroke="#3a3a3a" />
                <XAxis dataKey="name" tick={{ fill: "#9a9590", fontSize: 11 }} />
                <YAxis tick={{ fill: "#9a9590", fontSize: 11 }} />
                <Tooltip
                  contentStyle={{
                    background: "#2a2a2a",
                    border: "1px solid #3a3a3a",
                    borderRadius: 8,
                  }}
                />
                <Bar dataKey="recettes" name="CA HT" fill="var(--color-or)" />
                <Bar dataKey="depenses" name="Dépenses TTC" fill="var(--color-rouge)" />
              </BarChart>
            </ResponsiveContainer>
          </div>
        )}
      </Card>
    </div>
  );
}
