//! Environnement de démonstration isolé (demo.db).

use chrono::{Duration, NaiveDate, Utc};
use rusqlite::params;
use rust_decimal_macros::dec;
use uuid::Uuid;

use crate::auth::{hash_password, SessionUser};
use crate::db::{run_migrations, DbPool};
use crate::error::{AppError, AppResult};
use crate::models::contact::{ContactInput, ContactKind};
use crate::models::invoice::{InvoiceDraftInput, InvoiceLineInput};
use crate::models::supplier_invoice::SupplierInvoiceInput;
use crate::paths::{self, demo_database_path};
use crate::services::contacts::ContactsService;
use crate::services::invoices::InvoicesService;
use crate::services::settings::SettingsService;
use crate::services::suppliers::SuppliersService;
use crate::state::AppState;

pub const DEMO_USERNAME: &str = "demo";

pub struct DemoService;

impl DemoService {
    pub fn open_and_prepare() -> AppResult<DbPool> {
        let pool = DbPool::open_demo()?;
        pool.with_conn_mut(|conn| run_migrations(conn))?;
        paths::with_data_mode(true, || -> AppResult<()> {
            Self::ensure_demo_user(&pool)?;
            if Self::needs_seed(&pool)? {
                Self::seed(&pool)?;
            }
            Ok(())
        })?;
        Ok(pool)
    }

    pub fn enter(state: &AppState) -> AppResult<crate::state::SessionRecord> {
        let user = paths::with_data_mode(true, || {
            state.with_demo_pool(|db| {
                Self::ensure_demo_user(db)?;
                if Self::needs_seed(db)? {
                    Self::seed(db)?;
                }
                Self::demo_session_user(db)
            })
        })?;

        Ok(state.sessions.create_session(user))
    }

    pub fn reset(state: &AppState) -> AppResult<()> {
        state.sessions.invalidate_demo_sessions();

        let old = {
            let mut guard = state.demo_db.lock();
            // Remplace temporairement pour libérer le fichier sous Windows.
            let placeholder = DbPool::open_at(":memory:")?;
            std::mem::replace(&mut *guard, placeholder)
        };
        old.checkpoint_and_close()?;

        let path = demo_database_path()?;
        for suffix in ["", "-wal", "-shm"] {
            let p = if suffix.is_empty() {
                path.clone()
            } else {
                PathBufExt(&path).with_suffix(suffix)
            };
            let _ = std::fs::remove_file(&p);
        }
        // Nettoyer pièces jointes démo (pas la prod).
        paths::with_data_mode(true, || {
            if let Ok(att) = paths::attachments_dir() {
                let _ = std::fs::remove_dir_all(&att);
                let _ = std::fs::create_dir_all(att.join("imports"));
                let _ = std::fs::create_dir_all(att.join("pdf"));
            }
            Ok::<(), AppError>(())
        })?;

        let fresh = Self::open_and_prepare()?;
        *state.demo_db.lock() = fresh;
        Ok(())
    }

    fn needs_seed(db: &DbPool) -> AppResult<bool> {
        db.with_conn(|conn| {
            let n: i64 = conn.query_row("SELECT COUNT(*) FROM contacts", [], |r| r.get(0))?;
            Ok(n == 0)
        })
    }

