use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContactKind {
    Client,
    Fournisseur,
    LesDeux,
}

impl ContactKind {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            ContactKind::Client => "client",
            ContactKind::Fournisseur => "fournisseur",
            ContactKind::LesDeux => "les_deux",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "client" => Some(ContactKind::Client),
            "fournisseur" => Some(ContactKind::Fournisseur),
            "les_deux" => Some(ContactKind::LesDeux),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub kind: ContactKind,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
    pub country: String,
    pub siret: Option<String>,
    pub vat_number: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContactInput {
    pub kind: ContactKind,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub siret: Option<String>,
    pub vat_number: Option<String>,
    pub notes: Option<String>,
}