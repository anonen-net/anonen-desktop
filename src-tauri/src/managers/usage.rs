use crate::managers::anonen_cloud_auth::AnonenCloudAuthManager;
use crate::remote_asr::AsrGatewayUsage;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

const USAGE_UPDATE_EVENT: &str = "anonen-cloud-usage-update";

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type)]
pub struct AnonenCloudPlan {
    pub name: String,
    pub week_cap_s: i64,
    pub month_cap_s: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type)]
pub struct AnonenCloudSubscription {
    pub status: String,
    pub current_period_end: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type)]
pub struct AnonenCloudUsageSnapshot {
    pub usage: AsrGatewayUsage,
    pub plan: Option<AnonenCloudPlan>,
    pub subscription: Option<AnonenCloudSubscription>,
}

pub struct UsageManager {
    http: reqwest::blocking::Client,
    last: Mutex<Option<AnonenCloudUsageSnapshot>>,
}

impl UsageManager {
    pub fn new() -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to build usage HTTP client");
        Self {
            http,
            last: Mutex::new(None),
        }
    }

    pub fn current(&self) -> Option<AnonenCloudUsageSnapshot> {
        self.last.lock().ok().and_then(|g| g.clone())
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.last.lock() {
            *guard = None;
        }
    }

    pub fn update_from_transcribe(&self, app: &AppHandle, usage: AsrGatewayUsage) {
        let mut snapshot = AnonenCloudUsageSnapshot {
            usage,
            plan: None,
            subscription: None,
        };
        if let Ok(mut guard) = self.last.lock() {
            if let Some(prev) = guard.as_ref() {
                snapshot.plan = prev.plan.clone();
                snapshot.subscription = prev.subscription.clone();
            }
            *guard = Some(snapshot.clone());
        }
        let _ = app.emit(USAGE_UPDATE_EVENT, &snapshot);
    }

    pub fn fetch(
        &self,
        app: &AppHandle,
        base_url: &str,
    ) -> Result<AnonenCloudUsageSnapshot, String> {
        let auth = app.state::<AnonenCloudAuthManager>();

        let asked_for = auth.status().email;
        let access = auth.get_access_token()?;
        let url = format!("{}/v1/usage", base_url.trim_end_matches('/'));
        let response = crate::remote_asr::identify(self.http.get(&url))
            .bearer_auth(&access)
            .send()
            .map_err(|e| format!("usage request failed: {}", e))?;
        let status = response.status();
        let body = response.text().unwrap_or_default();
        info!("[anonen-cloud-usage] fetch status={}", status);

        let now = auth.status();
        if !keep_usage_reply(asked_for.as_deref(), now.signed_in, now.email.as_deref()) {
            info!("[anonen-cloud-usage] account changed during the fetch; result discarded");
            return Err("account_changed".to_string());
        }
        if !status.is_success() {
            if status == reqwest::StatusCode::PAYMENT_REQUIRED {
                clear_cached_entitlement(app);
                self.clear();
                let _ = app.emit("anonen-cloud-checkout-required", serde_json::json!({}));
                return Err("no_active_subscription".to_string());
            }

            if status == reqwest::StatusCode::UNAUTHORIZED {
                auth.invalidate_access_token();
            }

            return Err(format!(
                "usage request failed ({}): {}",
                status,
                crate::utils::quoted_body(&body)
            ));
        }
        let parsed: Value = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse usage response: {}", e))?;
        let snapshot = AnonenCloudUsageSnapshot {
            usage: serde_json::from_value(parsed.get("usage").cloned().ok_or_else(|| {
                format!(
                    "usage response missing 'usage': {}",
                    crate::utils::quoted_body(&body)
                )
            })?)
            .map_err(|e| format!("Invalid usage block: {}", e))?,
            plan: parsed
                .get("plan")
                .and_then(|p| serde_json::from_value(p.clone()).ok()),
            subscription: parsed
                .get("subscription")
                .and_then(|s| serde_json::from_value(s.clone()).ok()),
        };

        match (
            snapshot.subscription.as_ref(),
            auth.status().email.as_deref(),
        ) {
            (Some(sub), Some(email)) => store_cached_entitlement(app, email, &sub.status),
            _ => clear_cached_entitlement(app),
        }
        if let Ok(mut guard) = self.last.lock() {
            *guard = Some(snapshot.clone());
        }
        let _ = app.emit(USAGE_UPDATE_EVENT, &snapshot);
        Ok(snapshot)
    }
}