    fn ensure_demo_user(db: &DbPool) -> AppResult<()> {
        let exists: bool = db.with_conn(|conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM users WHERE username = ?1 COLLATE NOCASE",
                params![DEMO_USERNAME],
                |r| r.get(0),
            )?;
            Ok(n > 0)
        })?;
        if exists {
            return Ok(());
        }
        // Mot de passe aléatoire non documenté : entrée démo sans mot de passe.
        let hash = hash_password(&format!("DemoLocal-{}!", Uuid::new_v4()))?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        db.with_conn_mut(|conn| {
            conn.execute(
                "INSERT INTO users (id, username, password_hash, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, DEMO_USERNAME, hash, now, now],
            )?;
            Ok(())
        })
    }

    fn demo_session_user(db: &DbPool) -> AppResult<SessionUser> {
        db.with_conn(|conn| {
            let (id, username): (String, String) = conn.query_row(
                "SELECT id, username FROM users WHERE username = ?1 COLLATE NOCASE",
                params![DEMO_USERNAME],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            Ok(SessionUser {
                id,
                username,
                is_demo: true,
            })
        })
    }

    /// Jeu de données fictif — aucune donnée réelle, aucun RIB.
    pub fn seed(db: &DbPool) -> AppResult<()> {
        Self::ensure_demo_user(db)?;
        let user = Self::demo_session_user(db)?;

        let mut settings = SettingsService::load(db)?;
        settings.theatre_name = "Théâtre Démo Carreterie".into();
        settings.theatre_address = "12 rue des Exemples\n84000 Avignon".into();
        settings.theatre_siret = "00000000000000".into();
        settings.theatre_vat_number = "FR00000000000".into();
        settings.legal_mentions =
            "DÉMONSTRATION — SANS VALEUR COMPTABLE. Données fictives Nexyros.".into();
        SettingsService::update(
            db,
            &user.id,
            crate::models::settings::AppSettingsPatch {
                theatre_name: Some(settings.theatre_name),
                theatre_address: Some(settings.theatre_address),
                theatre_siret: Some(settings.theatre_siret),
                theatre_vat_number: Some(settings.theatre_vat_number),
                invoice_prefix: Some("DEMO-".into()),
                credit_note_prefix: Some("DEMO-AV-".into()),
                legal_mentions: Some(settings.legal_mentions),
                default_tax_rate: Some(dec!(20)),
                ..Default::default()
            },
        )?;

        let client_a = ContactsService::create(
            db,
            &user.id,
            ContactInput {
                kind: ContactKind::Client,
                name: "Association Exemple Culture".into(),
                email: Some("contact@exemple-culture.demo".into()),
                phone: Some("04 00 00 00 01".into()),
                address_line1: Some("1 place de la Démo".into()),
                address_line2: None,
                postal_code: Some("84000".into()),
                city: Some("Avignon".into()),
                country: Some("France".into()),
                // SIRET pédagogique valide (Luhn) — données fictives
                siret: Some("73282932000074".into()),
                vat_number: None,
                notes: Some("Contact fictif — démonstration".into()),
            },
        )?;

        let client_b = ContactsService::create(
            db,
            &user.id,
            ContactInput {
                kind: ContactKind::Client,
                name: "Mairie de Village-Démo".into(),
                email: Some("mairie@village-demo.example".into()),
                phone: None,
                address_line1: Some("Hôtel de Ville".into()),
                address_line2: None,
                postal_code: Some("84100".into()),
                city: Some("Orange".into()),
                country: Some("France".into()),
                siret: None,
                vat_number: None,
                notes: Some("Fictif".into()),
            },
        )?;

        let _fournisseur = ContactsService::create(
            db,
            &user.id,
            ContactInput {
                kind: ContactKind::Fournisseur,
                name: "Éclairage Fiction SAS".into(),
                email: Some("factures@eclairage-fiction.demo".into()),
                phone: Some("04 00 00 00 99".into()),
                address_line1: Some("88 avenue Imaginaire".into()),
                address_line2: None,
                postal_code: Some("13001".into()),
                city: Some("Marseille".into()),
                country: Some("France".into()),
                siret: Some("44306184100047".into()),
                vat_number: None,
                notes: Some("Fournisseur fictif".into()),
            },
        )?;

        let today = Utc::now().date_naive();
        let issue = today - Duration::days(20);
        let due = today + Duration::days(10);

        let draft = InvoicesService::create_draft(
            db,
            &user.id,
            InvoiceDraftInput {
                contact_id: client_a.id.clone(),
                issue_date: Some(issue),
                due_date: Some(due),
                customer_notes: Some("Prestation spectacle — données de démonstration".into()),
                lines: vec![
                    InvoiceLineInput {
                        description: "Location salle (soirée démo)".into(),
                        quantity: dec!(1),
                        unit_price_ht: dec!(800.00),
                        tax_rate_percent: dec!(20),
                        line_kind: Some(crate::models::invoice::InvoiceLineKind::LocationSalle),
                    },
                    InvoiceLineInput {
                        description: "Régie lumière forfait".into(),
                        quantity: dec!(1),
                        unit_price_ht: dec!(350.00),
                        tax_rate_percent: dec!(20), ..Default::default() },
                ],
                invoice_kind: None,
                ..Default::default()
            },
        )?;
        let issued = InvoicesService::issue(db, &user.id, &draft.id)?;

        // Acompte fictif
        InvoicesService::record_payment(
            db,
            &user.id,
            crate::models::invoice::RecordPaymentInput {
                invoice_id: issued.id.clone(),
                amount: dec!(500.00),
                paid_at: issue + Duration::days(5),
                method: Some("virement".into()),
                reference: Some("ACOMPTE-DEMO-001".into()),
                notes: Some("Acompte fictif".into()),
                payment_type: Some(crate::models::invoice::PaymentType::Acompte),
            },
        )?;

        let draft2 = InvoicesService::create_draft(
            db,
            &user.id,
            InvoiceDraftInput {
                contact_id: client_b.id.clone(),
                issue_date: Some(today - Duration::days(5)),
                due_date: Some(today + Duration::days(25)),
                customer_notes: None,
                lines: vec![InvoiceLineInput {
                    description: "Atelier pédagogique (démo)".into(),
                    quantity: dec!(2),
                    unit_price_ht: dec!(120.00),
                    tax_rate_percent: dec!(5.5), ..Default::default() }],
                invoice_kind: None,
                ..Default::default()
            },
        )?;
        let issued2 = InvoicesService::issue(db, &user.id, &draft2.id)?;

        // Avoir partiel
        InvoicesService::create_credit_note_draft(
            db,
            &user.id,
            &issued2.id,
            vec![InvoiceLineInput {
                description: "Avoir partiel — annulation atelier (démo)".into(),
                quantity: dec!(1),
                unit_price_ht: dec!(120.00),
                tax_rate_percent: dec!(5.5), ..Default::default() }],
            Some("Avoir fictif sans valeur comptable".into()),
        )?;

        // Brouillon libre
        InvoicesService::create_draft(
            db,
            &user.id,
            InvoiceDraftInput {
                contact_id: client_a.id,
                issue_date: Some(today),
                due_date: None,
                customer_notes: Some("Brouillon de démonstration".into()),
                lines: vec![InvoiceLineInput {
                    description: "Option technique".into(),
                    quantity: dec!(1),
                    unit_price_ht: dec!(90.00),
                    tax_rate_percent: dec!(20), ..Default::default() }],
                invoice_kind: None,
                ..Default::default()
            },
        )?;

        SuppliersService::create(
            db,
            &user.id,
            SupplierInvoiceInput {
                supplier_contact_id: Some(_fournisseur.id),
                reference: Some("F-DEMO-2026-0042".into()),
                invoice_date: Some(NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()),
                amount_ht: Some(dec!(420.00)),
                amount_ttc: Some(dec!(504.00)),
                tax_rate_percent: Some(dec!(20)),
                notes: Some("Facture fournisseur fictive".into()),
                attachment_path: None,
            },
        )?;

        Ok(())
    }
}

/// Extension locale pour suffixes -wal / -shm.
struct PathBufExt<'a>(&'a std::path::PathBuf);

impl PathBufExt<'_> {
    fn with_suffix(&self, suffix: &str) -> std::path::PathBuf {
        let mut s = self.0.as_os_str().to_os_string();
        s.push(suffix);
        std::path::PathBuf::from(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::with_data_mode;

    #[test]
    fn seed_isole_de_prod() {
        let dir = tempfile::tempdir().unwrap();
        // Test unitaire : seed en mémoire
        let pool = DbPool::open_in_memory().unwrap();
        pool.with_conn_mut(|c| run_migrations(c)).unwrap();
        with_data_mode(true, || {
            DemoService::seed(&pool).unwrap();
            let n: i64 = pool
                .with_conn(|c| {
                    Ok(c.query_row("SELECT COUNT(*) FROM contacts", [], |r| r.get(0))?)
                })
                .unwrap();
            assert!(n >= 3);
            let _ = dir;
        });
    }
}
