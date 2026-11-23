//! Backend Carreterie Gestion — Théâtre de la Carreterie (Nexyros).

pub const DISCLAIMER_E_FACTURATION: &str = "Ce logiciel n'est pas une plateforme de facturation électronique agréée au sens de la réglementation française.";

mod auth;
mod commands;
mod db;
mod error;
mod models;
mod money;
mod paths;
mod services;
mod state;

pub use error::{AppError, AppResult};
pub use state::AppState;

use db::{run_migrations, DbPool};
use paths::{ensure_app_directories, production_database_path};
use services::demo::DemoService;
use tauri::Manager;

/// Sauvegarde cohérente de la base prod avant migrations (si le fichier existe).
fn backup_before_migrate() -> AppResult<()> {
    let db_path = production_database_path()?;
    if !db_path.exists() {
        return Ok(());
    }
    let backups = paths::app_data_root()?.join("backups").join("pre-migrate");
    std::fs::create_dir_all(&backups)?;
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let dest = backups.join(format!("carreterie-pre-migrate-{stamp}.db"));
    std::fs::copy(&db_path, &dest)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            ensure_app_directories().expect("Impossible de créer les dossiers AppData");
            backup_before_migrate().expect("Sauvegarde pré-migration impossible");

            let prod = DbPool::open_production().expect("Impossible d'ouvrir la base de production");
            prod.with_conn_mut(|conn| run_migrations(conn))
                .expect("Échec des migrations (production)");

            let demo = DemoService::open_and_prepare().expect("Impossible de préparer demo.db");

            let app_state = AppState::new(prod, demo);
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_disclaimer,
            commands::db_ping,
            commands::auth_first_launch,
            commands::auth_setup_admin,
            commands::auth_login,
            commands::auth_enter_demo,
            commands::demo_reset,
            commands::auth_logout,
            commands::auth_session_ping,
            commands::auth_change_password,
            commands::auth_profile_get,
            commands::auth_profile_update,
            commands::auth_profile_set_avatar,
            commands::settings_get,
            commands::settings_update,
            commands::contacts_list,
            commands::contacts_get,
            commands::contacts_create,
            commands::contacts_update,
            commands::contacts_delete,
            commands::invoices_list,
            commands::invoices_get,
            commands::invoices_create_draft,
            commands::invoices_update_draft,
            commands::invoices_delete_draft,
            commands::invoices_issue,
            commands::invoices_create_credit_note,
            commands::invoices_generate_pdf,
            commands::invoices_record_payment,
            commands::suppliers_list,
            commands::suppliers_get,
            commands::suppliers_create,
            commands::suppliers_update,
            commands::suppliers_delete,
            commands::dashboard_stats,
            commands::archives_export_csv,
            commands::backup_list,
            commands::backup_create,
            commands::backup_restore,
            commands::audit_list,
            commands::import_limits_get,
            commands::import_analyze_pdfs,
            commands::import_list,
            commands::import_get,
            commands::import_pdf_data,
            commands::import_match_contacts,
            commands::import_validate,
            commands::import_reject,
        ])
        .run(tauri::generate_context!())
        .expect("Erreur au démarrage de Carreterie Gestion");
}