const ENTITLEMENT_STORE_PATH: &str = "entitlement_store.json";
const ENTITLEMENT_KEY: &str = "last_known_subscription";

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CachedEntitlement {
    email: String,

    status: String,

    updated_at: String,
}

pub fn is_active_subscription_status(status: &str) -> bool {
    matches!(status, "trialing" | "active" | "past_due")
}

fn keep_usage_reply(asked_for: Option<&str>, signed_in_now: bool, now: Option<&str>) -> bool {
    signed_in_now
        && match (asked_for, now) {
            (Some(a), Some(b)) => a == b,
            (None, None) => true,
            _ => false,
        }
}

fn entitlement_store(
    app: &AppHandle,
) -> Option<std::sync::Arc<tauri_plugin_store::Store<tauri::Wry>>> {
    match app.store(crate::portable::store_path(ENTITLEMENT_STORE_PATH)) {
        Ok(store) => Some(store),
        Err(e) => {
            warn!("[anonen-cloud-usage] entitlement store unavailable: {}", e);
            None
        }
    }
}

pub fn store_cached_entitlement(app: &AppHandle, email: &str, status: &str) {
    if let Some(store) = entitlement_store(app) {
        let cached = CachedEntitlement {
            email: email.to_string(),
            status: status.to_string(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        match serde_json::to_value(&cached) {
            Ok(value) => store.set(ENTITLEMENT_KEY, value),
            Err(e) => warn!(
                "[anonen-cloud-usage] failed to serialize entitlement: {}",
                e
            ),
        }
    }
}

pub fn clear_cached_entitlement(app: &AppHandle) {
    if let Some(store) = entitlement_store(app) {
        store.delete(ENTITLEMENT_KEY);
    }
}

pub fn cached_entitlement_status(app: &AppHandle) -> Option<String> {
    let email = app.state::<AnonenCloudAuthManager>().status().email?;
    let store = entitlement_store(app)?;
    let cached: CachedEntitlement = serde_json::from_value(store.get(ENTITLEMENT_KEY)?).ok()?;
    if !cached.email.eq_ignore_ascii_case(&email) {
        return None;
    }
    Some(cached.status)
}

pub const FALLBACK_MAX_RECORDING_SECONDS: u64 = 240;

pub fn max_recording_seconds(app: &AppHandle) -> u64 {
    if let Some(usage_mgr) = app.try_state::<UsageManager>() {
        if let Some(snapshot) = usage_mgr.current() {
            if let Some(v) = snapshot.usage.max_request_s.filter(|v| *v > 0) {
                return v as u64;
            }
        }
    }
    FALLBACK_MAX_RECORDING_SECONDS
}

pub fn has_active_subscription(app: &AppHandle) -> bool {
    if let Some(usage_mgr) = app.try_state::<UsageManager>() {
        if let Some(snapshot) = usage_mgr.current() {
            if let Some(sub) = snapshot.subscription.as_ref() {
                return is_active_subscription_status(&sub.status);
            }
        }
    }
    cached_entitlement_status(app)
        .map(|s| is_active_subscription_status(&s))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::keep_usage_reply;

    #[test]
    fn a_usage_reply_is_kept_only_for_the_account_that_asked() {
        let (a, b) = (Some("a@example.com"), Some("b@example.com"));
        assert!(keep_usage_reply(a, true, a));

        assert!(!keep_usage_reply(a, true, b));

        assert!(!keep_usage_reply(a, false, None));
        assert!(!keep_usage_reply(None, false, None));

        assert!(!keep_usage_reply(None, true, b));
        assert!(!keep_usage_reply(a, true, None));
    }

    #[test]
    fn the_account_check_runs_before_any_reply_touches_the_state() {
        let src = include_str!("usage.rs");
        let body = &src[src.find("pub fn fetch(").expect("fetch is in usage.rs")..];
        let check = body
            .find("keep_usage_reply(")
            .expect("fetch checks the account");
        let payment = body.find("PAYMENT_REQUIRED").expect("fetch handles 402");
        assert!(
            check < payment,
            "the account check must come before the 402/401 branches"
        );
    }

    #[test]
    fn an_unknown_email_on_both_ends_does_not_lock_out_the_signed_in_user() {
        assert!(keep_usage_reply(None, true, None));
    }
}
