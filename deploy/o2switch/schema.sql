-- Schéma Carreterie Gestion pour MySQL/MariaDB (o2switch)
-- Charset recommandé : utf8mb4

SET NAMES utf8mb4;
SET FOREIGN_KEY_CHECKS = 0;

CREATE TABLE IF NOT EXISTS schema_version (
    version INT NOT NULL PRIMARY KEY
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS users (
    id VARCHAR(36) NOT NULL PRIMARY KEY,
    username VARCHAR(191) NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    email TEXT NOT NULL DEFAULT '',
    first_name TEXT NOT NULL DEFAULT '',
    last_name TEXT NOT NULL DEFAULT '',
    avatar_path TEXT NOT NULL DEFAULT '',
    UNIQUE KEY uq_users_username (username)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS sessions_meta (
    `key` VARCHAR(191) NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS login_attempts (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(191) NOT NULL,
    failed_at TEXT NOT NULL,
    KEY idx_login_attempts_user_time (username, failed_at(32))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS contacts (
    id VARCHAR(36) NOT NULL PRIMARY KEY,
    kind VARCHAR(32) NOT NULL,
    name TEXT NOT NULL,
    email TEXT,
    phone TEXT,
    address_line1 TEXT,
    address_line2 TEXT,
    postal_code TEXT,
    city TEXT,
    country TEXT NOT NULL DEFAULT 'France',
    siret VARCHAR(32),
    vat_number TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    KEY idx_contacts_siret (siret)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS invoices (
    id VARCHAR(36) NOT NULL PRIMARY KEY,
    number VARCHAR(64) UNIQUE,
    contact_id VARCHAR(36) NOT NULL,
    status VARCHAR(32) NOT NULL,
    invoice_kind VARCHAR(32) NOT NULL DEFAULT 'facture',
    related_invoice_id VARCHAR(36),
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
    accepted_at TEXT,
    rental_start TEXT,
    rental_end TEXT,
    KEY idx_invoices_contact (contact_id),
    KEY idx_invoices_status (status),
    KEY idx_invoices_number (number),
    KEY idx_invoices_kind (invoice_kind),
    KEY idx_invoices_related (related_invoice_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS invoice_lines (
    id VARCHAR(36) NOT NULL PRIMARY KEY,
    invoice_id VARCHAR(36) NOT NULL,
    position INT NOT NULL,
    description TEXT NOT NULL,
    quantity TEXT NOT NULL,
    unit_price_ht TEXT NOT NULL,
    tax_rate_percent TEXT NOT NULL,
    line_ht TEXT NOT NULL,
    line_tva TEXT NOT NULL,
    line_ttc TEXT NOT NULL,
    line_kind TEXT NOT NULL DEFAULT 'standard',
    KEY idx_invoice_lines_invoice (invoice_id),
    CONSTRAINT fk_invoice_lines_invoice FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS invoice_payments (
    id VARCHAR(36) NOT NULL PRIMARY KEY,
    invoice_id VARCHAR(36) NOT NULL,
    amount TEXT NOT NULL,
    paid_at TEXT NOT NULL,
    method TEXT,
    reference TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    payment_type TEXT NOT NULL DEFAULT 'reglement',
    KEY idx_invoice_payments_invoice (invoice_id),
    CONSTRAINT fk_invoice_payments_invoice FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS supplier_invoices (
    id VARCHAR(36) NOT NULL PRIMARY KEY,
    supplier_contact_id VARCHAR(36),
    reference TEXT,
    invoice_date TEXT,
    amount_ht TEXT,
    amount_ttc TEXT,
    tax_rate_percent TEXT,
    notes TEXT,
    attachment_path TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    due_date TEXT,
    payment_status TEXT NOT NULL DEFAULT 'impayee'
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS settings (
    `key` VARCHAR(191) NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS audit_log (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    created_at TEXT NOT NULL,
    user_id TEXT,
    action TEXT NOT NULL,
    entity_type TEXT,
    entity_id TEXT,
    details TEXT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS imported_documents (
    id VARCHAR(36) NOT NULL PRIMARY KEY,
    file_hash VARCHAR(128) NOT NULL,
    original_filename TEXT NOT NULL,
    stored_path TEXT NOT NULL,
    direction VARCHAR(32) NOT NULL,
    document_type VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL,
    extraction_source VARCHAR(32) NOT NULL,
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
    contact_id VARCHAR(36),
    linked_draft_invoice_id TEXT,
    linked_supplier_invoice_id TEXT,
    uncertain_fields TEXT,
    missing_fields TEXT,
    amounts_coherent INT,
    raw_text_excerpt TEXT,
    ocr_available INT NOT NULL DEFAULT 0,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    validated_at TEXT,
    validated_by TEXT,
    UNIQUE KEY uq_imported_hash (file_hash),
    KEY idx_imported_status (status),
    KEY idx_imported_contact (contact_id),
    KEY idx_imported_number (invoice_number(64))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Fichiers partagés (PDF, logos, imports) — cache local dérivé sur chaque PC
CREATE TABLE IF NOT EXISTS file_blobs (
    path VARCHAR(512) NOT NULL PRIMARY KEY,
    content LONGBLOB NOT NULL,
    size_bytes BIGINT NOT NULL,
    content_type VARCHAR(128) NOT NULL DEFAULT 'application/octet-stream',
    updated_at TEXT NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT IGNORE INTO schema_version (version) VALUES (7);

INSERT IGNORE INTO settings (`key`, value) VALUES ('theatre_name', 'Théâtre de la Carreterie');
INSERT IGNORE INTO settings (`key`, value) VALUES ('theatre_address', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('theatre_siret', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('theatre_vat_number', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('logo_path', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('invoice_prefix', 'FA-');
INSERT IGNORE INTO settings (`key`, value) VALUES ('credit_note_prefix', 'AV-');
INSERT IGNORE INTO settings (`key`, value) VALUES ('invoice_year', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('invoice_last_sequence', '0');
INSERT IGNORE INTO settings (`key`, value) VALUES ('credit_note_last_sequence', '0');
INSERT IGNORE INTO settings (`key`, value) VALUES ('legal_mentions', 'TVA non applicable, art. 293 B du CGI — mentions légales à compléter dans les paramètres.');
INSERT IGNORE INTO settings (`key`, value) VALUES ('default_tax_rate', '20.00');
INSERT IGNORE INTO settings (`key`, value) VALUES ('export_folder', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_enabled', '0');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_host', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_port', '587');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_encryption', 'starttls');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_username', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_password', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_from_name', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_from_email', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_reply_to', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_cc', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_bcc', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('smtp_timeout_secs', '30');
INSERT IGNORE INTO settings (`key`, value) VALUES ('devis_prefix', 'DV-');
INSERT IGNORE INTO settings (`key`, value) VALUES ('devis_last_sequence', '0');
INSERT IGNORE INTO settings (`key`, value) VALUES ('bank_iban', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('bank_account', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('bank_bic', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('bank_name', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('bank_holder', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('pdf_primary_color', '#8B1E1E');
INSERT IGNORE INTO settings (`key`, value) VALUES ('pdf_header_subtitle', '');
INSERT IGNORE INTO settings (`key`, value) VALUES ('pdf_use_custom_logo', '0');
INSERT IGNORE INTO settings (`key`, value) VALUES ('pdf_template', 'classique');

SET FOREIGN_KEY_CHECKS = 1;
