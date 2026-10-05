use crate::actions::{process_transcription_output, record_transcription_stats};
use crate::managers::{
    anonen_cloud_auth::AnonenCloudAuthManager,
    history::{HistoryAudioUsage, HistoryManager, PaginatedHistory},
    transcription::TranscriptionManager,
};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
#[specta::specta]
pub async fn get_history_entries(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    cursor: Option<i64>,
    limit: Option<usize>,

    saved_only: bool,
) -> Result<PaginatedHistory, String> {
    history_manager
        .get_history_entries(cursor, limit, saved_only)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_history_entry_saved(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .toggle_saved_status(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_audio_file_path(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    file_name: String,
) -> Result<String, String> {
    let path = history_manager.get_audio_file_path(&file_name);
    path.to_str()
        .ok_or_else(|| "Invalid file path".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_history_entry(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .delete_entry(id)
        .await
        .map_err(|e| e.to_string())
}

fn ensure_retranscribe_allowed(app: &AppHandle) -> Result<(), String> {
    if !app.state::<AnonenCloudAuthManager>().status().signed_in {
        return Err("Sign in required. Open Settings → Models and sign in first.".to_string());
    }
    if !crate::managers::usage::has_active_subscription(app) {
        return Err("有効な契約がありません。Webでご契約ください。".to_string());
    }
    Ok(())
}

async fn retry_history_entry_inner(
    app: &AppHandle,
    history_manager: &Arc<HistoryManager>,
    transcription_manager: &Arc<TranscriptionManager>,
    id: i64,
    force_post_process: bool,
    opus_override: Option<bool>,
) -> Result<(), String> {
    ensure_retranscribe_allowed(app)?;

    let entry = history_manager
        .get_entry_by_id(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))?;

    let audio_path = history_manager.get_audio_file_path(&entry.file_name);
    let samples = crate::audio_toolkit::read_wav_samples(&audio_path)
        .map_err(|e| format!("Failed to load audio: {}", e))?;

    if samples.is_empty() {
        return Err("Recording has no audio samples".to_string());
    }

    transcription_manager.initiate_model_load();

    transcription_manager.warm_connection();

    let tm = Arc::clone(transcription_manager);

    let transcription_time = Instant::now();
    let transcription = tauri::async_runtime::spawn_blocking(move || {
        tm.transcribe_with_options(
            samples,
            opus_override,
            crate::actions::Cancellation::Independent,
        )
    })
    .await
    .map_err(|e| format!("Transcription task panicked: {}", e))?
    .map_err(|e| {
        crate::actions::raise_own_window_notice(app, &crate::cloud_failure::failure_of(&e), true);

        crate::cloud_failure::retry_failure_reply(&e)
    })?;
    let transcribe_ms = transcription_time.elapsed().as_millis();

    if transcription.is_empty() {
        return Err("Recording contains no speech".to_string());
    }

    record_transcription_stats(app, transcribe_ms);

    let post_process = force_post_process || entry.post_process_requested;
    let processed = process_transcription_output(app, &transcription, post_process).await;
    history_manager
        .update_transcription(
            id,
            transcription,
            processed.post_processed_text,
            processed.post_process_prompt,
            transcription_manager.last_model(),
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn retry_history_entry_transcription(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    id: i64,
) -> Result<(), String> {
    retry_history_entry_inner(
        &app,
        history_manager.inner(),
        transcription_manager.inner(),
        id,
        false,
        None,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn retry_history_entry_transcription_with_post_process(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    id: i64,
) -> Result<(), String> {
    retry_history_entry_inner(
        &app,
        history_manager.inner(),
        transcription_manager.inner(),
        id,
        true,
        None,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn retry_history_entry_with_format(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    id: i64,
    use_opus: bool,
) -> Result<(), String> {
    retry_history_entry_inner(
        &app,
        history_manager.inner(),
        transcription_manager.inner(),
        id,
        false,
        Some(use_opus),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_history_audio_usage(
    history_manager: State<'_, Arc<HistoryManager>>,
) -> Result<HistoryAudioUsage, String> {
    history_manager.audio_usage().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_history_retention(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    retention: String,
) -> Result<(), String> {
    use crate::settings::HistoryRetention;

    let retention = match retention.as_str() {
        "none" => HistoryRetention::None,
        "recent" => HistoryRetention::Recent,
        "unlimited" => HistoryRetention::Unlimited,
        other => return Err(format!("Invalid history retention: {}", other)),
    };

    let mut settings = crate::settings::get_settings(&app);
    settings.history_retention = Some(retention);

    settings.history_enabled = None;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn clear_all_history(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
) -> Result<usize, String> {
    history_manager
        .clear_all_entries()
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn prefetch_enclave_key(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<(), String> {
    transcription_manager.prefetch_enclave_key();
    Ok(())
}
