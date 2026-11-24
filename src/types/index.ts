/** Types partagés – alignés sur les modèles serde Rust (snake_case) */

export type DecimalValue = string | number;

export type ContactKind = "client" | "fournisseur" | "les_deux";

export type InvoiceStatus = "brouillon" | "emise" | "avoir";

export type InvoiceKind = "facture" | "avoir" | "devis";

export type InvoiceLineKind = "standard" | "location_salle";

export type PaymentType = "reglement" | "acompte" | "solde";

export interface ErrorPayload {
  message: string;
  code: string;
  disclaimer?: string;
}

export interface FirstLaunchStatus {
  needs_setup: boolean;
  disclaimer: string;
}

export interface LoginResponse {
  token: string;
  username: string;
  user_id: string;
  disclaimer: string;
  is_demo?: boolean;
}

export interface Contact {
  id: string;
  kind: ContactKind;
  name: string;
  email?: string | null;
  phone?: string | null;
  address_line1?: string | null;
  address_line2?: string | null;
  postal_code?: string | null;
  city?: string | null;
  country: string;
  siret?: string | null;
  vat_number?: string | null;
  notes?: string | null;
  created_at: string;
  updated_at: string;
}

export interface ContactInput {
  kind: ContactKind;
  name: string;
  email?: string;
  phone?: string;
  address_line1?: string;
  address_line2?: string;
  postal_code?: string;
  city?: string;
  country?: string;
  siret?: string;
  vat_number?: string;
  notes?: string;
}

export interface InvoiceLine {
  id: string;
  invoice_id: string;
  position: number;
  description: string;
  quantity: DecimalValue;
  unit_price_ht: DecimalValue;
  tax_rate_percent: DecimalValue;
  line_ht: DecimalValue;
  line_tva: DecimalValue;
  line_ttc: DecimalValue;
  line_kind?: InvoiceLineKind;
}

export interface InvoiceLineInput {
  description: string;
  quantity: DecimalValue;
  unit_price_ht: DecimalValue;
  tax_rate_percent: DecimalValue;
  line_kind?: InvoiceLineKind;
}

export interface Invoice {
  id: string;
  number?: string | null;
  contact_id: string;
  status: InvoiceStatus;
  invoice_kind: InvoiceKind;
  related_invoice_id?: string | null;
  issue_date?: string | null;
  due_date?: string | null;
  customer_notes?: string | null;
  total_ht: DecimalValue;
  total_tva: DecimalValue;
  total_ttc: DecimalValue;
  amount_paid?: DecimalValue;
  payment_status?: string;
  lines: InvoiceLine[];
  created_at: string;
  updated_at: string;
  issued_at?: string | null;
  accepted_at?: string | null;
  rental_start?: string | null;
  rental_end?: string | null;
}

export interface InvoicePayment {
  id: string;
  invoice_id: string;
  amount: DecimalValue;
  paid_at: string;
  method?: string | null;
  reference?: string | null;
  notes?: string | null;
  payment_type: PaymentType;
  created_at: string;
}

export interface RecordPaymentInput {
  invoice_id: string;
  amount: DecimalValue;
  paid_at: string;
  method?: string;
  reference?: string;
  notes?: string;
  payment_type?: PaymentType;
}

export interface InvoiceDraftInput {
  contact_id: string;
  issue_date?: string;
  due_date?: string;
  customer_notes?: string;
  lines: InvoiceLineInput[];
  invoice_kind?: InvoiceKind;
  related_invoice_id?: string;
  rental_start?: string;
  rental_end?: string;
}

export interface ConvertDevisInput {
  devis_id: string;
  issue_immediately?: boolean;
}

export interface SendDocumentEmailInput {
  invoice_id: string;
  to_email?: string;
  subject?: string;
  body?: string;
}

export interface SupplierInvoice {
  id: string;
  supplier_contact_id?: string | null;
  reference?: string | null;
  invoice_date?: string | null;
  amount_ht?: DecimalValue | null;
  amount_ttc?: DecimalValue | null;
  tax_rate_percent?: DecimalValue | null;
  notes?: string | null;
  attachment_path?: string | null;
  created_at: string;
  updated_at: string;
}

export interface SupplierInvoiceInput {
  supplier_contact_id?: string;
  reference?: string;
  invoice_date?: string;
  amount_ht?: DecimalValue;
  amount_ttc?: DecimalValue;
  tax_rate_percent?: DecimalValue;
  notes?: string;
  attachment_path?: string;
}

export interface MonthlyActivityRow {
  month: string;
  revenue_ht: DecimalValue;
  supplier_ttc: DecimalValue;
}

export interface DashboardStats {
  contacts_count: number;
  drafts_count: number;
  issued_invoices_count: number;
  credit_notes_count: number;
  supplier_invoices_count: number;
  unpaid_invoices_count: number;
  total_issued_ttc: DecimalValue;
  total_supplier_ttc: DecimalValue;
  total_unpaid_ttc: DecimalValue;
  total_issued_ht: DecimalValue;
  total_unpaid_ht: DecimalValue;
  pending_quotes_count: number;
  quotes_issued_count: number;
  monthly_activity: MonthlyActivityRow[];
}

export interface AuditEntry {
  id: number;
  created_at: string;
  user_id?: string | null;
  action: string;
  entity_type?: string | null;
  entity_id?: string | null;
  details?: string | null;
}

