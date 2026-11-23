use serde::Serialize;

use crate::auth::{self, SessionUser};
use crate::db::DbPool;
use crate::error::{AppResult, ErrorPayload};
use crate::models::contact::{Contact, ContactInput};
use crate::models::invoice::{
    Invoice, InvoiceDraftInput, InvoiceLineInput, InvoiceStatus, RecordPaymentInput,
};
use crate::models::settings::{AppSettings, AppSettingsPatch};
use crate::models::supplier_invoice::{SupplierInvoice, SupplierInvoiceInput};
use crate::models::audit::AuditEntry;
use crate::models::import::{
    ContactMatchCandidate, ImportAnalyzeResult, ImportListFilters, ImportValidateInput,
    ImportedDocument,
};
use crate::paths;
use crate::services::archives::ArchivesService;
use crate::services::audit::AuditService;
use crate::services::backup::{BackupInfo, BackupService};
use crate::services::contacts::ContactsService;
use crate::services::dashboard::{DashboardService, DashboardStats};
use crate::services::demo::DemoService;
use crate::services::invoices::InvoicesService;
use crate::services::pdf::PdfService;
use crate::services::settings::SettingsService;
use crate::services::import::ImportService;
use crate::services::suppliers::SuppliersService;
use crate::state::AppState;
use crate::DISCLAIMER_E_FACTURATION;

fn map_err<T>(result: AppResult<T>) -> Result<T, ErrorPayload> {
    result.map_err(ErrorPayload::from)
}

/// Toutes les commandes métier passent par ici : pool + chemins alignés sur le mode session.
fn authed<T, F>(state: &AppState, session_token: &str, f: F) -> Result<T, ErrorPayload>
where
    F: FnOnce(&SessionUser, &DbPool) -> AppResult<T>,
{
    let user = state.require_session(session_token).map_err(ErrorPayload::from)?;
    map_err(paths::with_data_mode(user.is_demo, || {
        state.with_pool_for_user(&user, |db| f(&user, db))
    }))
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub username: String,
    pub user_id: String,
    pub disclaimer: &'static str,
    pub is_demo: bool,
}

#[derive(Serialize)]
pub struct FirstLaunchStatus {
    pub needs_setup: bool,
    pub disclaimer: &'static str,
}

#[tauri::command]
pub fn app_disclaimer() -> &'static str {
    DISCLAIMER_E_FACTURATION
}

#[derive(Serialize)]
pub struct DbStatus {
    pub connected: bool,
    pub is_demo: bool,
}

#[tauri::command]
pub fn db_ping(
    state: tauri::State<'_, AppState>,
    session_token: Option<String>,
) -> Result<DbStatus, ErrorPayload> {
    if let Some(token) = session_token.filter(|t| !t.trim().is_empty()) {
        return authed(&state, &token, |user, db| {
            db.with_conn(|conn| {
                conn.query_row("SELECT 1", [], |row| row.get::<_, i32>(0))
                    .map_err(crate::error::AppError::BaseDeDonnees)?;
                Ok(())
            })?;
            Ok(DbStatus {
                connected: true,
                is_demo: user.is_demo,
            })
        });
    }
    let ok = state
        .prod_db
        .with_conn(|conn| {
            conn.query_row("SELECT 1", [], |row| row.get::<_, i32>(0))
                .map_err(crate::error::AppError::BaseDeDonnees)?;
            Ok(())
        })
        .is_ok();
    Ok(DbStatus {
        connected: ok,
        is_demo: false,
    })
}

#[tauri::command]
pub fn auth_first_launch(state: tauri::State<'_, AppState>) -> Result<FirstLaunchStatus, ErrorPayload> {
    let needs = !auth::has_any_user(&state.prod_db).map_err(ErrorPayload::from)?;
    Ok(FirstLaunchStatus {
        needs_setup: needs,
        disclaimer: DISCLAIMER_E_FACTURATION,
    })
}

