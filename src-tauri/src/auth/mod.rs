use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use rand::rngs::OsRng;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::services::audit::AuditService;
use crate::state::{SessionManager, LOCKOUT_MAX_ATTEMPTS, LOCKOUT_MINUTES};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionUser {
    pub id: String,
    pub username: String,
    /// Session liée à demo.db — jamais à la base de production.
    #[serde(default)]
    pub is_demo: bool,
}

pub fn hash_password(password: &str) -> AppResult<String> {
    if password.len() < 12 {
        return Err(AppError::validation(
            "Le mot de passe doit contenir au moins 12 caractères.",
        ));
    }
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::interne(format!("Hachage impossible : {e}")))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(password_hash)
        .map_err(|_| AppError::interne("Hash stocké invalide."))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn has_any_user(db: &DbPool) -> AppResult<bool> {
    db.with_conn(|conn| {
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        Ok(count > 0)
    })
}

pub fn setup_initial_admin(db: &DbPool, username: &str, password: &str) -> AppResult<()> {
    if has_any_user(db)? {
        return Err(AppError::AdminDejaConfigure);
    }
    let username = username.trim();
    if username.is_empty() {
        return Err(AppError::validation("Le nom d'utilisateur est obligatoire."));
    }
    let hash = hash_password(password)?;
    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    db.with_conn_mut(|conn| {
        conn.execute(
            "INSERT INTO users (id, username, password_hash, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, username, hash, now, now],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO sessions_meta (key, value) VALUES ('admin_configured', '1')",
            [],
        )?;
        Ok(())
    })?;
    AuditService::log_system(db, "admin_cree", Some("user"), Some(&id), Some(username))?;
    Ok(())
}

fn is_locked(conn: &Connection, username: &str) -> AppResult<Option<u64>> {
    let since = (Utc::now() - Duration::minutes(LOCKOUT_MINUTES)).to_rfc3339();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM login_attempts WHERE username = ?1 AND failed_at >= ?2",
        params![username, since],
        |row| row.get(0),
    )?;
    if count >= LOCKOUT_MAX_ATTEMPTS as i64 {
        let last: String = conn.query_row(
            "SELECT failed_at FROM login_attempts WHERE username = ?1 ORDER BY failed_at DESC LIMIT 1",
            params![username],
            |row| row.get(0),
        )?;
        let last_dt = chrono::DateTime::parse_from_rfc3339(&last)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let unlock = last_dt + Duration::minutes(LOCKOUT_MINUTES);
        let remaining = (unlock - Utc::now()).num_minutes().max(1) as u64;
        return Ok(Some(remaining));
    }
    Ok(None)
}

fn record_failed_attempt(conn: &mut Connection, username: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO login_attempts (username, failed_at) VALUES (?1, ?2)",
        params![username, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

fn clear_attempts(conn: &mut Connection, username: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM login_attempts WHERE username = ?1",
        params![username],
    )?;
    Ok(())
}

pub fn login(
    db: &DbPool,
    sessions: &SessionManager,
    username: &str,
    password: &str,
) -> AppResult<crate::state::SessionRecord> {
    let username = username.trim();
    if username.is_empty() || password.is_empty() {
        return Err(AppError::IdentifiantsIncorrects);
    }

    let user_row = db.with_conn_mut(|conn| -> AppResult<Option<(String, String)>> {
        if let Some(minutes) = is_locked(conn, username)? {
            return Err(AppError::CompteVerrouille { minutes });
        }
        let row = conn.query_row(
            "SELECT id, password_hash FROM users WHERE username = ?1 COLLATE NOCASE",
            params![username],
            |r| Ok((r.get(0)?, r.get(1)?)),
        );
        match row {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::BaseDeDonnees(e)),
        }
    })?;

    let Some((user_id, hash)) = user_row else {
        db.with_conn_mut(|conn| record_failed_attempt(conn, username))?;
        return Err(AppError::IdentifiantsIncorrects);
    };

    if !verify_password(password, &hash)? {
        db.with_conn_mut(|conn| record_failed_attempt(conn, username))?;
        return Err(AppError::IdentifiantsIncorrects);
    }

    db.with_conn_mut(|conn| clear_attempts(conn, username))?;

    let session = sessions.create_session(SessionUser {
        id: user_id.clone(),
        username: username.to_string(),
        is_demo: false,
    });

    AuditService::log(
        db,
        &user_id,
        "connexion",
        Some("session"),
        Some(&session.token),
        None,
    )?;

    Ok(session)
}

pub fn logout(db: &DbPool, sessions: &SessionManager, token: &str, user_id: &str) -> AppResult<()> {
    sessions.invalidate(token);
    AuditService::log(db, user_id, "deconnexion", None, None, None)?;
    Ok(())
}

pub fn change_password(
    db: &DbPool,
    user_id: &str,
    current_password: &str,
    new_password: &str,
) -> AppResult<()> {
    let hash = db.with_conn(|conn| -> AppResult<String> {
        conn.query_row(
            "SELECT password_hash FROM users WHERE id = ?1",
            params![user_id],
            |row| row.get(0),
        )
        .map_err(|_| AppError::Introuvable)
    })?;

    if !verify_password(current_password, &hash)? {
        return Err(AppError::MotDePasseActuelIncorrect);
    }

    let new_hash = hash_password(new_password)?;
    let now = Utc::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        conn.execute(
            "UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
            params![new_hash, now, user_id],
        )?;
        Ok(())
    })?;

    AuditService::log(
        db,
        user_id,
        "changement_mot_de_passe",
        Some("user"),
        Some(user_id),
        None,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_migrations;
    use rusqlite::Connection;

  #[test]
    fn hash_roundtrip() {
        let h = hash_password("MotDePasse12!").unwrap();
        assert!(verify_password("MotDePasse12!", &h).unwrap());
        assert!(!verify_password("autre", &h).unwrap());
    }

    #[test]
    fn lockout_after_five() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        for _ in 0..5 {
            record_failed_attempt(&mut conn, "admin").unwrap();
        }
        assert!(is_locked(&conn, "admin").unwrap().is_some());
    }
}