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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceLineKind {
    #[default]
    Standard,
    LocationSalle,
}

impl InvoiceLineKind {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            InvoiceLineKind::Standard => "standard",
            InvoiceLineKind::LocationSalle => "location_salle",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "location_salle" => InvoiceLineKind::LocationSalle,
            _ => InvoiceLineKind::Standard,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaymentType {
    #[default]
    Reglement,
    Acompte,
    Solde,
}

impl PaymentType {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            PaymentType::Reglement => "reglement",
            PaymentType::Acompte => "acompte",
            PaymentType::Solde => "solde",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "acompte" => PaymentType::Acompte,
            "solde" => PaymentType::Solde,
            _ => PaymentType::Reglement,
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
    #[serde(default)]
    pub line_kind: InvoiceLineKind,
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
    #[serde(default)]
    pub accepted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub rental_start: Option<String>,
    #[serde(default)]
    pub rental_end: Option<String>,
    pub lines: Vec<InvoiceLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoicePayment {
    pub id: String,
    pub invoice_id: String,
    pub amount: Decimal,
    pub paid_at: NaiveDate,
    pub method: Option<String>,
    pub reference: Option<String>,
    pub notes: Option<String>,
    pub payment_type: PaymentType,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecordPaymentInput {
    pub invoice_id: String,
    pub amount: Decimal,
    pub paid_at: NaiveDate,
    pub method: Option<String>,
    pub reference: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub payment_type: Option<PaymentType>,
}

impl Default for RecordPaymentInput {
    fn default() -> Self {
        Self {
            invoice_id: String::new(),
            amount: Decimal::ZERO,
            paid_at: chrono::Utc::now().date_naive(),
            method: None,
            reference: None,
            notes: None,
            payment_type: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvoiceLineInput {
    pub description: String,
    pub quantity: Decimal,
    pub unit_price_ht: Decimal,
    pub tax_rate_percent: Decimal,
    #[serde(default)]
    pub line_kind: Option<InvoiceLineKind>,
}

impl Default for InvoiceLineInput {
    fn default() -> Self {
        Self {
            description: String::new(),
            quantity: Decimal::ZERO,
            unit_price_ht: Decimal::ZERO,
            tax_rate_percent: Decimal::ZERO,
            line_kind: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvoiceDraftInput {
    pub contact_id: String,
    pub issue_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub customer_notes: Option<String>,
    pub lines: Vec<InvoiceLineInput>,
    #[serde(default)]
    pub invoice_kind: Option<InvoiceKind>,
    #[serde(default)]
    pub related_invoice_id: Option<String>,
    #[serde(default)]
    pub rental_start: Option<String>,
    #[serde(default)]
    pub rental_end: Option<String>,
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
            related_invoice_id: None,
            rental_start: None,
            rental_end: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConvertDevisInput {
    pub devis_id: String,
    #[serde(default)]
    pub issue_immediately: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendDocumentEmailInput {
    pub invoice_id: String,
    pub to_email: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
}
