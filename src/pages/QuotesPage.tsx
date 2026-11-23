import { ClientInvoicesPage } from "@/pages/ClientInvoicesPage";

/** Page Devis — réutilise l’éditeur factures en mode devis. */
export function QuotesPage() {
  return <ClientInvoicesPage documentKind="devis" />;
}
