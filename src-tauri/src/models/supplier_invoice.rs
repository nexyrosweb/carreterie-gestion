use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplierInvoice {
    pub id: String,
    pub supplier_contact_id: Option<String>,
    pub reference: Option<String>,
    pub invoice_date: Option<NaiveDate>,
    pub amount_ht: Option<Decimal>,
    pub amount_ttc: Option<Decimal>,
    pub tax_rate_percent: Option<Decimal>,
    pub notes: Option<String>,
    pub attachment_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupplierInvoiceInput {
    pub supplier_contact_id: Option<String>,
    pub reference: Option<String>,
    pub invoice_date: Option<NaiveDate>,
    pub amount_ht: Option<Decimal>,
    pub amount_ttc: Option<Decimal>,
    pub tax_rate_percent: Option<Decimal>,
    pub notes: Option<String>,
    pub attachment_path: Option<String>,
}