#[tauri::command]
pub fn auth_setup_admin(
    state: tauri::State<'_, AppState>,
    username: String,
    password: String,
) -> Result<(), ErrorPayload> {
    map_err(auth::setup_initial_admin(&state.prod_db, &username, &password))
}

#[tauri::command]
pub fn auth_login(
    state: tauri::State<'_, AppState>,
    username: String,
    password: String,
) -> Result<LoginResponse, ErrorPayload> {
    let session = state.login(&username, &password).map_err(ErrorPayload::from)?;
    Ok(LoginResponse {
        token: session.token,
        username: session.user.username,
        user_id: session.user.id,
        disclaimer: DISCLAIMER_E_FACTURATION,
        is_demo: false,
    })
}

#[tauri::command]
pub fn auth_enter_demo(state: tauri::State<'_, AppState>) -> Result<LoginResponse, ErrorPayload> {
    let session = DemoService::enter(&state).map_err(ErrorPayload::from)?;
    Ok(LoginResponse {
        token: session.token,
        username: session.user.username,
        user_id: session.user.id,
        disclaimer: DISCLAIMER_E_FACTURATION,
        is_demo: true,
    })
}

#[tauri::command]
pub fn demo_reset(
    state: tauri::State<'_, AppState>,
    session_token: String,
) -> Result<(), ErrorPayload> {
    let user = state.require_session(&session_token).map_err(ErrorPayload::from)?;
    if !user.is_demo {
        return Err(ErrorPayload::from(crate::error::AppError::interdit(
            "La réinitialisation démo n'est disponible qu'en mode démonstration.",
        )));
    }
    map_err(DemoService::reset(&state))
}

#[tauri::command]
pub fn auth_logout(
    state: tauri::State<'_, AppState>,
    session_token: String,
) -> Result<(), ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        auth::logout(db, &state.sessions, &session_token, &user.id)
    })
}

#[tauri::command]
pub fn auth_session_ping(
    state: tauri::State<'_, AppState>,
    session_token: String,
) -> Result<LoginResponse, ErrorPayload> {
    let record = state.sessions.touch(&session_token).map_err(ErrorPayload::from)?;
    Ok(LoginResponse {
        token: record.token,
        username: record.user.username,
        user_id: record.user.id,
        disclaimer: DISCLAIMER_E_FACTURATION,
        is_demo: record.user.is_demo,
    })
}

#[tauri::command]
pub fn auth_change_password(
    state: tauri::State<'_, AppState>,
    session_token: String,
    current_password: String,
    new_password: String,
) -> Result<(), ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        if user.is_demo {
            return Err(crate::error::AppError::interdit(
                "Changement de mot de passe indisponible en mode démonstration.",
            ));
        }
        auth::change_password(db, &user.id, &current_password, &new_password)
    })
}

#[tauri::command]
pub fn settings_get(
    state: tauri::State<'_, AppState>,
    session_token: String,
) -> Result<AppSettings, ErrorPayload> {
    authed(&state, &session_token, |_u, db| SettingsService::load(db))
}

#[tauri::command]
pub fn settings_update(
    state: tauri::State<'_, AppState>,
    session_token: String,
    patch: AppSettingsPatch,
) -> Result<AppSettings, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        SettingsService::update(db, &user.id, patch)
    })
}

#[tauri::command]
pub fn contacts_list(
    state: tauri::State<'_, AppState>,
    session_token: String,
    search: Option<String>,
) -> Result<Vec<Contact>, ErrorPayload> {
    authed(&state, &session_token, |_u, db| {
        ContactsService::list(db, search.as_deref())
    })
}

#[tauri::command]
pub fn contacts_get(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
) -> Result<Contact, ErrorPayload> {
    authed(&state, &session_token, |_u, db| ContactsService::get(db, &id))
}

#[tauri::command]
pub fn contacts_create(
    state: tauri::State<'_, AppState>,
    session_token: String,
    input: ContactInput,
) -> Result<Contact, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        ContactsService::create(db, &user.id, input)
    })
}

