use std::cell::Cell;
use std::path::PathBuf;

use crate::error::{AppError, AppResult};

/// Dossier racine sous %APPDATA% (jamais le répertoire d'installation).
pub const APP_DATA_FOLDER: &str = "fr.nexyros.carreterie-gestion";
/// Base de production (conservée pour les installations existantes).
pub const DB_FILE_NAME: &str = "carreterie.db";
pub const DEMO_DIR: &str = "demo";
pub const DEMO_DB_FILE_NAME: &str = "demo.db";
pub const ATTACHMENTS_DIR: &str = "attachments";
pub const BACKUPS_DIR: &str = "backups";

thread_local! {
    static DEMO_DATA_MODE: Cell<bool> = const { Cell::new(false) };
}

/// Exécute `f` en forçant les chemins fichiers vers prod (`false`) ou démo (`true`).
pub fn with_data_mode<R, F>(demo: bool, f: F) -> R
where
    F: FnOnce() -> R,
{
    DEMO_DATA_MODE.with(|cell| {
        let previous = cell.replace(demo);
        let result = f();
        cell.set(previous);
        result
    })
}

pub fn is_demo_data_mode() -> bool {
    DEMO_DATA_MODE.with(|c| c.get())
}

pub fn app_data_root() -> AppResult<PathBuf> {
    let appdata = std::env::var("APPDATA").map_err(|_| {
        AppError::interne("Variable d'environnement APPDATA introuvable (Windows requis).")
    })?;
    Ok(PathBuf::from(appdata).join(APP_DATA_FOLDER))
}

/// Racine données active (prod ou sous-dossier demo/).
pub fn active_data_root() -> AppResult<PathBuf> {
    if is_demo_data_mode() {
        Ok(app_data_root()?.join(DEMO_DIR))
    } else {
        app_data_root()
    }
}

pub fn production_database_path() -> AppResult<PathBuf> {
    Ok(app_data_root()?.join(DB_FILE_NAME))
}

pub fn demo_database_path() -> AppResult<PathBuf> {
    Ok(app_data_root()?.join(DEMO_DIR).join(DEMO_DB_FILE_NAME))
}

pub fn database_path() -> AppResult<PathBuf> {
    if is_demo_data_mode() {
        demo_database_path()
    } else {
        production_database_path()
    }
}

pub fn attachments_dir() -> AppResult<PathBuf> {
    Ok(active_data_root()?.join(ATTACHMENTS_DIR))
}

pub fn backups_dir() -> AppResult<PathBuf> {
    Ok(active_data_root()?.join(BACKUPS_DIR))
}

/// PDF importés (sous le dossier pièces jointes, inclus dans les sauvegardes).
pub fn imports_dir() -> AppResult<PathBuf> {
    Ok(attachments_dir()?.join("imports"))
}

/// Avatars / logos profil utilisateur.
pub fn avatars_dir() -> AppResult<PathBuf> {
    Ok(attachments_dir()?.join("avatars"))
}

/// Espace disque libre (octets) pour le volume contenant `path`.
pub fn free_disk_bytes(path: &std::path::Path) -> AppResult<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        #[link(name = "kernel32")]
        extern "system" {
            fn GetDiskFreeSpaceExW(
                lp_directory_name: *const u16,
                lp_free_bytes_available_to_caller: *mut u64,
                lp_total_number_of_bytes: *mut u64,
                lp_total_number_of_free_bytes: *mut u64,
            ) -> i32;
        }

        let mut dir = if path.exists() {
            path.to_path_buf()
        } else {
            path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| path.to_path_buf())
        };
        if !dir.exists() {
            dir = app_data_root()?;
            std::fs::create_dir_all(&dir)?;
        }
        let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut free_avail: u64 = 0;
        let mut total: u64 = 0;
        let mut free_total: u64 = 0;
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_avail,
                &mut total,
                &mut free_total,
            )
        };
        if ok == 0 {
            return Err(AppError::interne("Impossible de lire l'espace disque libre."));
        }
        Ok(free_avail)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(2 * 1024 * 1024 * 1024)
    }
}

/// Taille max d'un fichier d'import : espace libre moins une réserve de 200 Mo.
pub fn max_import_bytes() -> AppResult<u64> {
    let imports = imports_dir()?;
    std::fs::create_dir_all(&imports)?;
    let free = free_disk_bytes(&imports)?;
    const RESERVE: u64 = 200 * 1024 * 1024;
    const MIN_CAP: u64 = 1024 * 1024;
    let max = free.saturating_sub(RESERVE);
    Ok(max.max(MIN_CAP).min(free.saturating_sub(MIN_CAP).max(MIN_CAP)))
}

pub fn ensure_app_directories() -> AppResult<()> {
    let root = app_data_root()?;
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir_all(root.join(ATTACHMENTS_DIR))?;
    std::fs::create_dir_all(root.join(ATTACHMENTS_DIR).join("imports"))?;
    std::fs::create_dir_all(root.join(ATTACHMENTS_DIR).join("avatars"))?;
    std::fs::create_dir_all(root.join(BACKUPS_DIR))?;

    let demo_root = root.join(DEMO_DIR);
    std::fs::create_dir_all(&demo_root)?;
    std::fs::create_dir_all(demo_root.join(ATTACHMENTS_DIR))?;
    std::fs::create_dir_all(demo_root.join(ATTACHMENTS_DIR).join("imports"))?;
    std::fs::create_dir_all(demo_root.join(ATTACHMENTS_DIR).join("avatars"))?;
    std::fs::create_dir_all(demo_root.join(BACKUPS_DIR))?;
    Ok(())
}

/// Refuse tout chemin qui sortirait de la racine active (anti-contournement).
pub fn assert_path_inside_active_root(path: &std::path::Path) -> AppResult<()> {
    let root = active_data_root()?.canonicalize().unwrap_or_else(|_| active_data_root().unwrap());
    let candidate = if path.exists() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else if let Some(parent) = path.parent() {
        let parent_c = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
        parent_c.join(path.file_name().unwrap_or_default())
    } else {
        path.to_path_buf()
    };
    if !candidate.starts_with(&root) {
        return Err(AppError::interdit(
            "Accès fichier refusé : hors de l'espace de données autorisé pour ce mode.",
        ));
    }
    Ok(())
}
