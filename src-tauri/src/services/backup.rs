use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use chrono::Utc;
use rand::RngCore;
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::paths::{attachments_dir, backups_dir, database_path};
use crate::services::audit::AuditService;

const MANIFEST: &str = "manifest.sha256";
const DB_ENTRY: &str = "carreterie.db";
const NONCE_LEN: usize = 12;

pub struct BackupService;

#[derive(serde::Serialize)]
pub struct BackupInfo {
    pub path: String,
    pub encrypted: bool,
    pub created_at: String,
    pub filename: String,
    pub size_bytes: u64,
}

impl BackupService {
    pub fn create(db: &DbPool, user_id: &str, password: Option<&str>) -> AppResult<BackupInfo> {
        if db.is_remote() {
            return Err(AppError::validation(
                "Mode o2switch : sauvegardez via phpMyAdmin (Export SQL), pas via VACUUM local.",
            ));
        }
        let backups = backups_dir()?;
        fs::create_dir_all(&backups)?;

        let stamp = Utc::now().format("%Y%m%d_%H%M%S");
        let base_name = format!("carreterie_sauvegarde_{stamp}");
        let zip_path = backups.join(format!("{base_name}.zip"));
        let vacuum_path = backups.join(format!("{base_name}_vacuum.db"));

        db.with_conn_mut(|conn| {
            conn.execute(
                &format!("VACUUM INTO '{}'", vacuum_path.to_string_lossy().replace('\'', "''")),
                [],
            )?;
            Ok(())
        })?;

        let mut checksums = String::new();
        let db_hash = sha256_file(&vacuum_path)?;
        checksums.push_str(&format!("{db_hash}  {DB_ENTRY}\n"));

        let zip_file = File::create(&zip_path)?;
        let mut zip = ZipWriter::new(zip_file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        let att_dir = attachments_dir()?;
        if att_dir.exists() {
            for entry in fs::read_dir(&att_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("fichier");
                    let hash = sha256_file(&path)?;
                    checksums.push_str(&format!("{hash}  attachments/{name}\n"));
                }
            }
        }

        add_file_to_zip(&mut zip, &vacuum_path, DB_ENTRY, options)?;

        if att_dir.exists() {
            add_dir_to_zip(&mut zip, &att_dir, "attachments", options)?;
        }

        zip.start_file(MANIFEST, options)
            .map_err(|e| AppError::interne(e.to_string()))?;
        zip.write_all(checksums.as_bytes())
            .map_err(|e| AppError::interne(e.to_string()))?;

        zip.finish()?;

        let mut final_path = zip_path.clone();
        let encrypted = password.is_some();

        if let Some(pass) = password.filter(|p| !p.is_empty()) {
            let enc_path = backups.join(format!("{base_name}.zip.enc"));
            encrypt_file(&zip_path, &enc_path, pass)?;
            fs::remove_file(&zip_path)?;
            final_path = enc_path;
        }

        fs::write(backups.join(format!("{base_name}_{MANIFEST}")), &checksums)?;

        if vacuum_path.exists() {
            let _ = fs::remove_file(&vacuum_path);
        }

        AuditService::log(
            db,
            user_id,
            "sauvegarde_creee",
            Some("backup"),
            None,
            Some(final_path.to_string_lossy().as_ref()),
        )?;

        let size_bytes = fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
        let filename = final_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sauvegarde.zip")
            .to_string();
        Ok(BackupInfo {
            path: final_path.to_string_lossy().to_string(),
            encrypted,
            created_at: Utc::now().to_rfc3339(),
            filename,
            size_bytes,
        })
    }

