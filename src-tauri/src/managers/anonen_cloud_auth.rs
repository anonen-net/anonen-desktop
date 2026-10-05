use base64::Engine;
use keyring::Entry;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const KEYRING_SERVICE: &str = "anonen-cloud";
const KEYRING_REFRESH_TOKEN: &str = "refresh_token";
const KEYRING_EMAIL: &str = "email";

const REFRESH_SKEW_SECS: u64 = 60;

#[derive(Debug, Clone)]
struct AccessToken {
    token: String,
    expires_at: Instant,
}

impl AccessToken {
    fn is_fresh(&self) -> bool {
        Instant::now() + Duration::from_secs(REFRESH_SKEW_SECS) < self.expires_at
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type)]
pub struct AnonenCloudAuthStatus {
    pub signed_in: bool,
    pub email: Option<String>,

    pub user_id: Option<String>,
}

fn should_clear_rejected_token(sent: &str, stored: Option<&str>) -> bool {
    stored == Some(sent)
}

fn user_id_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let claims: Value = serde_json::from_slice(&decoded).ok()?;
    claims
        .get("sub")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn configured_endpoint(setting: Option<String>) -> String {
    setting
        .map(|url| crate::managers::force_https(&url))
        .unwrap_or_default()
}

fn auth_base(supabase_url: &str) -> Result<&str, String> {
    if supabase_url.is_empty() {
        return Err("Supabase URL is not configured".to_string());
    }
    Ok(supabase_url.trim_end_matches('/'))
}

pub struct AnonenCloudAuthManager {
    supabase_url: String,

    anon_key: String,
    http: reqwest::blocking::Client,
    access: Mutex<Option<AccessToken>>,

    refresh_lock: Mutex<()>,
}

impl AnonenCloudAuthManager {
    pub fn new() -> Self {
        let supabase_url = configured_endpoint(crate::managers::build_time_setting(
            option_env!("ANONEN_SUPABASE_URL"),
            "ANONEN_SUPABASE_URL",
        ));
        let anon_key = crate::managers::build_time_setting(
            option_env!("ANONEN_SUPABASE_ANON_KEY"),
            "ANONEN_SUPABASE_ANON_KEY",
        )
        .unwrap_or_default();

        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build Anonen Cloud auth HTTP client");

        Self {
            supabase_url,
            anon_key,
            http,
            access: Mutex::new(None),
            refresh_lock: Mutex::new(()),
        }
    }

    fn keyring_entry(field: &str) -> Result<Entry, String> {
        Entry::new(KEYRING_SERVICE, field).map_err(|e| format!("keyring init failed: {}", e))
    }

