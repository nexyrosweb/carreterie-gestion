import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { AuthProvider, useAuth } from "@/context/AuthContext";
import { ThemeProvider } from "@/context/ThemeContext";
import { ToastProvider } from "@/context/ToastContext";
import { AppShell } from "@/components/layout/AppShell";
import { Spinner } from "@/components/ui/Spinner";
import { AboutPage } from "@/pages/AboutPage";
import { ArchivesPage } from "@/pages/ArchivesPage";
import { BackupsPage } from "@/pages/BackupsPage";
import { ClientInvoicesPage } from "@/pages/ClientInvoicesPage";
import { ContactsPage } from "@/pages/ContactsPage";
import { DashboardPage } from "@/pages/DashboardPage";
import { ImportPage } from "@/pages/ImportPage";
import { LockScreen } from "@/pages/LockScreen";
import { LoginPage } from "@/pages/LoginPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { SupplierInvoicesPage } from "@/pages/SupplierInvoicesPage";

function ProtectedRoutes() {
  const { loading, authenticated, locked } = useAuth();

  if (loading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-[var(--color-page)]">
        <Spinner className="h-10 w-10 text-[var(--color-or)]" />
      </div>
    );
  }

  if (!authenticated) {
    return <LoginPage />;
  }

  return (
    <>
      {locked ? <LockScreen /> : null}
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<DashboardPage />} />
          <Route path="factures-clients" element={<ClientInvoicesPage />} />
          <Route path="factures-fournisseurs" element={<SupplierInvoicesPage />} />
          <Route path="import-pdf" element={<ImportPage />} />
          <Route path="contacts" element={<ContactsPage />} />
          <Route path="archives" element={<ArchivesPage />} />
          <Route path="sauvegardes" element={<BackupsPage />} />
          <Route path="parametres" element={<SettingsPage />} />
          <Route path="a-propos" element={<AboutPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </>
  );
}

export default function App() {
  return (
    <ThemeProvider>
      <ToastProvider>
        <AuthProvider>
          <BrowserRouter>
            <ProtectedRoutes />
          </BrowserRouter>
        </AuthProvider>
      </ToastProvider>
    </ThemeProvider>
  );
}