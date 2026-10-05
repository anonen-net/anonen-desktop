use crate::managers::anonen_cloud_auth::{AnonenCloudAuthManager, AnonenCloudAuthStatus};
use crate::managers::model::ModelManager;
use crate::managers::usage::{AnonenCloudUsageSnapshot, UsageManager};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[tauri::command]
#[specta::specta]
pub fn anonen_cloud_auth_status(app: AppHandle) -> Result<AnonenCloudAuthStatus, String> {
    Ok(app.state::<AnonenCloudAuthManager>().status())
}

#[tauri::command]
#[specta::specta]
pub async fn anonen_cloud_request_otp(
    app: AppHandle,
    email: String,
    captcha_token: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        app.state::<AnonenCloudAuthManager>()
            .request_otp(&email, captcha_token.as_deref())
    })
    .await
    .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
#[specta::specta]
pub async fn anonen_cloud_verify_otp(
    app: AppHandle,
    email: String,
    token: String,
) -> Result<AnonenCloudAuthStatus, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<AnonenCloudAuthStatus, String> {
        let manager = app.state::<AnonenCloudAuthManager>();
        manager.verify_otp(&email, &token)?;

        crate::managers::usage::clear_cached_entitlement(&app);
        app.state::<UsageManager>().clear();
        Ok(manager.status())
    })
    .await
    .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
#[specta::specta]
pub async fn anonen_cloud_logout(app: AppHandle) -> Result<AnonenCloudAuthStatus, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<AnonenCloudAuthStatus, String> {
        let manager = app.state::<AnonenCloudAuthManager>();
        manager.logout();

        crate::managers::usage::clear_cached_entitlement(&app);
        app.state::<UsageManager>().clear();
        Ok(manager.status())
    })
    .await
    .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
#[specta::specta]
pub fn anonen_cloud_current_usage(
    app: AppHandle,
) -> Result<Option<AnonenCloudUsageSnapshot>, String> {
    Ok(app.state::<UsageManager>().current())
}

#[tauri::command]
#[specta::specta]
pub fn anonen_cloud_cached_subscription_status(app: AppHandle) -> Result<Option<String>, String> {
    Ok(crate::managers::usage::cached_entitlement_status(&app))
}

#[tauri::command]
#[specta::specta]
pub async fn anonen_cloud_fetch_models(
    app: AppHandle,
) -> Result<Vec<crate::remote_asr::AsrGatewayModel>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let model_manager = app.state::<Arc<ModelManager>>();
        let base_url = model_manager.anonen_cloud_base_url();
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("HTTP client build failed: {}", e))?;
        let models = crate::remote_asr::fetch_gateway_models(&client, &base_url)?;
        model_manager.register_cloud_models(&models);
        Ok(models)
    })
    .await
    .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
#[specta::specta]
pub async fn anonen_cloud_fetch_usage(app: AppHandle) -> Result<AnonenCloudUsageSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<AnonenCloudUsageSnapshot, String> {
        let model_manager = app.state::<Arc<ModelManager>>();
        let base_url = model_manager.anonen_cloud_base_url();
        app.state::<UsageManager>().fetch(&app, &base_url)
    })
    .await
    .map_err(|e| format!("task join failed: {}", e))?
}
