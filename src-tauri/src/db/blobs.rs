//! Stockage fichiers partagés (PDF, logos) dans MySQL `file_blobs` + cache local.

use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::db::{DbPool, SqlVal};
use crate::error::{AppError, AppResult};
use crate::paths::{active_data_root, attachments_dir};

pub struct BlobStore;

impl BlobStore {
    fn relative_key(path: &Path) -> AppResult<String> {
        let root = active_data_root()?;
        let canon = if path.exists() {
            path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
        } else {
            path.to_path_buf()
        };
        let root_c = root.canonicalize().unwrap_or(root);
        let rel = canon.strip_prefix(&root_c).unwrap_or(path);
        Ok(rel.to_string_lossy().replace('\\', "/"))
    }

    pub fn put_file(db: &DbPool, path: &Path, content_type: &str) -> AppResult<()> {
        if !db.is_remote() {
            return Ok(());
        }
        let bytes = std::fs::read(path)?;
        let key = Self::relative_key(path)?;
        Self::put_bytes(db, &key, &bytes, content_type)
    }

    pub fn put_bytes(db: &DbPool, key: &str, bytes: &[u8], content_type: &str) -> AppResult<()> {
        if !db.is_remote() {
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        db.execute(
            "INSERT INTO file_blobs (path, content, size_bytes, content_type, updated_at) VALUES (?, ?, ?, ?, ?)
             ON DUPLICATE KEY UPDATE content = VALUES(content), size_bytes = VALUES(size_bytes), content_type = VALUES(content_type), updated_at = VALUES(updated_at)",
            &[
                SqlVal::text(key),
                SqlVal::Blob(bytes.to_vec()),
                SqlVal::int(bytes.len() as i64),
                SqlVal::text(content_type),
                SqlVal::text(now),
            ],
        )?;
        Ok(())
    }

    /// Assure un fichier local (cache) à partir du blob distant.
    #[allow(dead_code)]
    pub fn ensure_local(db: &DbPool, relative_or_absolute: &Path) -> AppResult<PathBuf> {
        if !db.is_remote() {
            return Ok(relative_or_absolute.to_path_buf());
        }
        let key = if relative_or_absolute.is_absolute() {
            Self::relative_key(relative_or_absolute)?
        } else {
            relative_or_absolute.to_string_lossy().replace('\\', "/")
        };
        let local = active_data_root()?.join(key.replace('/', std::path::MAIN_SEPARATOR_STR));
        if local.exists() {
            return Ok(local);
        }
        let row = db.query_one_opt(
            "SELECT content FROM file_blobs WHERE path = ?",
            &[SqlVal::text(&key)],
        )?;
        let Some(cells) = row else {
            return Err(AppError::Introuvable);
        };
        let bytes = cells.first().map(|c| c.as_blob()).transpose()?.unwrap_or_default();
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&local, bytes)?;
        Ok(local)
    }

    pub fn sync_attachments_dir(db: &DbPool) -> AppResult<usize> {
        if !db.is_remote() {
            return Ok(0);
        }
        let dir = attachments_dir()?;
        if !dir.exists() {
            return Ok(0);
        }
        let mut n = 0;
        for entry in walkdir_files(&dir)? {
            let ct = content_type_for(&entry);
            Self::put_file(db, &entry, ct)?;
            n += 1;
        }
        Ok(n)
    }
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

fn walkdir_files(dir: &Path) -> AppResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    fn walk(d: &Path, out: &mut Vec<PathBuf>) -> AppResult<()> {
        for entry in std::fs::read_dir(d)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out)?;
            } else if p.is_file() {
                out.push(p);
            }
        }
        Ok(())
    }
    walk(dir, &mut out)?;
    Ok(out)
}