    pub fn restore(
        db: &DbPool,
        user_id: &str,
        backup_path: &Path,
        password: Option<&str>,
    ) -> AppResult<()> {
        if !backup_path.exists() {
            return Err(AppError::validation("Fichier de sauvegarde introuvable."));
        }

        let temp_dir = std::env::temp_dir().join(format!(
            "carreterie_restore_{}",
            Utc::now().format("%Y%m%d%H%M%S%3f")
        ));
        fs::create_dir_all(&temp_dir)?;
        let work_zip = if backup_path.extension().and_then(|e| e.to_str()) == Some("enc") {
            let pass = password.filter(|p| !p.is_empty()).ok_or_else(|| {
                AppError::validation("Mot de passe requis pour cette sauvegarde chiffrée.")
            })?;
            let out = temp_dir.join("restore.zip");
            decrypt_file(backup_path, &out, pass)?;
            out
        } else {
            backup_path.to_path_buf()
        };

        let zip_file = File::open(&work_zip)?;
        let mut archive = ZipArchive::new(zip_file).map_err(|e| AppError::interne(e.to_string()))?;
        let extract_db = temp_dir.join(DB_ENTRY);
        {
            let mut entry = archive.by_name(DB_ENTRY).map_err(|_| {
                AppError::validation("Archive invalide : base de données absente.")
            })?;
            let mut out = File::create(&extract_db)?;
            std::io::copy(&mut entry, &mut out)?;
        }

        let expected_hash = sha256_file(&extract_db)?;
        verify_manifest_in_zip(&work_zip, &expected_hash)?;

        let target = database_path()?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if target.exists() {
            let backup_old = target.with_extension("db.bak");
            fs::copy(&target, &backup_old)?;
        }
        fs::copy(&extract_db, &target)?;

        // Recharger la connexion : le pool garde l'ancien handle — fermer n'est pas trivial;
        // l'application devrait redémarrer après restauration (documenté côté commande).
        let _ = db.with_conn(|conn| {
            conn.execute("PRAGMA integrity_check", [])?;
            Ok(())
        });

        AuditService::log(
            db,
            user_id,
            "sauvegarde_restauree",
            Some("backup"),
            None,
            Some(backup_path.to_string_lossy().as_ref()),
        )?;
        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    pub fn list() -> AppResult<Vec<BackupInfo>> {
        let backups = backups_dir()?;
        if !backups.exists() {
            return Ok(Vec::new());
        }
        let mut items = Vec::new();
        for entry in fs::read_dir(&backups)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if !(name.ends_with(".zip") || name.ends_with(".zip.enc")) {
                continue;
            }
            let meta = entry.metadata()?;
            let created = meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<Utc>::from(t).to_rfc3339())
                .unwrap_or_else(|| Utc::now().to_rfc3339());
            items.push(BackupInfo {
                path: path.to_string_lossy().to_string(),
                encrypted: name.ends_with(".enc"),
                created_at: created,
                filename: name,
                size_bytes: meta.len(),
            });
        }
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(items)
    }
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn add_file_to_zip(
    zip: &mut ZipWriter<File>,
    path: &Path,
    name: &str,
    options: SimpleFileOptions,
) -> AppResult<()> {
    zip.start_file(name, options)
        .map_err(|e| AppError::interne(e.to_string()))?;
    let mut f = File::open(path)?;
    std::io::copy(&mut f, zip).map_err(|e| AppError::interne(e.to_string()))?;
    Ok(())
}

fn derive_key(password: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(b"carreterie-gestion-backup-v1");
    hasher.finalize().into()
}

fn encrypt_file(src: &Path, dst: &Path, password: &str) -> AppResult<()> {
    let key = derive_key(password);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| AppError::interne(format!("Clé AES invalide : {e}")))?;
    let mut plaintext = Vec::new();
    File::open(src)?.read_to_end(&mut plaintext)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| AppError::interne(format!("Chiffrement échoué : {e}")))?;
    let mut out = File::create(dst)?;
    out.write_all(&nonce_bytes)?;
    out.write_all(&ciphertext)?;
    Ok(())
}

fn decrypt_file(src: &Path, dst: &Path, password: &str) -> AppResult<()> {
    let key = derive_key(password);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| AppError::interne(format!("Clé AES invalide : {e}")))?;
    let mut data = Vec::new();
    File::open(src)?.read_to_end(&mut data)?;
    if data.len() <= NONCE_LEN {
        return Err(AppError::validation("Fichier chiffré corrompu."));
    }
    let (nonce_bytes, cipher_bytes) = data.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, cipher_bytes)
        .map_err(|_| AppError::validation("Mot de passe de sauvegarde incorrect."))?;
    let mut out = File::create(dst)?;
    out.write_all(&plaintext)?;
    Ok(())
}

fn add_dir_to_zip(
    zip: &mut ZipWriter<File>,
    dir: &Path,
    prefix: &str,
    options: SimpleFileOptions,
) -> AppResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("fichier");
        let zip_path = format!("{prefix}/{name}");
        if path.is_dir() {
            add_dir_to_zip(zip, &path, &zip_path, options)?;
        } else if path.is_file() {
            add_file_to_zip(zip, &path, &zip_path, options)?;
        }
    }
    Ok(())
}

fn verify_manifest_in_zip(zip_path: &Path, db_hash: &str) -> AppResult<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| AppError::interne(e.to_string()))?;
    let mut found = false;
    if let Ok(mut entry) = archive.by_name(MANIFEST) {
        let mut content = String::new();
        entry.read_to_string(&mut content)?;
        for line in content.lines() {
            if line.contains(DB_ENTRY) {
                let expected = line.split_whitespace().next().unwrap_or("");
                if expected != db_hash {
                    return Err(AppError::validation(
                        "Intégrité de la sauvegarde non vérifiée (SHA-256).",
                    ));
                }
                found = true;
            }
        }
    }
    if !found {
        if db_hash.len() != 64 {
            return Err(AppError::validation("Empreinte SHA-256 invalide."));
        }
    }
    Ok(())
}