import { useCallback, useEffect, useState } from "react";
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
import { formatMoneyDecimal } from "@/lib/money";
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

  if (loading) {
    return (
      <div className="flex justify-center py-20">
        <Spinner className="h-8 w-8 text-[var(--color-or)]" />
      </div>
    );
  }

  const kpis = [
    { label: "Recettes (factures émises TTC)", value: formatMoneyDecimal(stats?.total_issued_ttc) },
    { label: "Dépenses fournisseurs TTC", value: formatMoneyDecimal(stats?.total_supplier_ttc) },
    { label: "Impayés clients TTC", value: formatMoneyDecimal(stats?.total_unpaid_ttc) },
    { label: "Factures impayées", value: String(stats?.unpaid_invoices_count ?? 0) },
    { label: "Factures émises", value: String(stats?.issued_invoices_count ?? 0) },
    { label: "Brouillons", value: String(stats?.drafts_count ?? 0) },
    { label: "Avoirs", value: String(stats?.credit_notes_count ?? 0) },
    { label: "Contacts", value: String(stats?.contacts_count ?? 0) },
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

      <Card title="Activité mensuelle">
        <EmptyState
          title="Données mensuelles non disponibles"
          description="Les graphiques seront activés lorsqu'une agrégation mensuelle sera exposée par le backend."
        />
        <DashboardChartPlaceholder />
      </Card>
    </div>
  );
}

function DashboardChartPlaceholder() {
  const chartData: Array<{ name: string; recettes: number; depenses: number }> = [];
  return (
    <div className="mt-4 h-48 w-full opacity-40">
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
          <Bar dataKey="recettes" name="Recettes" fill="var(--color-or)" />
          <Bar dataKey="depenses" name="Dépenses" fill="var(--color-rouge)" />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}