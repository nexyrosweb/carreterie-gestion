use chrono::Utc;
use rusqlite::params;
use rust_decimal::Decimal;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::settings::{AppSettings, AppSettingsPatch};
use crate::money::{parse_decimal_amount, round_eur};
use crate::services::audit::AuditService;

pub struct SettingsService;

impl SettingsService {
    fn get_key(conn: &rusqlite::Connection, key: &str) -> AppResult<String> {
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .map_err(|_| AppError::interne(format!("Paramètre manquant : {key}")))
    }

    fn get_key_or(conn: &rusqlite::Connection, key: &str, default: &str) -> String {
        Self::get_key(conn, key).unwrap_or_else(|_| default.to_string())
    }

    fn set_key(conn: &mut rusqlite::Connection, key: &str, value: &str) -> AppResult<()> {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn load(db: &DbPool) -> AppResult<AppSettings> {
        db.with_conn(|conn| {
            let default_tax = parse_decimal_amount(&Self::get_key(conn, "default_tax_rate")?)?;
            let smtp_enabled = Self::get_key_or(conn, "smtp_enabled", "0") == "1";
            let smtp_port: u16 = Self::get_key_or(conn, "smtp_port", "587")
                .parse()
                .unwrap_or(587);
            let smtp_timeout: u32 = Self::get_key_or(conn, "smtp_timeout_secs", "30")
                .parse()
                .unwrap_or(30);
            Ok(AppSettings {
                theatre_name: Self::get_key(conn, "theatre_name")?,
                theatre_address: Self::get_key(conn, "theatre_address")?,
                theatre_siret: Self::get_key(conn, "theatre_siret")?,
                theatre_vat_number: Self::get_key(conn, "theatre_vat_number")?,
                logo_path: Self::get_key(conn, "logo_path")?,
                invoice_prefix: Self::get_key(conn, "invoice_prefix")?,
                credit_note_prefix: Self::get_key(conn, "credit_note_prefix")?,
                invoice_year: Self::get_key(conn, "invoice_year")?,
                invoice_last_sequence: Self::get_key(conn, "invoice_last_sequence")?
                    .parse()
                    .unwrap_or(0),
                credit_note_last_sequence: Self::get_key(conn, "credit_note_last_sequence")?
                    .parse()
                    .unwrap_or(0),
                legal_mentions: Self::get_key(conn, "legal_mentions")?,
                default_tax_rate: default_tax,
                export_folder: Self::get_key(conn, "export_folder")?,
                smtp_enabled,
                smtp_host: Self::get_key_or(conn, "smtp_host", ""),
                smtp_port,
                smtp_encryption: Self::get_key_or(conn, "smtp_encryption", "starttls"),
                smtp_username: Self::get_key_or(conn, "smtp_username", ""),
                smtp_password: Self::get_key_or(conn, "smtp_password", ""),
                smtp_from_name: Self::get_key_or(conn, "smtp_from_name", ""),
                smtp_from_email: Self::get_key_or(conn, "smtp_from_email", ""),
                smtp_reply_to: Self::get_key_or(conn, "smtp_reply_to", ""),
                smtp_cc: Self::get_key_or(conn, "smtp_cc", ""),
                smtp_bcc: Self::get_key_or(conn, "smtp_bcc", ""),
                smtp_timeout_secs: smtp_timeout,
            })
        })
    }

    pub fn update(db: &DbPool, user_id: &str, patch: AppSettingsPatch) -> AppResult<AppSettings> {
        db.with_conn_mut(|conn| {
            if let Some(v) = patch.theatre_name {
                Self::set_key(conn, "theatre_name", &v)?;
            }
            if let Some(v) = patch.theatre_address {
                Self::set_key(conn, "theatre_address", &v)?;
            }
            if let Some(v) = patch.theatre_siret {
                Self::set_key(conn, "theatre_siret", &v)?;
            }
            if let Some(v) = patch.theatre_vat_number {
                Self::set_key(conn, "theatre_vat_number", &v)?;
            }
            if let Some(v) = patch.logo_path {
                Self::set_key(conn, "logo_path", &v)?;
            }
            if let Some(v) = patch.invoice_prefix {
                Self::set_key(conn, "invoice_prefix", &v)?;
            }
            if let Some(v) = patch.credit_note_prefix {
                Self::set_key(conn, "credit_note_prefix", &v)?;
            }
            if let Some(v) = patch.legal_mentions {
                Self::set_key(conn, "legal_mentions", &v)?;
            }
            if let Some(v) = patch.default_tax_rate {
                Self::set_key(conn, "default_tax_rate", &format!("{:.2}", round_eur(v)))?;
            }
            if let Some(v) = patch.export_folder {
                Self::set_key(conn, "export_folder", &v)?;
            }
            if let Some(v) = patch.smtp_enabled {
                Self::set_key(conn, "smtp_enabled", if v { "1" } else { "0" })?;
            }
            if let Some(v) = patch.smtp_host {
                Self::set_key(conn, "smtp_host", &v)?;
            }
            if let Some(v) = patch.smtp_port {
                Self::set_key(conn, "smtp_port", &v.to_string())?;
            }
            if let Some(v) = patch.smtp_encryption {
                Self::set_key(conn, "smtp_encryption", &v)?;
            }
            if let Some(v) = patch.smtp_username {
                Self::set_key(conn, "smtp_username", &v)?;
            }
            if let Some(v) = patch.smtp_password {
                Self::set_key(conn, "smtp_password", &v)?;
            }
            if let Some(v) = patch.smtp_from_name {
                Self::set_key(conn, "smtp_from_name", &v)?;
            }
            if let Some(v) = patch.smtp_from_email {
                Self::set_key(conn, "smtp_from_email", &v)?;
            }
            if let Some(v) = patch.smtp_reply_to {
                Self::set_key(conn, "smtp_reply_to", &v)?;
            }
            if let Some(v) = patch.smtp_cc {
                Self::set_key(conn, "smtp_cc", &v)?;
            }
            if let Some(v) = patch.smtp_bcc {
                Self::set_key(conn, "smtp_bcc", &v)?;
            }
            if let Some(v) = patch.smtp_timeout_secs {
                Self::set_key(conn, "smtp_timeout_secs", &v.to_string())?;
            }
            Ok(())
        })?;
        AuditService::log(
            db,
            user_id,
            "parametres_modifies",
            Some("settings"),
            None,
            None,
        )?;
        Self::load(db)
    }

    pub fn next_invoice_number(conn: &mut rusqlite::Connection, is_credit: bool) -> AppResult<String> {
        let year = Utc::now().format("%Y").to_string();
        let stored_year = Self::get_key(conn, "invoice_year")?;
        if stored_year != year {
            Self::set_key(conn, "invoice_year", &year)?;
            Self::set_key(conn, "invoice_last_sequence", "0")?;
            Self::set_key(conn, "credit_note_last_sequence", "0")?;
        }

        let (seq_key, prefix) = if is_credit {
            (
                "credit_note_last_sequence",
                Self::get_key(conn, "credit_note_prefix")?,
            )
        } else {
            (
                "invoice_last_sequence",
                Self::get_key(conn, "invoice_prefix")?,
            )
        };

        let mut seq: u32 = Self::get_key(conn, seq_key)?.parse().unwrap_or(0);
        seq += 1;
        Self::set_key(conn, seq_key, &seq.to_string())?;

        Ok(format!("{}{year}-{seq:04}", prefix))
    }

    pub fn get_default_tax(db: &DbPool) -> AppResult<Decimal> {
        Ok(Self::load(db)?.default_tax_rate)
    }
}
