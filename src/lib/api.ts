import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  AppSettings,
  AppSettingsPatch,
  ImportLimits,
  UserProfile,
  UserProfilePatch,
  AuditEntry,
  BackupInfo,
  Contact,
  ContactInput,
  ContactMatchCandidate,
  DashboardStats,
  ErrorPayload,
  FirstLaunchStatus,
  ImportAnalyzeResult,
  ImportedDocument,
  ImportValidateInput,
  ConvertDevisInput,
  Invoice,
  InvoiceDraftInput,
  InvoiceLineInput,
  InvoicePayment,
  LoginResponse,
  RecordPaymentInput,
  SendDocumentEmailInput,
  SupplierInvoice,
  SupplierInvoiceInput,
} from "@/types";

export class ApiError extends Error {
  readonly code?: string;
  readonly disclaimer?: string;

  constructor(message: string, code?: string, disclaimer?: string) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.disclaimer = disclaimer;
  }
}

function parseInvokeError(err: unknown): ApiError {
  if (typeof err === "string") {
    try {
      const parsed = JSON.parse(err) as ErrorPayload;
      if (parsed.message) {
        return new ApiError(parsed.message, parsed.code, parsed.disclaimer);
      }
    } catch {
      return new ApiError(err);
    }
    return new ApiError(err);
  }
  if (err instanceof Error) {
    return new ApiError(err.message);
  }
  return new ApiError("Erreur inconnue lors de l'appel API.");
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Hors fenêtre Tauri (navigateur / aperçu Cursor), l'IPC n'existe pas.
  if (!isTauri() || !(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) {
    throw new ApiError(
      "L'application doit tourner dans la fenêtre Carreterie Gestion (npm run tauri dev), pas dans un navigateur web.",
      "HORS_TAURI",
    );
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    if (msg.includes("invoke") && msg.includes("undefined")) {
      throw new ApiError(
        "Connexion Tauri indisponible. Fermez l'onglet navigateur et utilisez la fenêtre native de l'application.",
        "HORS_TAURI",
      );
    }
    throw parseInvokeError(e);
  }
}

/** Tauri 2 convertit les paramètres Rust snake_case → camelCase côté JS. */
function withSession(
  sessionToken: string | null | undefined,
  extra?: Record<string, unknown>,
): Record<string, unknown> {
  if (!sessionToken) {
    throw new ApiError("Session requise. Veuillez vous reconnecter.", "SESSION_INVALIDE");
  }
  return { sessionToken, ...extra };
}

// — App / Auth —
export const apiAppDisclaimer = () => call<string>("app_disclaimer");

export const apiDbPing = (sessionToken?: string | null) =>
  call<{ connected: boolean; is_demo: boolean }>(
    "db_ping",
    sessionToken ? { sessionToken } : {},
  );

export const apiAuthFirstLaunch = () => call<FirstLaunchStatus>("auth_first_launch");

export const apiAuthSetupAdmin = (username: string, password: string) =>
  call<void>("auth_setup_admin", { username, password });

export const apiAuthLogin = (username: string, password: string) =>
  call<LoginResponse>("auth_login", { username, password });

export const apiAuthEnterDemo = () => call<LoginResponse>("auth_enter_demo");

export const apiDemoReset = (sessionToken: string) =>
  call<void>("demo_reset", withSession(sessionToken));

export const apiAuthLogout = (sessionToken: string) =>
  call<void>("auth_logout", { sessionToken });

export const apiAuthSessionPing = (sessionToken: string) =>
  call<LoginResponse>("auth_session_ping", { sessionToken });

export const apiAuthChangePassword = (
  sessionToken: string,
  currentPassword: string,
  newPassword: string,
) =>
  call<void>("auth_change_password", {
    sessionToken,
    currentPassword,
    newPassword,
  });

export const apiAuthProfileGet = (sessionToken: string) =>
  call<UserProfile>("auth_profile_get", withSession(sessionToken));

export const apiAuthProfileUpdate = (sessionToken: string, patch: UserProfilePatch) =>
  call<UserProfile>("auth_profile_update", withSession(sessionToken, { patch }));

export const apiAuthProfileSetAvatar = (sessionToken: string, sourcePath: string) =>
  call<UserProfile>(
    "auth_profile_set_avatar",
    withSession(sessionToken, { sourcePath }),
  );

// — Contacts —
export const apiContactsList = (sessionToken: string, search?: string) =>
  call<Contact[]>(
    "contacts_list",
    withSession(sessionToken, search ? { search } : undefined),
  );

export const apiContactsGet = (sessionToken: string, id: string) =>
  call<Contact>("contacts_get", withSession(sessionToken, { id }));

export const apiContactsCreate = (sessionToken: string, input: ContactInput) =>
  call<Contact>("contacts_create", withSession(sessionToken, { input }));

export const apiContactsUpdate = (
  sessionToken: string,
  id: string,
  input: ContactInput,
) => call<Contact>("contacts_update", withSession(sessionToken, { id, input }));

export const apiContactsDelete = (sessionToken: string, id: string) =>
  call<void>("contacts_delete", withSession(sessionToken, { id }));

// — Client invoices —
export const apiInvoicesList = (
  sessionToken: string,
  status?: string,
  kind?: string,
) =>
  call<Invoice[]>(
    "invoices_list",
    withSession(sessionToken, {
      ...(status ? { status } : {}),
      ...(kind ? { kind } : {}),
    }),
  );

export const apiInvoicesGet = (sessionToken: string, id: string) =>
  call<Invoice>("invoices_get", withSession(sessionToken, { id }));

export const apiInvoicesCreateDraft = (
  sessionToken: string,
  input: InvoiceDraftInput,
) => call<Invoice>("invoices_create_draft", withSession(sessionToken, { input }));

export const apiInvoicesUpdateDraft = (
  sessionToken: string,
  id: string,
  input: InvoiceDraftInput,
) =>
  call<Invoice>("invoices_update_draft", withSession(sessionToken, { id, input }));

export const apiInvoicesDeleteDraft = (sessionToken: string, id: string) =>
  call<void>("invoices_delete_draft", withSession(sessionToken, { id }));

export const apiInvoicesIssue = (sessionToken: string, id: string) =>
  call<Invoice>("invoices_issue", withSession(sessionToken, { id }));

export const apiInvoicesCreateCreditNote = (
  sessionToken: string,
  relatedInvoiceId: string,
  lines: InvoiceLineInput[],
  customerNotes?: string,
) =>
  call<Invoice>("invoices_create_credit_note", {
    sessionToken,
    relatedInvoiceId,
    lines,
    customerNotes,
  });

export const apiInvoicesGeneratePdf = (sessionToken: string, id: string) =>
  call<string>("invoices_generate_pdf", withSession(sessionToken, { id }));

export const apiInvoicesPreviewPdf = (sessionToken: string, id: string) =>
  call<string>("invoices_preview_pdf", withSession(sessionToken, { id }));

export const apiInvoicesRecordPayment = (
  sessionToken: string,
  input: RecordPaymentInput,
) => call<Invoice>("invoices_record_payment", withSession(sessionToken, { input }));

export const apiInvoicesListPayments = (sessionToken: string, invoiceId: string) =>
  call<InvoicePayment[]>(
    "invoices_list_payments",
    withSession(sessionToken, { invoiceId }),
  );

export const apiInvoicesCreateFromDevis = (
  sessionToken: string,
  input: ConvertDevisInput,
) => call<Invoice>("invoices_create_from_devis", withSession(sessionToken, { input }));

export const apiInvoicesAcceptDevis = (sessionToken: string, devisId: string) =>
  call<Invoice>("invoices_accept_devis", withSession(sessionToken, { devisId }));

export const apiMailSendDocument = (
  sessionToken: string,
  input: SendDocumentEmailInput,
) => call<void>("mail_send_document", withSession(sessionToken, { input }));

export const apiMailTestConnection = (sessionToken: string) =>
  call<string>("mail_test_connection", withSession(sessionToken));

export const apiAccountingExportFecLite = (
  sessionToken: string,
  opts?: { dateFrom?: string; dateTo?: string },
) =>
  call<string>(
    "accounting_export_fec_lite",
    withSession(sessionToken, {
      dateFrom: opts?.dateFrom,
      dateTo: opts?.dateTo,
    }),
  );

export const apiContractsGenerateFromDevis = (sessionToken: string, devisId: string) =>
  call<string>("contracts_generate_from_devis", withSession(sessionToken, { devisId }));

export const apiContractsPreviewFromDevis = (sessionToken: string, devisId: string) =>
  call<string>("contracts_preview_from_devis", withSession(sessionToken, { devisId }));

export const apiSettingsSetLogo = (sessionToken: string, sourcePath: string) =>
  call<AppSettings>("settings_set_logo", withSession(sessionToken, { sourcePath }));

// — Supplier invoices (suppliers_*) —
export const apiSuppliersList = (sessionToken: string) =>
  call<SupplierInvoice[]>("suppliers_list", withSession(sessionToken));

export const apiSuppliersGet = (sessionToken: string, id: string) =>
  call<SupplierInvoice>("suppliers_get", withSession(sessionToken, { id }));

export const apiSuppliersCreate = (
  sessionToken: string,
  input: SupplierInvoiceInput,
) => call<SupplierInvoice>("suppliers_create", withSession(sessionToken, { input }));

export const apiSuppliersUpdate = (
  sessionToken: string,
  id: string,
  input: SupplierInvoiceInput,
) =>
  call<SupplierInvoice>("suppliers_update", withSession(sessionToken, { id, input }));

export const apiSuppliersDelete = (sessionToken: string, id: string) =>
  call<void>("suppliers_delete", withSession(sessionToken, { id }));

// — Dashboard —
export const apiDashboardStats = (sessionToken: string) =>
  call<DashboardStats>("dashboard_stats", withSession(sessionToken));

// — Archives / audit —
export const apiArchivesExportCsv = (sessionToken: string) =>
  call<string>("archives_export_csv", withSession(sessionToken));

export const apiAuditList = (sessionToken: string, limit?: number) =>
  call<AuditEntry[]>(
    "audit_list",
    withSession(sessionToken, limit != null ? { limit } : undefined),
  );

// — Backups —
export const apiBackupList = (sessionToken: string) =>
  call<BackupInfo[]>("backup_list", withSession(sessionToken)).catch(() => [] as BackupInfo[]);

export const apiBackupCreate = (sessionToken: string, password?: string) =>
  call<BackupInfo>(
    "backup_create",
    withSession(sessionToken, password ? { password } : undefined),
  );

export const apiBackupRestore = (
  sessionToken: string,
  backupPath: string,
  password?: string,
) =>
  call<void>("backup_restore", {
    sessionToken,
    backupPath,
    password: password ?? null,
  });

// — Settings —
export const apiSettingsGet = (sessionToken: string) =>
  call<AppSettings>("settings_get", withSession(sessionToken));

export const apiSettingsUpdate = (sessionToken: string, patch: AppSettingsPatch) =>
  call<AppSettings>("settings_update", withSession(sessionToken, { patch }));

// — Import PDF —
export const apiImportLimitsGet = (sessionToken: string) =>
  call<ImportLimits>("import_limits_get", withSession(sessionToken));

export const apiImportAnalyzePdfs = (
  sessionToken: string,
  paths: string[],
  direction: string,
) =>
  call<ImportAnalyzeResult[]>(
    "import_analyze_pdfs",
    withSession(sessionToken, { paths, direction }),
  );

export const apiImportList = (
  sessionToken: string,
  filters?: {
    status?: string;
    direction?: string;
    year?: number;
    contactId?: string;
    documentType?: string;
  },
) =>
  call<ImportedDocument[]>(
    "import_list",
    withSession(sessionToken, {
      status: filters?.status,
      direction: filters?.direction,
      year: filters?.year,
      contactId: filters?.contactId,
      documentType: filters?.documentType,
    }),
  );

export const apiImportGet = (sessionToken: string, id: string) =>
  call<ImportedDocument>("import_get", withSession(sessionToken, { id }));

export const apiImportPdfData = (sessionToken: string, id: string) =>
  call<string>("import_pdf_data", withSession(sessionToken, { id }));

export const apiImportMatchContacts = (
  sessionToken: string,
  opts?: { siret?: string; siren?: string; name?: string },
) =>
  call<ContactMatchCandidate[]>(
    "import_match_contacts",
    withSession(sessionToken, {
      siret: opts?.siret,
      siren: opts?.siren,
      name: opts?.name,
    }),
  );

export const apiImportValidate = (sessionToken: string, input: ImportValidateInput) =>
  call<ImportedDocument>("import_validate", withSession(sessionToken, { input }));

export const apiImportReject = (sessionToken: string, id: string, reason?: string) =>
  call<ImportedDocument>(
    "import_reject",
    withSession(sessionToken, { id, reason }),
  );
