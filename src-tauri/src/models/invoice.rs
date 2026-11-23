use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Brouillon,
    Emise,
    Avoir,
}

impl InvoiceStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            InvoiceStatus::Brouillon => "brouillon",
            InvoiceStatus::Emise => "emise",
            InvoiceStatus::Avoir => "avoir",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "brouillon" => Some(InvoiceStatus::Brouillon),
            "emise" => Some(InvoiceStatus::Emise),
            "avoir" => Some(InvoiceStatus::Avoir),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceKind {
    Facture,
    Avoir,
    Devis,
}

impl InvoiceKind {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            InvoiceKind::Facture => "facture",
            InvoiceKind::Avoir => "avoir",
            InvoiceKind::Devis => "devis",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "facture" => Some(InvoiceKind::Facture),
            "avoir" => Some(InvoiceKind::Avoir),
            "devis" => Some(InvoiceKind::Devis),
            _ => None,
        }
    }

    pub fn document_title(&self) -> &'static str {
        match self {
            InvoiceKind::Facture => "Facture",
            InvoiceKind::Avoir => "Avoir",
            InvoiceKind::Devis => "Devis",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceLine {
    pub id: String,
    pub invoice_id: String,
    pub position: i32,
    pub description: String,
    pub quantity: Decimal,
    pub unit_price_ht: Decimal,
    pub tax_rate_percent: Decimal,
    pub line_ht: Decimal,
    pub line_tva: Decimal,
    pub line_ttc: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub number: Option<String>,
    pub contact_id: String,
    pub status: InvoiceStatus,
    pub invoice_kind: InvoiceKind,
    pub related_invoice_id: Option<String>,
    pub issue_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub customer_notes: Option<String>,
    pub total_ht: Decimal,
    pub total_tva: Decimal,
    pub total_ttc: Decimal,
    pub amount_paid: Decimal,
    pub payment_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub issued_at: Option<DateTime<Utc>>,
    pub lines: Vec<InvoiceLine>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecordPaymentInput {
    pub invoice_id: String,
    pub amount: Decimal,
    pub paid_at: NaiveDate,
    pub method: Option<String>,
    pub reference: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvoiceLineInput {
    pub description: String,
    pub quantity: Decimal,
    pub unit_price_ht: Decimal,
    pub tax_rate_percent: Decimal,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvoiceDraftInput {
    pub contact_id: String,
    pub issue_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub customer_notes: Option<String>,
    pub lines: Vec<InvoiceLineInput>,
    /// `facture` (défaut) ou `devis`.
    #[serde(default)]
    pub invoice_kind: Option<InvoiceKind>,
}

impl Default for InvoiceDraftInput {
    fn default() -> Self {
        Self {
            contact_id: String::new(),
            issue_date: None,
            due_date: None,
            customer_notes: None,
            lines: Vec::new(),
            invoice_kind: None,
        }
    }
}