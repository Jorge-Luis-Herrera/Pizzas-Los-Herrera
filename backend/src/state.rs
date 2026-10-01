use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sea_orm::DatabaseConnection;
use tokio::sync::Mutex;

const TOKEN_TTL: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub admin_user: String,
    pub admin_password: String,
    pub tokens: Arc<Mutex<HashMap<String, Instant>>>,
}

impl AppState {
    pub fn new(db: DatabaseConnection) -> Self {
        let admin_user = std::env::var("ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        let admin_password =
            std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin".to_string());

        if admin_password == "admin" {
            eprintln!(
                "⚠️  ADMIN_PASSWORD no configurado; usando 'admin'. Cámbialo en producción."
            );
        }

        Self {
            db,
            admin_user,
            admin_password,
            tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn issue_token(&self) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let mut tokens = self.tokens.lock().await;
        Self::purge_expired(&mut tokens);
        tokens.insert(token.clone(), Instant::now());
        token
    }

    pub async fn revoke_token(&self, token: &str) {
        let mut tokens = self.tokens.lock().await;
        tokens.remove(token);
    }

    pub async fn is_token_valid(&self, token: &str) -> bool {
        let mut tokens = self.tokens.lock().await;
        Self::purge_expired(&mut tokens);
        match tokens.get(token) {
            Some(issued_at) if issued_at.elapsed() < TOKEN_TTL => true,
            Some(_) => {
                tokens.remove(token);
                false
            }
            None => false,
        }
    }

    fn purge_expired(tokens: &mut HashMap<String, Instant>) {
        tokens.retain(|_, issued_at| issued_at.elapsed() < TOKEN_TTL);
    }
}