#[tauri::command]
pub fn contacts_update(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
    input: ContactInput,
) -> Result<Contact, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        ContactsService::update(db, &user.id, &id, input)
    })
}

#[tauri::command]
pub fn contacts_delete(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
) -> Result<(), ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        ContactsService::delete(db, &user.id, &id)
    })
}

#[tauri::command]
pub fn invoices_list(
    state: tauri::State<'_, AppState>,
    session_token: String,
    status: Option<String>,
) -> Result<Vec<Invoice>, ErrorPayload> {
    authed(&state, &session_token, |_u, db| {
        let st = status.as_deref().and_then(InvoiceStatus::from_db_str);
        InvoicesService::list(db, st)
    })
}

#[tauri::command]
pub fn invoices_get(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
) -> Result<Invoice, ErrorPayload> {
    authed(&state, &session_token, |_u, db| InvoicesService::get(db, &id))
}

#[tauri::command]
pub fn invoices_create_draft(
    state: tauri::State<'_, AppState>,
    session_token: String,
    input: InvoiceDraftInput,
) -> Result<Invoice, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        InvoicesService::create_draft(db, &user.id, input)
    })
}

#[tauri::command]
pub fn invoices_update_draft(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
    input: InvoiceDraftInput,
) -> Result<Invoice, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        InvoicesService::update_draft(db, &user.id, &id, input)
    })
}

#[tauri::command]
pub fn invoices_delete_draft(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
) -> Result<(), ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        InvoicesService::delete_draft(db, &user.id, &id)
    })
}

#[tauri::command]
pub fn invoices_issue(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
) -> Result<Invoice, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        InvoicesService::issue(db, &user.id, &id)
    })
}

#[tauri::command]
pub fn invoices_create_credit_note(
    state: tauri::State<'_, AppState>,
    session_token: String,
    related_invoice_id: String,
    lines: Vec<InvoiceLineInput>,
    customer_notes: Option<String>,
) -> Result<Invoice, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        InvoicesService::create_credit_note_draft(
            db,
            &user.id,
            &related_invoice_id,
            lines,
            customer_notes,
        )
    })
}

#[tauri::command]
pub fn invoices_generate_pdf(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
) -> Result<String, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        let path = PdfService::generate_invoice_pdf(db, &id, user.is_demo)?;
        paths::assert_path_inside_active_root(&path)?;
        Ok(path.to_string_lossy().to_string())
    })
}

#[tauri::command]
pub fn invoices_record_payment(
    state: tauri::State<'_, AppState>,
    session_token: String,
    input: RecordPaymentInput,
) -> Result<Invoice, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        InvoicesService::record_payment(db, &user.id, input)
    })
}

#[tauri::command]
pub fn suppliers_list(
    state: tauri::State<'_, AppState>,
    session_token: String,
) -> Result<Vec<SupplierInvoice>, ErrorPayload> {
    authed(&state, &session_token, |_u, db| SuppliersService::list(db))
}

#[tauri::command]
pub fn suppliers_get(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
) -> Result<SupplierInvoice, ErrorPayload> {
    authed(&state, &session_token, |_u, db| SuppliersService::get(db, &id))
}

#[tauri::command]
pub fn suppliers_create(
    state: tauri::State<'_, AppState>,
    session_token: String,
    input: SupplierInvoiceInput,
) -> Result<SupplierInvoice, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        SuppliersService::create(db, &user.id, input)
    })
}

#[tauri::command]
pub fn suppliers_update(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
    input: SupplierInvoiceInput,
) -> Result<SupplierInvoice, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        SuppliersService::update(db, &user.id, &id, input)
    })
}

#[tauri::command]
pub fn suppliers_delete(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
) -> Result<(), ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        SuppliersService::delete(db, &user.id, &id)
    })
}

#[tauri::command]
pub fn dashboard_stats(
    state: tauri::State<'_, AppState>,
    session_token: String,
) -> Result<DashboardStats, ErrorPayload> {
    authed(&state, &session_token, |_u, db| DashboardService::stats(db))
}

