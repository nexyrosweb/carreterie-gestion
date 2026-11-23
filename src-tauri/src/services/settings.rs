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