    fn load_refresh_token(&self) -> Option<String> {
        match Self::keyring_entry(KEYRING_REFRESH_TOKEN) {
            Ok(entry) => match entry.get_password() {
                Ok(s) if !s.is_empty() => Some(s),
                Ok(_) => None,
                Err(keyring::Error::NoEntry) => None,
                Err(e) => {
                    warn!("[anonen-cloud-auth] keyring read failed: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("[anonen-cloud-auth] keyring entry failed: {}", e);
                None
            }
        }
    }

    fn store_refresh_token(&self, token: &str) -> Result<(), String> {
        let entry = Self::keyring_entry(KEYRING_REFRESH_TOKEN)?;
        entry
            .set_password(token)
            .map_err(|e| format!("keyring write failed: {}", e))
    }

    fn clear_refresh_token(&self) {
        if let Ok(entry) = Self::keyring_entry(KEYRING_REFRESH_TOKEN) {
            let _ = entry.delete_credential();
        }
    }

    fn store_email(&self, email: &str) {
        if let Ok(entry) = Self::keyring_entry(KEYRING_EMAIL) {
            let _ = entry.set_password(email);
        }
    }

    fn load_email(&self) -> Option<String> {
        Self::keyring_entry(KEYRING_EMAIL)
            .ok()
            .and_then(|e| e.get_password().ok())
            .filter(|s| !s.is_empty())
    }

    fn clear_email(&self) {
        if let Ok(entry) = Self::keyring_entry(KEYRING_EMAIL) {
            let _ = entry.delete_credential();
        }
    }

    pub fn status(&self) -> AnonenCloudAuthStatus {
        let access = self.access.lock().ok().and_then(|guard| guard.clone());
        let has_access = access.as_ref().is_some_and(|a| a.is_fresh());
        let has_refresh = self.load_refresh_token().is_some();
        AnonenCloudAuthStatus {
            signed_in: has_access || has_refresh,
            email: self.load_email(),

            user_id: access.as_ref().and_then(|a| user_id_from_jwt(&a.token)),
        }
    }

    pub fn request_otp(&self, email: &str, captcha_token: Option<&str>) -> Result<(), String> {
        let url = format!("{}/auth/v1/otp", auth_base(&self.supabase_url)?);
        let mut body = serde_json::json!({ "email": email });
        if let Some(token) = captcha_token.filter(|token| !token.is_empty()) {
            body["gotrue_meta_security"] = serde_json::json!({ "captcha_token": token });
        }
        let response = self
            .http
            .post(&url)
            .header("apikey", &self.anon_key)
            .json(&body)
            .send()
            .map_err(|e| format!("OTP request failed: {}", e))?;
        let status = response.status();
        let body = response.text().unwrap_or_default();
        info!("[anonen-cloud-auth] otp status={}", status);
        if !status.is_success() {
            return Err(format!("OTP request failed ({}): {}", status, body));
        }
        Ok(())
    }

    fn consume_session_response(&self, body: &str) -> Result<(), String> {
        let parsed: Value = serde_json::from_str(body)
            .map_err(|e| format!("Failed to parse verify response: {}", e))?;

        let access = parsed
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "verify response missing access_token".to_string())?;
        let refresh = parsed
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "verify response missing refresh_token".to_string())?;
        let expires_in = parsed
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);
        let email = parsed
            .pointer("/user/email")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        self.store_refresh_token(refresh)?;
        if let Some(e) = email.as_deref() {
            self.store_email(e);
        }
        if let Ok(mut guard) = self.access.lock() {
            *guard = Some(AccessToken {
                token: access.to_string(),
                expires_at: Instant::now() + Duration::from_secs(expires_in),
            });
        }
        Ok(())
    }

    pub fn verify_otp(&self, email: &str, token: &str) -> Result<(), String> {
        self.verify_with_payload(email, token, "email")
    }

    fn verify_with_payload(&self, email: &str, token: &str, link_type: &str) -> Result<(), String> {
        auth_base(&self.supabase_url)?;

        let _refresh_guard = self
            .refresh_lock
            .lock()
            .map_err(|_| "token refresh lock poisoned".to_string())?;
        let url = format!("{}/auth/v1/verify", auth_base(&self.supabase_url)?);
        let body_json = serde_json::json!({
            "email": email,
            "token": token,
            "type": link_type,
        });
        let response = self
            .http
            .post(&url)
            .header("apikey", &self.anon_key)
            .json(&body_json)
            .send()
            .map_err(|e| format!("Verify failed: {}", e))?;
        let status = response.status();
        let body = response.text().unwrap_or_default();
        info!(
            "[anonen-cloud-auth] verify type={} status={}",
            link_type, status
        );
        if !status.is_success() {
            return Err(format!("Verify failed ({}): {}", status, body));
        }
        self.consume_session_response(&body)
    }

    fn refresh_access_token(&self, refresh_token: &str) -> Result<AccessToken, String> {
        let url = format!(
            "{}/auth/v1/token?grant_type=refresh_token",
            auth_base(&self.supabase_url)?
        );
        let response = self
            .http
            .post(&url)
            .header("apikey", &self.anon_key)
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .map_err(refresh_not_sent)?;
        let status = response.status();
        let body = response.text().unwrap_or_default();
        info!("[anonen-cloud-auth] refresh status={}", status);
        if !status.is_success() {
            if refresh_token_is_dead(status.as_u16()) {
                if should_clear_rejected_token(refresh_token, self.load_refresh_token().as_deref())
                {
                    warn!(
                        "[anonen-cloud-auth] refresh_token rejected ({}); clearing it",
                        status
                    );
                    self.clear_refresh_token();
                } else {
                    warn!(
                        "[anonen-cloud-auth] refresh_token rejected ({}), but a newer one is stored; keeping it",
                        status
                    );
                }
            }
            return Err(refresh_refused(status.as_u16(), &body));
        }
        let parsed: Value = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse refresh response: {}", e))?;

        let access = parsed
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "refresh response missing access_token".to_string())?;
        let new_refresh = parsed
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "refresh response missing refresh_token".to_string())?;
        let expires_in = parsed
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        self.store_refresh_token(new_refresh)?;
        Ok(AccessToken {
            token: access.to_string(),
            expires_at: Instant::now() + Duration::from_secs(expires_in),
        })
    }

    pub fn get_access_token(&self) -> Result<String, String> {
        if let Ok(guard) = self.access.lock() {
            if let Some(at) = guard.as_ref() {
                if at.is_fresh() {
                    return Ok(at.token.clone());
                }
            }
        }

        let _refresh_guard = self
            .refresh_lock
            .lock()
            .map_err(|_| "token refresh lock poisoned".to_string())?;
        if let Ok(guard) = self.access.lock() {
            if let Some(at) = guard.as_ref() {
                if at.is_fresh() {
                    return Ok(at.token.clone());
                }
            }
        }

        let refresh = self
            .load_refresh_token()
            .ok_or_else(|| NOT_SIGNED_IN.to_string())?;
        let new_token = self.refresh_access_token(&refresh)?;
        let token_str = new_token.token.clone();
        if let Ok(mut guard) = self.access.lock() {
            *guard = Some(new_token);
        }
        Ok(token_str)
    }

    pub fn invalidate_access_token(&self) {
        if let Ok(mut guard) = self.access.lock() {
            *guard = None;
        }
    }

    pub fn logout(&self) {
        if let (Ok(base), Ok(token)) = (auth_base(&self.supabase_url), self.get_access_token()) {
            let url = format!("{base}/auth/v1/logout?scope=local");
            match self
                .http
                .post(&url)
                .header("apikey", &self.anon_key)
                .bearer_auth(token)
                .send()
            {
                Ok(resp) => info!(
                    "[anonen-cloud-auth] server-side logout status={}",
                    resp.status()
                ),
                Err(e) => warn!(
                    "[anonen-cloud-auth] server-side logout skipped (offline?): {}",
                    e
                ),
            }
        }
        self.clear_refresh_token();
        self.clear_email();
        if let Ok(mut guard) = self.access.lock() {
            *guard = None;
        }
    }
}

