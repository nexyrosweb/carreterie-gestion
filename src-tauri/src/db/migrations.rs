use rusqlite::Connection;

use crate::error::{AppError, AppResult};

const MIGRATION_V1: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS users (
    id TEXT NOT NULL PRIMARY KEY,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions_meta (
    key TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS login_attempts (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL COLLATE NOCASE,
    failed_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_login_attempts_user_time ON login_attempts(username, failed_at);

CREATE TABLE IF NOT EXISTS contacts (
    id TEXT NOT NULL PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('client', 'fournisseur', 'les_deux')),
    name TEXT NOT NULL,
    email TEXT,
    phone TEXT,
    address_line1 TEXT,
    address_line2 TEXT,
    postal_code TEXT,
    city TEXT,
    country TEXT NOT NULL DEFAULT 'France',
    siret TEXT,
    vat_number TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS invoices (
    id TEXT NOT NULL PRIMARY KEY,
    number TEXT UNIQUE,
    contact_id TEXT NOT NULL REFERENCES contacts(id),
    status TEXT NOT NULL CHECK (status IN ('brouillon', 'emise', 'avoir')),
    invoice_kind TEXT NOT NULL DEFAULT 'facture' CHECK (invoice_kind IN ('facture', 'avoir')),
    related_invoice_id TEXT REFERENCES invoices(id),
    issue_date TEXT,
    due_date TEXT,
    customer_notes TEXT,
    total_ht TEXT NOT NULL DEFAULT '0.00',
    total_tva TEXT NOT NULL DEFAULT '0.00',
    total_ttc TEXT NOT NULL DEFAULT '0.00',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    issued_at TEXT,
    CHECK (
        (status = 'brouillon' AND number IS NULL)
        OR (status IN ('emise', 'avoir') AND number IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_invoices_contact ON invoices(contact_id);
CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices(status);
CREATE INDEX IF NOT EXISTS idx_invoices_number ON invoices(number);

CREATE TABLE IF NOT EXISTS invoice_lines (
    id TEXT NOT NULL PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    description TEXT NOT NULL,
    quantity TEXT NOT NULL,
    unit_price_ht TEXT NOT NULL,
    tax_rate_percent TEXT NOT NULL,
    line_ht TEXT NOT NULL,
    line_tva TEXT NOT NULL,
    line_ttc TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS supplier_invoices (
    id TEXT NOT NULL PRIMARY KEY,
    supplier_contact_id TEXT REFERENCES contacts(id),
    reference TEXT,
    invoice_date TEXT,
    amount_ht TEXT,
    amount_ttc TEXT,
    tax_rate_percent TEXT,
    notes TEXT,
    attachment_path TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    user_id TEXT,
    action TEXT NOT NULL,
    entity_type TEXT,
    entity_id TEXT,
    details TEXT
);

INSERT OR IGNORE INTO schema_version (version) VALUES (1);

INSERT OR IGNORE INTO settings (key, value) VALUES ('theatre_name', 'Théâtre de la Carreterie');
INSERT OR IGNORE INTO settings (key, value) VALUES ('theatre_address', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('theatre_siret', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('theatre_vat_number', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('logo_path', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('invoice_prefix', 'FA-');
INSERT OR IGNORE INTO settings (key, value) VALUES ('credit_note_prefix', 'AV-');
INSERT OR IGNORE INTO settings (key, value) VALUES ('invoice_year', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('invoice_last_sequence', '0');
INSERT OR IGNORE INTO settings (key, value) VALUES ('credit_note_last_sequence', '0');
INSERT OR IGNORE INTO settings (key, value) VALUES ('legal_mentions', 'TVA non applicable, art. 293 B du CGI — mentions légales à compléter dans les paramètres.');
INSERT OR IGNORE INTO settings (key, value) VALUES ('default_tax_rate', '20.00');
INSERT OR IGNORE INTO settings (key, value) VALUES ('export_folder', '');
"#;

const MIGRATION_V2: &str = r#"
ALTER TABLE invoices ADD COLUMN amount_paid TEXT NOT NULL DEFAULT '0.00';
ALTER TABLE invoices ADD COLUMN payment_status TEXT NOT NULL DEFAULT 'impayee';
ALTER TABLE supplier_invoices ADD COLUMN due_date TEXT;
ALTER TABLE supplier_invoices ADD COLUMN payment_status TEXT NOT NULL DEFAULT 'impayee';

CREATE TABLE IF NOT EXISTS invoice_payments (
    id TEXT NOT NULL PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    amount TEXT NOT NULL,
    paid_at TEXT NOT NULL,
    method TEXT,
    reference TEXT,
    notes TEXT,
    created_at TEXT NOT NULL
);

INSERT OR IGNORE INTO schema_version (version) VALUES (2);
"#;

const MIGRATION_V3: &str = r#"
CREATE TABLE IF NOT EXISTS imported_documents (
    id TEXT PRIMARY KEY,
    file_hash TEXT NOT NULL UNIQUE,
    original_filename TEXT NOT NULL,
    stored_path TEXT NOT NULL,
    direction TEXT NOT NULL CHECK (direction IN ('client','fournisseur')),
    document_type TEXT NOT NULL CHECK (document_type IN ('facture','acompte','avoir','inconnu')),
    status TEXT NOT NULL CHECK (status IN ('en_attente','valide','rejete','doublon')),
    extraction_source TEXT NOT NULL CHECK (extraction_source IN ('texte','ocr','manuel','aucun')),
    issuer_name TEXT,
    party_name TEXT,
    party_address TEXT,
    siren TEXT,
    siret TEXT,
    invoice_number TEXT,
    issue_date TEXT,
    payment_date TEXT,
    payment_method TEXT,
    payment_status TEXT,
    description TEXT,
    total_ht TEXT,
    tax_rate TEXT,
    total_tva TEXT,
    total_ttc TEXT,
    related_invoice_ref TEXT,
    contact_id TEXT REFERENCES contacts(id),
    linked_draft_invoice_id TEXT,
    linked_supplier_invoice_id TEXT,
    uncertain_fields TEXT,
    missing_fields TEXT,
    amounts_coherent INTEGER,
    raw_text_excerpt TEXT,
    ocr_available INTEGER NOT NULL DEFAULT 0,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    validated_at TEXT,
    validated_by TEXT
);
CREATE INDEX IF NOT EXISTS idx_imported_hash ON imported_documents(file_hash);
CREATE INDEX IF NOT EXISTS idx_imported_status ON imported_documents(status);
CREATE INDEX IF NOT EXISTS idx_imported_contact ON imported_documents(contact_id);
CREATE INDEX IF NOT EXISTS idx_imported_number ON imported_documents(invoice_number);

INSERT OR IGNORE INTO schema_version (version) VALUES (3);
"#;

const MIGRATION_V4: &str = r#"
ALTER TABLE users ADD COLUMN email TEXT NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN first_name TEXT NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN last_name TEXT NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN avatar_path TEXT NOT NULL DEFAULT '';

INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_enabled', '0');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_host', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_port', '587');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_encryption', 'starttls');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_username', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_password', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_from_name', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_from_email', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_reply_to', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_cc', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_bcc', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('smtp_timeout_secs', '30');

INSERT OR IGNORE INTO schema_version (version) VALUES (4);
"#;

const MIGRATION_V5: &str = r#"
PRAGMA foreign_keys=OFF;

CREATE TABLE invoices_v5 (
    id TEXT NOT NULL PRIMARY KEY,
    number TEXT UNIQUE,
    contact_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('brouillon', 'emise', 'avoir')),
    invoice_kind TEXT NOT NULL DEFAULT 'facture' CHECK (invoice_kind IN ('facture', 'avoir', 'devis')),
    related_invoice_id TEXT,
    issue_date TEXT,
    due_date TEXT,
    customer_notes TEXT,
    total_ht TEXT NOT NULL DEFAULT '0.00',
    total_tva TEXT NOT NULL DEFAULT '0.00',
    total_ttc TEXT NOT NULL DEFAULT '0.00',
    amount_paid TEXT NOT NULL DEFAULT '0.00',
    payment_status TEXT NOT NULL DEFAULT 'impayee',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    issued_at TEXT,
    CHECK (
        (status = 'brouillon' AND number IS NULL)
        OR (status IN ('emise', 'avoir') AND number IS NOT NULL)
    )
);

INSERT INTO invoices_v5 (
    id, number, contact_id, status, invoice_kind, related_invoice_id,
    issue_date, due_date, customer_notes, total_ht, total_tva, total_ttc,
    amount_paid, payment_status, created_at, updated_at, issued_at
)
SELECT
    id, number, contact_id, status, invoice_kind, related_invoice_id,
    issue_date, due_date, customer_notes, total_ht, total_tva, total_ttc,
    COALESCE(amount_paid, '0.00'), COALESCE(payment_status, 'impayee'),
    created_at, updated_at, issued_at
FROM invoices;

DROP TABLE invoices;
ALTER TABLE invoices_v5 RENAME TO invoices;

CREATE INDEX IF NOT EXISTS idx_invoices_contact ON invoices(contact_id);
CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices(status);
CREATE INDEX IF NOT EXISTS idx_invoices_number ON invoices(number);
CREATE INDEX IF NOT EXISTS idx_invoices_kind ON invoices(invoice_kind);

INSERT OR IGNORE INTO settings (key, value) VALUES ('devis_prefix', 'DV-');
INSERT OR IGNORE INTO settings (key, value) VALUES ('devis_last_sequence', '0');
INSERT OR IGNORE INTO settings (key, value) VALUES ('bank_iban', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('bank_account', '');

INSERT OR IGNORE INTO schema_version (version) VALUES (5);

PRAGMA foreign_keys=ON;
"#;

const MIGRATION_V6: &str = r#"
ALTER TABLE invoices ADD COLUMN accepted_at TEXT;
ALTER TABLE invoices ADD COLUMN rental_start TEXT;
ALTER TABLE invoices ADD COLUMN rental_end TEXT;

ALTER TABLE invoice_lines ADD COLUMN line_kind TEXT NOT NULL DEFAULT 'standard';
ALTER TABLE invoice_payments ADD COLUMN payment_type TEXT NOT NULL DEFAULT 'reglement';

INSERT OR IGNORE INTO settings (key, value) VALUES ('pdf_primary_color', '#8B1E1E');
INSERT OR IGNORE INTO settings (key, value) VALUES ('pdf_header_subtitle', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('pdf_use_custom_logo', '0');

CREATE INDEX IF NOT EXISTS idx_invoices_related ON invoices(related_invoice_id);

INSERT OR IGNORE INTO schema_version (version) VALUES (6);
"#;

pub fn run_migrations(conn: &Connection) -> AppResult<()> {
    let version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if version < 1 {
        // schema_version table may not exist yet
        let has_schema = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_version'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_schema {
            conn.execute_batch(MIGRATION_V1)
                .map_err(AppError::BaseDeDonnees)?;
        }
    }

    let version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if version < 2 {
        conn.execute_batch(MIGRATION_V2)
            .map_err(AppError::BaseDeDonnees)?;
    }

    let version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if version < 3 {
        conn.execute_batch(MIGRATION_V3)
            .map_err(AppError::BaseDeDonnees)?;
    }

    let version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if version < 4 {
        conn.execute_batch(MIGRATION_V4)
            .map_err(AppError::BaseDeDonnees)?;
    }

    let version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if version < 5 {
        conn.execute_batch(MIGRATION_V5)
            .map_err(AppError::BaseDeDonnees)?;
    }

    let version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if version < 6 {
        conn.execute_batch(MIGRATION_V6)
            .map_err(AppError::BaseDeDonnees)?;
    }

    Ok(())
}