export interface BackupInfo {
  path: string;
  encrypted: boolean;
  created_at: string;
  filename?: string;
  size_bytes?: number;
}

export interface AppSettings {
  theatre_name: string;
  theatre_address: string;
  theatre_siret: string;
  theatre_vat_number: string;
  logo_path: string;
  invoice_prefix: string;
  credit_note_prefix: string;
  invoice_year: string;
  invoice_last_sequence: number;
  credit_note_last_sequence: number;
  legal_mentions: string;
  default_tax_rate: DecimalValue;
  export_folder: string;
  smtp_enabled: boolean;
  smtp_host: string;
  smtp_port: number;
  smtp_encryption: string;
  smtp_username: string;
  smtp_password: string;
  smtp_from_name: string;
  smtp_from_email: string;
  smtp_reply_to: string;
  smtp_cc: string;
  smtp_bcc: string;
  smtp_timeout_secs: number;
  devis_prefix: string;
  bank_iban: string;
  bank_account: string;
  bank_bic: string;
  bank_name: string;
  bank_holder: string;
  pdf_primary_color: string;
  pdf_header_subtitle: string;
  pdf_use_custom_logo: boolean;
  /** `classique` | `compact` | `billetterie` */
  pdf_template: string;
}

export interface AppSettingsPatch {
  theatre_name?: string;
  theatre_address?: string;
  theatre_siret?: string;
  theatre_vat_number?: string;
  logo_path?: string;
  invoice_prefix?: string;
  credit_note_prefix?: string;
  legal_mentions?: string;
  default_tax_rate?: DecimalValue;
  export_folder?: string;
  smtp_enabled?: boolean;
  smtp_host?: string;
  smtp_port?: number;
  smtp_encryption?: string;
  smtp_username?: string;
  smtp_password?: string;
  smtp_from_name?: string;
  smtp_from_email?: string;
  smtp_reply_to?: string;
  smtp_cc?: string;
  smtp_bcc?: string;
  smtp_timeout_secs?: number;
  devis_prefix?: string;
  bank_iban?: string;
  bank_account?: string;
  bank_bic?: string;
  bank_name?: string;
  bank_holder?: string;
  pdf_primary_color?: string;
  pdf_header_subtitle?: string;
  pdf_use_custom_logo?: boolean;
  pdf_template?: string;
}

export interface UserProfile {
  id: string;
  username: string;
  email: string;
  first_name: string;
  last_name: string;
  avatar_path: string;
}

export interface UserProfilePatch {
  email?: string;
  first_name?: string;
  last_name?: string;
  avatar_path?: string;
}

export interface ImportLimits {
  max_bytes: number;
  max_mb: number;
  free_disk_bytes: number;
  accepted_extensions: string[];
  accepted_label: string;
}

/** Module import PDF */
export type ImportDirection = "client" | "fournisseur";
export type ImportDocumentType = "facture" | "acompte" | "avoir" | "inconnu";
export type ImportStatus = "en_attente" | "valide" | "rejete" | "doublon";
export type ExtractionSource = "texte" | "ocr" | "manuel" | "aucun";

export interface ImportedDocument {
  id: string;
  file_hash: string;
  original_filename: string;
  stored_path: string;
  direction: ImportDirection;
  document_type: ImportDocumentType;
  status: ImportStatus;
  extraction_source: ExtractionSource;
  issuer_name?: string | null;
  party_name?: string | null;
  party_address?: string | null;
  siren?: string | null;
  siret?: string | null;
  invoice_number?: string | null;
  issue_date?: string | null;
  payment_date?: string | null;
  payment_method?: string | null;
  payment_status?: string | null;
  description?: string | null;
  total_ht?: string | null;
  tax_rate?: string | null;
  total_tva?: string | null;
  total_ttc?: string | null;
  related_invoice_ref?: string | null;
  contact_id?: string | null;
  linked_draft_invoice_id?: string | null;
  linked_supplier_invoice_id?: string | null;
  uncertain_fields: string[];
  missing_fields: string[];
  amounts_coherent?: boolean | null;
  raw_text_excerpt?: string | null;
  ocr_available: boolean;
  notes?: string | null;
  created_at: string;
  updated_at: string;
  validated_at?: string | null;
  validated_by?: string | null;
}

export interface ContactMatchCandidate {
  id: string;
  name: string;
  siret?: string | null;
  score: number;
  match_reason: string;
}

export interface ImportAnalyzeResult {
  document: ImportedDocument;
  matches: ContactMatchCandidate[];
  warnings: string[];
}

export interface ImportValidateInput {
  id: string;
  direction: ImportDirection;
  document_type: ImportDocumentType;
  issuer_name?: string | null;
  party_name?: string | null;
  party_address?: string | null;
  siren?: string | null;
  siret?: string | null;
  invoice_number?: string | null;
  issue_date?: string | null;
  payment_date?: string | null;
  payment_method?: string | null;
  payment_status?: string | null;
  description?: string | null;
  total_ht?: string | null;
  tax_rate?: string | null;
  total_tva?: string | null;
  total_ttc?: string | null;
  related_invoice_ref?: string | null;
  contact_id?: string | null;
  create_new_contact?: ContactInput | null;
  create_linked_record: boolean;
  notes?: string | null;
}