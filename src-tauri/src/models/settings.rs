use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub theatre_name: String,
    pub theatre_address: String,
    pub theatre_siret: String,
    pub theatre_vat_number: String,
    pub logo_path: String,
    pub invoice_prefix: String,
    pub credit_note_prefix: String,
    pub invoice_year: String,
    pub invoice_last_sequence: u32,
    pub credit_note_last_sequence: u32,
    pub legal_mentions: String,
    pub default_tax_rate: Decimal,
    pub export_folder: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppSettingsPatch {
    pub theatre_name: Option<String>,
    pub theatre_address: Option<String>,
    pub theatre_siret: Option<String>,
    pub theatre_vat_number: Option<String>,
    pub logo_path: Option<String>,
    pub invoice_prefix: Option<String>,
    pub credit_note_prefix: Option<String>,
    pub legal_mentions: Option<String>,
    pub default_tax_rate: Option<Decimal>,
    pub export_folder: Option<String>,
}