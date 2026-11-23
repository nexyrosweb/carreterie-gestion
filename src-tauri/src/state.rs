use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use parking_lot::{Mutex, RwLock};
use uuid::Uuid;

use crate::auth::{self, SessionUser};
use crate::db::DbPool;
use crate::error::{AppError, AppResult};

pub const IDLE_LOCK_MINUTES: i64 = 15;
pub const LOCKOUT_MAX_ATTEMPTS: usize = 5;
pub const LOCKOUT_MINUTES: i64 = 15;

#[derive(Clone)]
pub struct SessionRecord {
    pub token: String,
    pub user: SessionUser,
    pub last_activity: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Default)]
struct SessionStore {
    by_token: HashMap<String, SessionRecord>,
}

#[derive(Clone)]
pub struct SessionManager {
    store: std::sync::Arc<RwLock<SessionStore>>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self {
            store: std::sync::Arc::new(RwLock::new(SessionStore::default())),
        }
    }
}

impl SessionManager {
    pub fn create_session(&self, user: SessionUser) -> SessionRecord {
        let token = Uuid::new_v4().to_string();
        let now = Utc::now();
        let record = SessionRecord {
            token: token.clone(),
            user,
            last_activity: now,
            created_at: now,
        };
        self.store
            .write()
            .by_token
            .insert(token.clone(), record.clone());
        record
    }

    pub fn invalidate(&self, token: &str) {
        self.store.write().by_token.remove(token);
    }

    pub fn invalidate_demo_sessions(&self) {
        self.store.write().by_token.retain(|_, rec| !rec.user.is_demo);
    }

    pub fn touch(&self, token: &str) -> AppResult<SessionRecord> {
        let mut guard = self.store.write();
        let record = guard
            .by_token
            .get_mut(token)
            .ok_or(AppError::SessionInvalide)?;
        let idle = chrono::Duration::minutes(IDLE_LOCK_MINUTES);
        if Utc::now().signed_duration_since(record.last_activity) > idle {
            guard.by_token.remove(token);
            return Err(AppError::SessionInvalide);
        }
        record.last_activity = Utc::now();
        Ok(record.clone())
    }

    pub fn require_user(&self, token: &str) -> AppResult<SessionUser> {
        Ok(self.touch(token)?.user)
    }
}

pub struct AppState {
    pub prod_db: DbPool,
    /// Pool démo remplacé à la réinitialisation.
    pub demo_db: Mutex<DbPool>,
    pub sessions: SessionManager,
}

impl AppState {
    pub fn new(prod_db: DbPool, demo_db: DbPool) -> Self {
        Self {
            prod_db,
            demo_db: Mutex::new(demo_db),
            sessions: SessionManager::default(),
        }
    }

    pub fn require_session(&self, token: &str) -> AppResult<SessionUser> {
        if token.trim().is_empty() {
            return Err(AppError::SessionInvalide);
        }
        self.sessions.require_user(token)
    }

    pub fn with_pool_for_user<R, F>(&self, user: &SessionUser, f: F) -> AppResult<R>
    where
        F: FnOnce(&DbPool) -> AppResult<R>,
    {
        if user.is_demo {
            self.with_demo_pool(f)
        } else {
            f(&self.prod_db)
        }
    }

    pub fn with_demo_pool<R, F>(&self, f: F) -> AppResult<R>
    where
        F: FnOnce(&DbPool) -> AppResult<R>,
    {
        let guard = self.demo_db.lock();
        f(&guard)
    }

    pub fn login(&self, username: &str, password: &str) -> AppResult<SessionRecord> {
        // Connexion admin toujours sur la base de production.
        auth::login(&self.prod_db, &self.sessions, username, password)
    }

    pub fn idle_duration() -> Duration {
        Duration::from_secs((IDLE_LOCK_MINUTES * 60) as u64)
    }
}