#[tauri::command]
pub fn archives_export_csv(
    state: tauri::State<'_, AppState>,
    session_token: String,
) -> Result<String, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        let path = ArchivesService::export_invoices_csv(db, &user.id)?;
        paths::assert_path_inside_active_root(&path)?;
        Ok(path.to_string_lossy().to_string())
    })
}

#[tauri::command]
pub fn backup_list(
    state: tauri::State<'_, AppState>,
    session_token: String,
) -> Result<Vec<BackupInfo>, ErrorPayload> {
    authed(&state, &session_token, |_u, _db| BackupService::list())
}

#[tauri::command]
pub fn backup_create(
    state: tauri::State<'_, AppState>,
    session_token: String,
    password: Option<String>,
) -> Result<BackupInfo, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        BackupService::create(db, &user.id, password.as_deref())
    })
}

#[tauri::command]
pub fn backup_restore(
    state: tauri::State<'_, AppState>,
    session_token: String,
    backup_path: String,
    password: Option<String>,
) -> Result<(), ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        // Source libre (USB / dossier utilisateur) ; la restauration écrit uniquement
        // dans la base active (prod ou démo) via paths::database_path().
        BackupService::restore(
            db,
            &user.id,
            std::path::Path::new(&backup_path),
            password.as_deref(),
        )
    })
}

#[tauri::command]
pub fn audit_list(
    state: tauri::State<'_, AppState>,
    session_token: String,
    limit: Option<i64>,
) -> Result<Vec<AuditEntry>, ErrorPayload> {
    authed(&state, &session_token, |_u, db| {
        AuditService::list_recent(db, limit.unwrap_or(100))
    })
}

#[tauri::command]
pub fn import_analyze_pdfs(
    state: tauri::State<'_, AppState>,
    session_token: String,
    paths: Vec<String>,
    direction: String,
) -> Result<Vec<ImportAnalyzeResult>, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        ImportService::analyze_paths(db, &user.id, paths, &direction)
    })
}

#[tauri::command]
pub fn import_list(
    state: tauri::State<'_, AppState>,
    session_token: String,
    status: Option<String>,
    direction: Option<String>,
    year: Option<i32>,
    contact_id: Option<String>,
    document_type: Option<String>,
) -> Result<Vec<ImportedDocument>, ErrorPayload> {
    authed(&state, &session_token, |_u, db| {
        ImportService::list(
            db,
            ImportListFilters {
                status,
                direction,
                year,
                contact_id,
                document_type,
            },
        )
    })
}

#[tauri::command]
pub fn import_get(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
) -> Result<ImportedDocument, ErrorPayload> {
    authed(&state, &session_token, |_u, db| ImportService::get(db, &id))
}

#[tauri::command]
pub fn import_pdf_data(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
) -> Result<String, ErrorPayload> {
    authed(&state, &session_token, |_u, db| {
        ImportService::get_pdf_base64(db, &id)
    })
}

#[tauri::command]
pub fn import_match_contacts(
    state: tauri::State<'_, AppState>,
    session_token: String,
    siret: Option<String>,
    siren: Option<String>,
    name: Option<String>,
) -> Result<Vec<ContactMatchCandidate>, ErrorPayload> {
    authed(&state, &session_token, |_u, db| {
        ImportService::match_contacts(db, siret.as_deref(), siren.as_deref(), name.as_deref())
    })
}

#[tauri::command]
pub fn import_validate(
    state: tauri::State<'_, AppState>,
    session_token: String,
    input: ImportValidateInput,
) -> Result<ImportedDocument, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        ImportService::validate(db, &user.id, input)
    })
}

#[tauri::command]
pub fn import_reject(
    state: tauri::State<'_, AppState>,
    session_token: String,
    id: String,
    reason: Option<String>,
) -> Result<ImportedDocument, ErrorPayload> {
    authed(&state, &session_token, |user, db| {
        ImportService::reject(db, &user.id, &id, reason)
    })
}