const NOT_SIGNED_IN: &str = "Not signed in to あのねん (no refresh_token). Sign in to continue.";
const REFRESH_NOT_SENT: &str = "token refresh failed: ";
const REFRESH_REFUSED: &str = "token refresh failed (";

fn refresh_not_sent(error: impl std::fmt::Display) -> String {
    format!("{REFRESH_NOT_SENT}{error}")
}

fn refresh_refused(status: u16, body: &str) -> String {
    format!("{REFRESH_REFUSED}{status}): {body}")
}

fn refresh_token_is_dead(status: u16) -> bool {
    status == 400 || status == 401
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenFailure {
    Transport,

    SignedOut,

    Other,
}

pub fn token_failure_without_body(message: &str) -> String {
    let status = message
        .strip_prefix(REFRESH_REFUSED)
        .and_then(|rest| rest.split_once("): "))
        .map(|(status, _body)| status)
        .filter(|status| status.parse::<u16>().is_ok());
    match status {
        Some(status) => format!("{REFRESH_REFUSED}{status})"),
        None => message.to_string(),
    }
}

pub fn token_failure(message: &str) -> TokenFailure {
    if message == NOT_SIGNED_IN {
        return TokenFailure::SignedOut;
    }
    if let Some(rest) = message.strip_prefix(REFRESH_REFUSED) {
        let dead = rest
            .split_once("): ")
            .and_then(|(status, _body)| status.parse::<u16>().ok())
            .is_some_and(refresh_token_is_dead);
        return if dead {
            TokenFailure::SignedOut
        } else {
            TokenFailure::Other
        };
    }
    if message.starts_with(REFRESH_NOT_SENT) {
        return TokenFailure::Transport;
    }
    TokenFailure::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_with_no_endpoint_configured_has_no_default_host() {
        assert_eq!("", configured_endpoint(None));
        assert_eq!(
            "https://abc.supabase.co",
            configured_endpoint(Some("https://abc.supabase.co".to_string()))
        );

        assert_eq!(
            "https://abc.supabase.co",
            configured_endpoint(Some("http://abc.supabase.co".to_string()))
        );
    }

    #[test]
    fn no_sign_in_request_can_be_built_without_an_endpoint() {
        assert_eq!(
            Err("Supabase URL is not configured".to_string()),
            auth_base(&configured_endpoint(None))
        );
        assert_eq!(
            Ok("https://abc.supabase.co"),
            auth_base("https://abc.supabase.co/")
        );
    }

    #[test]
    fn a_refresh_that_got_no_response_is_told_apart_from_a_refused_one() {
        assert_eq!(
            token_failure(&refresh_not_sent(
                "error sending request for url (https://x.supabase.co/auth/v1/token)"
            )),
            TokenFailure::Transport
        );
        assert_eq!(token_failure(NOT_SIGNED_IN), TokenFailure::SignedOut);
        for status in [400, 401] {
            assert_eq!(
                token_failure(&refresh_refused(status, "{}")),
                TokenFailure::SignedOut,
                "{status}"
            );
        }
    }

    #[test]
    fn a_failing_auth_server_is_not_a_dead_session() {
        for status in [403, 408, 429, 500, 502, 503, 504] {
            assert_eq!(
                token_failure(&refresh_refused(status, "{}")),
                TokenFailure::Other,
                "{status}"
            );
        }
        for message in [
            "token refresh lock poisoned",
            "Failed to parse refresh response: x",
            "refresh response missing access_token",
            "refresh response missing refresh_token",
            "keyring write failed: x",
            "Supabase URL is not configured",
            "",
        ] {
            assert_eq!(token_failure(message), TokenFailure::Other, "{message}");
        }
    }

    #[test]
    fn the_token_is_forgotten_on_exactly_the_statuses_read_as_signed_out() {
        for status in 100..600u16 {
            assert_eq!(
                token_failure(&refresh_refused(status, "{}")) == TokenFailure::SignedOut,
                refresh_token_is_dead(status),
                "{status}"
            );
        }
        assert!(refresh_token_is_dead(400) && refresh_token_is_dead(401));
        let production = include_str!("anonen_cloud_auth.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the file has a production part");
        assert!(
            production.contains("if refresh_token_is_dead(status.as_u16()) {"),
            "refresh_access_token must decide with refresh_token_is_dead"
        );
    }

    #[test]
    fn the_shown_form_of_a_refused_refresh_leaves_the_body_out() {
        for (status, body) in [
            (503, "認証に失敗しました。再度サインインしてください"),
            (
                429,
                "<!DOCTYPE html><html><head><title>Just a moment...</title>",
            ),
            (400, "{\"error\":\"invalid_grant\"}"),
            (500, ""),
            (502, "): token refresh failed: x"),
        ] {
            assert_eq!(
                token_failure_without_body(&refresh_refused(status, body)),
                format!("token refresh failed ({status})"),
                "{body}"
            );
        }

        for own in [
            NOT_SIGNED_IN,
            "token refresh lock poisoned",
            "Failed to parse refresh response: expected value at line 1 column 1",
            "Supabase URL is not configured",
            "token refresh failed: error sending request for url (https://x/)",
            "",
        ] {
            assert_eq!(token_failure_without_body(own), own);
        }
    }

    #[test]
    fn a_refused_refresh_is_read_by_its_status_never_by_its_body() {
        for body in [
            "token refresh failed: error sending request",
            "): token refresh failed: x",
            "network_error",
            "400): invalid_grant",
            "token refresh failed (401): x",
            NOT_SIGNED_IN,
        ] {
            assert_eq!(
                token_failure(&refresh_refused(503, body)),
                TokenFailure::Other,
                "{body}"
            );
            assert_eq!(
                token_failure(&refresh_refused(400, body)),
                TokenFailure::SignedOut,
                "{body}"
            );
        }
    }

    #[test]
    fn every_sign_in_url_goes_through_the_endpoint_check() {
        let source = include_str!("anonen_cloud_auth.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("the file has production code");
        assert_eq!(
            0,
            production
                .matches("self.supabase_url.trim_end_matches")
                .count(),
            "build the URL from auth_base(&self.supabase_url)"
        );

        assert_eq!(
            5,
            production.matches("auth_base(&self.supabase_url)").count()
        );
    }

    #[test]
    fn a_rejected_token_is_cleared_when_it_is_still_the_stored_one() {
        assert!(should_clear_rejected_token("old", Some("old")));
    }

    #[test]
    fn a_rejected_token_does_not_take_a_newer_sign_in_with_it() {
        assert!(!should_clear_rejected_token("old", Some("brand-new")));
    }

    #[test]
    fn nothing_is_cleared_when_nothing_is_stored() {
        assert!(!should_clear_rejected_token("old", None));
    }

    fn jwt_with_payload(payload: &str) -> String {
        let encode =
            |s: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s.as_bytes());
        format!(
            "{}.{}.not-a-real-signature",
            encode(r#"{"alg":"HS256","typ":"JWT"}"#),
            encode(payload)
        )
    }

    #[test]
    fn user_id_from_jwt_reads_sub() {
        let token = jwt_with_payload(
            r#"{"sub":"3f9a1c2b-4d5e-6f70-8a9b-0c1d2e3f4a5b","email":"user@example.com"}"#,
        );
        assert_eq!(
            user_id_from_jwt(&token).as_deref(),
            Some("3f9a1c2b-4d5e-6f70-8a9b-0c1d2e3f4a5b")
        );
    }

    #[test]
    fn user_id_from_jwt_returns_none_when_unusable() {
        assert_eq!(user_id_from_jwt("not-a-jwt"), None);
        assert_eq!(user_id_from_jwt("header.@@@@.signature"), None);
        assert_eq!(
            user_id_from_jwt(&jwt_with_payload(r#"{"email":"user@example.com"}"#)),
            None
        );
        assert_eq!(user_id_from_jwt(&jwt_with_payload(r#"{"sub":""}"#)), None);
    }
}
