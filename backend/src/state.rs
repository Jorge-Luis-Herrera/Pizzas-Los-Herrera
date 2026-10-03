use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sea_orm::DatabaseConnection;
use sha2::Sha256;
use tokio::sync::Mutex;

type HmacSha256 = Hmac<Sha256>;

const TOKEN_TTL: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub admin_user: String,
    pub admin_password: String,
    /// Clave con la que se firman los tokens de sesión.
    token_secret: Arc<Vec<u8>>,
    /// Tokens revocados antes de expirar (logout). Permite cerrar sesión
    /// sin volver a reiniciar el servidor.
    revoked: Arc<Mutex<HashMap<String, Instant>>>,
    /// Contador de intentos fallidos de login por clave (IP o usuario).
    login_failures: Arc<Mutex<HashMap<String, LoginAttempts>>>,
}

#[derive(Clone)]
struct LoginAttempts {
    count: u32,
    first_at: Instant,
}

/// Ventana y máximo de intentos fallidos antes de bloquear el login.
const LOGIN_WINDOW: Duration = Duration::from_secs(300);
const LOGIN_MAX_ATTEMPTS: u32 = 5;

impl AppState {
    pub fn new(db: DatabaseConnection) -> Self {
        let admin_user = std::env::var("ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        let admin_password =
            std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin".to_string());

        if admin_password == "admin" {
            tracing::warn!(
                "ADMIN_PASSWORD no configurado; se está usando 'admin'. Cámbialo en producción."
            );
        }

        // La clave de firma se deriva de la contraseña del admin para no
        // introducir una variable de entorno más que recordar en producción.
        let token_secret = Arc::new(
            format!("{}-{}", admin_user, admin_password)
                .as_bytes()
                .to_vec(),
        );

        Self {
            db,
            admin_user,
            admin_password,
            token_secret,
            revoked: Arc::new(Mutex::new(HashMap::new())),
            login_failures: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Comprueba si un login está bloqueado por intentos fallidos recientes.
    /// Devuelve los segundos restantes de bloqueo si lo está.
    pub async fn login_lockout(&self, key: &str) -> Option<u64> {
        let mut failures = self.login_failures.lock().await;
        let now = Instant::now();

        failures.retain(|_, v| now.duration_since(v.first_at) < LOGIN_WINDOW);

        match failures.get(key) {
            Some(v) if v.count >= LOGIN_MAX_ATTEMPTS => {
                let elapsed = now.duration_since(v.first_at);
                Some(LOGIN_WINDOW.saturating_sub(elapsed).as_secs() + 1)
            }
            _ => None,
        }
    }

    pub async fn record_login_failure(&self, key: &str) {
        let mut failures = self.login_failures.lock().await;
        let now = Instant::now();
        failures.retain(|_, v| now.duration_since(v.first_at) < LOGIN_WINDOW);

        let entry = failures.entry(key.to_string()).or_insert(LoginAttempts {
            count: 0,
            first_at: now,
        });
        entry.count += 1;
    }

    pub async fn clear_login_failures(&self, key: &str) {
        self.login_failures.lock().await.remove(key);
    }

    /// Emite un token firmado: `<expiry_epoch>|<id_aleatorio>|<firma>`.
    /// Al ser firmado y llevar su propia caducidad, sobrevive a reinicios
    /// del contenedor (Azure Container Apps escala a cero).
    pub fn issue_token(&self) -> String {
        let expiry = chrono::Utc::now().timestamp() + TOKEN_TTL.as_secs() as i64;
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let payload = format!("{}|{}", expiry, nonce);
        let signature = self.sign(&payload);
        format!("{}|{}|{}", expiry, nonce, URL_SAFE_NO_PAD.encode(signature))
    }

    fn sign(&self, payload: &str) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(&self.token_secret)
            .expect("HMAC acepta claves de cualquier tamaño");
        mac.update(payload.as_bytes());
        mac.finalize().into_bytes().to_vec()
    }

    /// Verifica firma y caducidad. Sin estado: solo necesita la clave.
    pub fn verify_token(&self, token: &str) -> bool {
        let mut parts = token.split('|');
        let (expiry, nonce, signature_b64) = match (parts.next(), parts.next(), parts.next()) {
            (Some(e), Some(n), Some(s)) => (e, n, s),
            _ => return false,
        };
        if parts.next().is_some() {
            return false;
        }

        let expected = URL_SAFE_NO_PAD.encode(self.sign(&format!("{}|{}", expiry, nonce)));
        if !constant_time_eq(expected.as_bytes(), signature_b64.as_bytes()) {
            return false;
        }

        match expiry.parse::<i64>() {
            Ok(exp) => chrono::Utc::now().timestamp() < exp,
            Err(_) => false,
        }
    }

    /// Registra un token como revocado hasta que venza.
    pub async fn revoke_token(&self, token: &str) {
        let expiry = token
            .split('|')
            .next()
            .and_then(|e| e.parse::<i64>().ok())
            .unwrap_or_else(|| chrono::Utc::now().timestamp() + 60);
        let ttl = Duration::from_secs((expiry - chrono::Utc::now().timestamp()).max(0) as u64);
        self.revoked
            .lock()
            .await
            .insert(token.to_string(), Instant::now() + ttl);
    }

    pub async fn is_token_valid(&self, token: &str) -> bool {
        if !self.verify_token(token) {
            return false;
        }
        let mut revoked = self.revoked.lock().await;
        let now = Instant::now();
        revoked.retain(|_, exp| *exp > now);
        !revoked.contains_key(token)
    }
}

/// Comparación en tiempo constante para no filtrar información por tiempos.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
