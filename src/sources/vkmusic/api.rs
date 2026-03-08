use std::sync::Arc;

use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, error};

const VK_API: &str = "https://api.vk.com/method";
const VK_VERSION: &str = "5.199";
const API_UA: &str = "KateMobileAndroid/56 lite-460 (Android 4.4.2; SDK 19; x86; unknown Android SDK built for x86; en)";
const WEB_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0";

pub struct VkApiClient {
    pub client: Arc<reqwest::Client>,
    pub token: Arc<RwLock<Option<String>>>,
    pub user_id: Arc<RwLock<i64>>,
    pub cookie: Option<String>,
}

impl VkApiClient {
    pub fn new(
        client: Arc<reqwest::Client>,
        initial_token: Option<String>,
        cookie: Option<String>,
    ) -> Self {
        Self {
            client,
            token: Arc::new(RwLock::new(initial_token)),
            user_id: Arc::new(RwLock::new(0)),
            cookie,
        }
    }

    pub async fn call(&self, method: &str, params: &[(&'static str, String)]) -> Option<Value> {
        let token = self.acquire_token().await?;
        let body = self.do_request(method, &token, params).await?;

        if let Some(code) = body["error"]["error_code"].as_i64() {
            if code == 5 && self.cookie.is_some() {
                *self.token.write().await = None;
                let fresh = self.refresh_token().await?;
                return self
                    .do_request(method, &fresh, params)
                    .await
                    .map(|b| b["response"].clone());
            }
            error!(
                "VK API {}: {} — {}",
                method,
                code,
                body["error"]["error_msg"].as_str().unwrap_or("")
            );
            return None;
        }

        Some(body["response"].clone())
    }

    async fn do_request(
        &self,
        method: &str,
        token: &str,
        params: &[(&'static str, String)],
    ) -> Option<Value> {
        let mut url = format!(
            "{}/{}?access_token={}&v={}",
            VK_API, method, token, VK_VERSION
        );
        for (k, v) in params {
            url.push('&');
            url.push_str(k);
            url.push('=');
            url.push_str(&urlencoding::encode(v));
        }

        let resp = self
            .client
            .get(&url)
            .header("User-Agent", API_UA)
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            debug!("VK API {} -> HTTP {}", method, resp.status());
            return None;
        }

        resp.json().await.ok()
    }

    async fn acquire_token(&self) -> Option<String> {
        let guard = self.token.read().await;
        if let Some(t) = guard.as_ref() {
            return Some(t.clone());
        }
        drop(guard);
        self.refresh_token().await
    }

    pub async fn refresh_token(&self) -> Option<String> {
        let cookie = self.cookie.as_deref()?;
        debug!("VK Music: refreshing token");

        let resp = self
            .client
            .post("https://login.vk.ru/?act=web_token")
            .header("User-Agent", WEB_UA)
            .header("Referer", "https://vk.ru/")
            .header("Origin", "https://vk.ru")
            .header("Cookie", cookie)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body("version=1&app_id=6287487")
            .send()
            .await
            .ok()?;

        let body: Value = resp.json().await.ok()?;

        if body.get("type").and_then(|t| t.as_str()) != Some("okay") {
            error!(
                "VK Music token refresh failed: {}",
                body.get("error_info")
                    .and_then(|e| e.as_str())
                    .unwrap_or("unknown")
            );
            return None;
        }

        let data = &body["data"];
        let token = data["access_token"].as_str()?.to_string();
        let uid = data["user_id"].as_i64().unwrap_or(0);

        *self.token.write().await = Some(token.clone());
        *self.user_id.write().await = uid;

        Some(token)
    }
}
