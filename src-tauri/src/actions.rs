use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::audio_toolkit::{is_microphone_access_denied, is_no_input_device_error};
use crate::managers::anonen_cloud_auth::AnonenCloudAuthManager;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::HistoryManager;
use crate::managers::model::ModelManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, AppSettings};
use crate::shortcut;
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{
    self, show_preparing_overlay, show_processing_overlay, show_recording_overlay,
    show_transcribing_overlay,
};
use crate::{show_main_window, TranscriptionCoordinator};
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::{debug, error, warn};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Manager;
use tauri::{AppHandle, Emitter};

const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, serde::Serialize)]
struct RecordingErrorEvent {
    error_type: String,
    detail: Option<String>,
}

fn check_preconditions(app: &AppHandle) -> Option<(&'static str, String)> {
    let auth = app.state::<AnonenCloudAuthManager>();
    if !auth.status().signed_in {
        return Some((
            "not_signed_in",
            "Sign in required. Open Settings → Models and sign in first.".to_string(),
        ));
    }

    if !crate::managers::usage::has_active_subscription(app) {
        return Some((
            "no_active_subscription",
            "有効な契約がありません。Webでご契約ください。".to_string(),
        ));
    }

    let settings = get_settings(app);
    let selected = &settings.selected_model;
    if selected.is_empty() {
        return Some((
            "no_model_selected",
            "モデルが選ばれていません。設定 → モデルで選んでください。".to_string(),
        ));
    }

    if selected.starts_with(crate::managers::model::ANONEN_CLOUD_MODEL_PREFIX) {
        let mm = app.state::<Arc<ModelManager>>();
        if mm.get_remote_config(selected).is_none() && mm.has_registered_cloud_models() {
            return Some((
                "cloud_model_unavailable",
                "このモデルは提供終了しました。新しいモデルを選んでください".to_string(),
            ));
        }

        let base_url = mm.anonen_cloud_base_url();
        let probe_url = format!("{}/v1/models", base_url.trim_end_matches('/'));
        let reachable = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .ok()
            .and_then(|c| c.head(&probe_url).send().ok())
            .is_some();
        if !reachable {
            return Some((
                "network_unavailable",
                "ネットワークに接続できません。接続を確認してください。".to_string(),
            ));
        }
    }
    None
}

const RECORDING_LIMIT_WARNING_LEAD: Duration = Duration::from_secs(30);

const RECORDING_LIMIT_STOP_MARGIN: Duration = Duration::from_secs(2);

const RECORDING_LIMIT_POLL: Duration = Duration::from_millis(250);

fn spawn_recording_limit_watchdog(app: &AppHandle, binding_id: &str, my_generation: u64) {
    let is_cloud_selected = |app: &AppHandle| {
        get_settings(app)
            .selected_model
            .starts_with(crate::managers::model::ANONEN_CLOUD_MODEL_PREFIX)
    };

    if !is_cloud_selected(app) {
        return;
    }

    let cap = Duration::from_secs(crate::managers::usage::max_recording_seconds(app));
    let stop_at = cap.saturating_sub(RECORDING_LIMIT_STOP_MARGIN);
    let warn_at = stop_at.saturating_sub(RECORDING_LIMIT_WARNING_LEAD);
    log::info!(
        "[recording-limit] watchdog armed: cap={}s (auto-stop at {}s, warn at {}s)",
        cap.as_secs(),
        stop_at.as_secs(),
        warn_at.as_secs()
    );

    let app = app.clone();
    let binding_id = binding_id.to_string();
    std::thread::spawn(move || {
        let started = Instant::now();

        let mut warned = warn_at.is_zero();
        loop {
            std::thread::sleep(RECORDING_LIMIT_POLL);

            if PipelineGeneration::current(&app) != my_generation {
                return;
            }
            if !app.state::<Arc<AudioRecordingManager>>().is_recording() {
                return;
            }

            if !is_cloud_selected(&app) {
                log::info!("[recording-limit] selected model became local; watchdog disarmed");
                return;
            }

            let elapsed = started.elapsed();

            if !warned && elapsed >= warn_at {
                warned = true;
                let seconds_left = stop_at.saturating_sub(elapsed).as_secs();
                log::info!(
                    "[recording-limit] warning: ~{}s left before auto-stop",
                    seconds_left
                );
                utils::show_limit_warning_overlay(&app);

                {
                    use tauri_plugin_notification::NotificationExt;
                    let _ = app
                        .notification()
                        .builder()
                        .title("あのねん: まもなく録音の上限に達します")
                        .body(format!(
                            "残り約{}秒で自動停止し、ここまでの内容を文字起こしします。",
                            seconds_left
                        ))
                        .show();
                }
            }

            if elapsed >= stop_at {
                log::info!(
                    "[recording-limit] cap reached after {:.1}s; auto-stopping into the normal transcription flow",
                    elapsed.as_secs_f64()
                );
                if let Some(coordinator) = app.try_state::<TranscriptionCoordinator>() {
                    coordinator.request_auto_stop(&binding_id, my_generation);
                }

                {
                    use tauri_plugin_notification::NotificationExt;
                    let _ = app
                        .notification()
                        .builder()
                        .title("あのねん: 録音の上限に達しました")
                        .body("録音を自動停止し、ここまでの内容を文字起こしします。")
                        .show();
                }
                return;
            }
        }
    });
}

pub struct PipelineGeneration(pub AtomicU64);

impl PipelineGeneration {
    pub fn current(app: &AppHandle) -> u64 {
        app.try_state::<PipelineGeneration>()
            .map(|g| g.0.load(Ordering::SeqCst))
            .unwrap_or(0)
    }

    pub fn bump(app: &AppHandle) {
        if let Some(g) = app.try_state::<PipelineGeneration>() {
            g.0.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cancellation {
    Pipeline,

    Independent,
}

fn should_abandon(cancellation: Cancellation, baseline: u64, current: u64) -> bool {
    matches!(cancellation, Cancellation::Pipeline) && current != baseline
}

pub fn run_cancellable<T, F>(
    app_handle: &AppHandle,
    cancellation: Cancellation,
    f: F,
) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let baseline = PipelineGeneration::current(app_handle);
    run_cancellable_from(app_handle, cancellation, baseline, f)
}

pub fn run_cancellable_from<T, F>(
    app_handle: &AppHandle,
    cancellation: Cancellation,
    baseline: u64,
    f: F,
) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(v) => return Ok(v),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if should_abandon(
                    cancellation,
                    baseline,
                    PipelineGeneration::current(app_handle),
                ) {
                    return Err(anyhow::anyhow!("cancelled during in-flight request"));
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(anyhow::anyhow!("worker thread died before sending result"));
            }
        }
    }
}

pub fn sleep_unless_abandoned(
    app_handle: &AppHandle,
    cancellation: Cancellation,
    baseline: u64,
    delay: Duration,
) -> bool {
    sleep_unless_abandoned_with(cancellation, baseline, delay, || {
        PipelineGeneration::current(app_handle)
    })
}

fn sleep_unless_abandoned_with<F: Fn() -> u64>(
    cancellation: Cancellation,
    baseline: u64,
    delay: Duration,
    current_generation: F,
) -> bool {
    let deadline = Instant::now() + delay;
    loop {
        if should_abandon(cancellation, baseline, current_generation()) {
            return false;
        }
        let now = Instant::now();
        if now >= deadline {
            return true;
        }
        std::thread::sleep((deadline - now).min(CANCELLATION_POLL_INTERVAL));
    }
}

struct FinishGuard(AppHandle, u64);
impl Drop for FinishGuard {
    fn drop(&mut self) {
        if let Some(c) = self.0.try_state::<TranscriptionCoordinator>() {
            c.notify_processing_finished(self.1);
        }
    }
}

pub trait ShortcutAction: Send + Sync {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
}

struct TranscribeAction {
    post_process: bool,
}

fn notify_input_clipping(app: &AppHandle) {
    static NOTIFIED: AtomicBool = AtomicBool::new(false);
    log::warn!("[mic] input is clipping — the analog gain is too high");
    if NOTIFIED.swap(true, Ordering::SeqCst) {
        return;
    }

    let japanese = get_settings(app).app_language.starts_with("ja");
    let (title, body) = if japanese {
        (
            "あのねん: マイクの音が割れています",
            "入力音量が大きすぎて音が潰れています。あのねんを開くと、その場で下げられます。",
        )
    } else {
        (
            "Anonen: your microphone is clipping",
            "The input volume is too high and the audio is distorting. Open Anonen to turn it down.",
        )
    };
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}

#[allow(dead_code)]

fn strip_think_block(s: &str) -> &str {
    if let Some(rest) = s.trim_start().strip_prefix("<think>") {
        if let Some(end) = rest.find("</think>") {
            return rest[end + "</think>".len()..].trim_start();
        }
    }
    s
}

#[allow(dead_code)]

fn is_blank_transcription(transcription: &str) -> bool {
    transcription.trim().is_empty()
}

async fn complete_unless_cancelled<F, C>(operation: F, is_cancelled: C) -> Option<F::Output>
where
    F: Future,
    C: Fn() -> bool,
{
    tokio::pin!(operation);

    loop {
        if is_cancelled() {
            return None;
        }

        if let Ok(result) =
            tokio::time::timeout(CANCELLATION_POLL_INTERVAL, operation.as_mut()).await
        {
            return Some(result);
        }
    }
}

#[allow(unused)]
async fn post_process_transcription(
    _settings: &AppSettings,
    _transcription: &str,
) -> Option<String> {
    None
}

async fn maybe_convert_chinese_variant(
    settings: &AppSettings,
    transcription: &str,
) -> Option<String> {
    let is_simplified = settings.selected_language == "zh-Hans";
    let is_traditional = settings.selected_language == "zh-Hant";

    if !is_simplified && !is_traditional {
        debug!("selected_language is not Simplified or Traditional Chinese; skipping translation");
        return None;
    }

    debug!(
        "Starting Chinese translation using OpenCC for language: {}",
        settings.selected_language
    );

    let config = if is_simplified {
        BuiltinConfig::Tw2sp
    } else {
        BuiltinConfig::S2tw
    };

    match OpenCC::from_config(config) {
        Ok(converter) => {
            let converted = converter.convert(transcription);
            debug!(
                "OpenCC translation completed. Input length: {}, Output length: {}",
                transcription.len(),
                converted.len()
            );
            Some(converted)
        }
        Err(e) => {
            error!("Failed to initialize OpenCC converter: {}. Falling back to original transcription.", e);
            None
        }
    }
}

pub(crate) struct ProcessedTranscription {
    pub final_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
}

pub(crate) async fn process_transcription_output(
    app: &AppHandle,
    transcription: &str,
    post_process: bool,
) -> ProcessedTranscription {
    let settings = get_settings(app);
    let mut final_text = transcription.to_string();
    let mut post_processed_text: Option<String> = None;
    let mut post_process_prompt: Option<String> = None;

    if let Some(converted_text) = maybe_convert_chinese_variant(&settings, transcription).await {
        final_text = converted_text;
    }

    if post_process {
        if let Some(processed_text) = post_process_transcription(&settings, &final_text).await {
            post_processed_text = Some(processed_text.clone());
            final_text = processed_text;

            if let Some(prompt_id) = &settings.post_process_selected_prompt_id {
                if let Some(prompt) = settings
                    .post_process_prompts
                    .iter()
                    .find(|prompt| &prompt.id == prompt_id)
                {
                    post_process_prompt = Some(prompt.prompt.clone());
                }
            }
        }
    } else if final_text != transcription {
        post_processed_text = Some(final_text.clone());
    }

    ProcessedTranscription {
        final_text,
        post_processed_text,
        post_process_prompt,
    }
}

impl ShortcutAction for TranscribeAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!("TranscribeAction::start called for binding: {}", binding_id);

        if let Some((error_type, message)) = check_preconditions(app) {
            warn!("[pipeline] start blocked ({}): {}", error_type, message);

            PipelineGeneration::bump(app);

            if let Some(coordinator) = app.try_state::<TranscriptionCoordinator>() {
                coordinator.notify_cancel(false);
            }
            let _ = app.emit(
                "recording-error",
                RecordingErrorEvent {
                    error_type: error_type.to_string(),
                    detail: Some(message),
                },
            );
            show_main_window(app);
            return;
        }

        PipelineGeneration::bump(app);

        let my_generation = PipelineGeneration::current(app);

        let tm = app.state::<Arc<TranscriptionManager>>();
        let rm = app.state::<Arc<AudioRecordingManager>>();

        let kickoff_started = Instant::now();
        tm.initiate_model_load();
        tm.warm_connection();
        let rm_clone = Arc::clone(&rm);
        std::thread::spawn(move || {
            if let Err(e) = rm_clone.preload_vad() {
                debug!("VAD pre-load failed: {}", e);
            }
        });
        let kickoff_elapsed = kickoff_started.elapsed();

        let binding_id = binding_id.to_string();
        let tray_started = Instant::now();
        change_tray_icon(app, TrayIconState::Recording);
        let tray_elapsed = tray_started.elapsed();

        let overlay_started = Instant::now();
        show_preparing_overlay(app);

        shortcut::register_cancel_shortcut(app);
        let overlay_elapsed = overlay_started.elapsed();

        let plan_started = Instant::now();
        let settings = get_settings(app);
        let is_always_on = settings.always_on_microphone;
        let plan_elapsed = plan_started.elapsed();

        debug!(
            "start-path pre-recording steps: model_kickoff={kickoff_elapsed:?} tray={tray_elapsed:?} overlay+cancel_shortcut={overlay_elapsed:?} settings={plan_elapsed:?}"
        );
        debug!("Microphone mode - always_on: {}", is_always_on);

        let app_clone = app.clone();
        let rm_for_thread = Arc::clone(&rm);
        std::thread::spawn(move || {
            let mut recording_error: Option<String> = None;
            if is_always_on {
                debug!("Always-on mode: Playing audio feedback immediately");
                let rm_clone = Arc::clone(&rm_for_thread);
                let app_for_sound = app_clone.clone();
                std::thread::spawn(move || {
                    play_feedback_sound_blocking(&app_for_sound, SoundType::Start);
                    rm_clone.apply_mute();
                });

                if let Err(e) = rm_for_thread.try_start_recording(&binding_id) {
                    debug!("Recording failed: {}", e);
                    recording_error = Some(e);
                } else {
                    show_recording_overlay(&app_clone);
                }
            } else {
                debug!("On-demand mode: Starting recording first, then audio feedback");
                let recording_start_time = Instant::now();
                match rm_for_thread.try_start_recording(&binding_id) {
                    Ok(()) => {
                        debug!("Recording started in {:?}", recording_start_time.elapsed());

                        show_recording_overlay(&app_clone);
                        let app_for_sound = app_clone.clone();
                        let rm_clone = Arc::clone(&rm_for_thread);
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            play_feedback_sound_blocking(&app_for_sound, SoundType::Start);
                            rm_clone.apply_mute();
                        });
                    }
                    Err(e) => {
                        debug!("Failed to start recording: {}", e);
                        recording_error = Some(e);
                    }
                }
            }

            if recording_error.is_none() {
                spawn_recording_limit_watchdog(&app_clone, &binding_id, my_generation);
            }

            if let Some(err) = recording_error {
                shortcut::unregister_cancel_shortcut(&app_clone);

                if let Some(coordinator) = app_clone.try_state::<TranscriptionCoordinator>() {
                    coordinator.notify_cancel(false);
                }
                utils::hide_recording_overlay(&app_clone);
                change_tray_icon(&app_clone, TrayIconState::Idle);
                let error_type = if is_microphone_access_denied(&err) {
                    "microphone_permission_denied"
                } else if is_no_input_device_error(&err) {
                    "no_input_device"
                } else {
                    "unknown"
                };
                let _ = app_clone.emit(
                    "recording-error",
                    RecordingErrorEvent {
                        error_type: error_type.to_string(),
                        detail: Some(err),
                    },
                );
            }
        });

        debug!(
            "TranscribeAction::start dispatched in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let my_generation = PipelineGeneration::current(app);

        let stop_time = Instant::now();

        log::info!("[pipeline] stop pressed for binding: {}", binding_id);

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
        let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());

        let t_tray = Instant::now();
        change_tray_icon(app, TrayIconState::Transcribing);
        let tray_ms = t_tray.elapsed().as_millis();
        let t_overlay = Instant::now();
        show_transcribing_overlay(app);
        let overlay_ms = t_overlay.elapsed().as_millis();
        if tray_ms > 50 || overlay_ms > 50 {
            warn!(
                "[pipeline] ui swap slow: change_tray_icon {}ms, show_transcribing_overlay {}ms",
                tray_ms, overlay_ms
            );
        }

        {
            let rm_unmute = Arc::clone(&rm);
            std::thread::spawn(move || {
                let t = Instant::now();
                rm_unmute.remove_mute();
                let ms = t.elapsed().as_millis();
                if ms > 50 {
                    warn!("[pipeline] unmute (set_mute) took {}ms (>50ms)", ms);
                }
            });
        }

        play_feedback_sound(app, SoundType::Stop);

        let binding_id = binding_id.to_string();

        let post_process = self.post_process || get_settings(app).post_process_enabled;

        tauri::async_runtime::spawn(async move {
            let _guard = FinishGuard(ah.clone(), my_generation);

            let is_cancelled = || PipelineGeneration::current(&ah) != my_generation;
            debug!(
                "Starting async transcription task for binding: {}",
                binding_id
            );

            let wait_started = Instant::now();
            const MAX_WAIT_FOR_START: Duration = Duration::from_secs(15);
            while !rm.is_recording() && wait_started.elapsed() < MAX_WAIT_FOR_START {
                if is_cancelled() {
                    log::info!(
                        "[pipeline] superseded while waiting for recording start; aborting quietly"
                    );
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let wait_ms = wait_started.elapsed().as_millis();
            if wait_ms > 50 {
                log::info!(
                    "[pipeline] waited {}ms for recording to actually start before stopping",
                    wait_ms
                );
            }

            if is_cancelled() {
                log::info!("[pipeline] superseded after start-wait; aborting quietly");
                return;
            }
            if !rm.is_recording() {
                warn!(
                    "[pipeline] stop fired but recording never started after {}ms; aborting",
                    wait_ms
                );
                if !is_cancelled() {
                    shortcut::unregister_cancel_shortcut(&ah);
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                }
                return;
            }

            let pipeline_start = Instant::now();
            let stop_recording_time = Instant::now();
            if let Some(samples) = rm.stop_recording(&binding_id) {
                let stop_ms = stop_recording_time.elapsed().as_millis();
                log::info!(
                    "[pipeline] recording stopped: {}ms, {} samples ({:.1}s audio)",
                    stop_ms,
                    samples.len(),
                    samples.len() as f64 / 16000.0,
                );

                const LOW_VOLUME_RMS_THRESHOLD: f32 = 0.03;
                let low_volume = rm
                    .last_input_rms()
                    .map_or(false, |rms| rms < LOW_VOLUME_RMS_THRESHOLD);

                if rm.last_input_level().map_or(false, |l| l.is_clipping()) {
                    notify_input_clipping(&ah);
                }

                if samples.is_empty() {
                    debug!("Recording produced no audio samples; skipping persistence");
                    shortcut::unregister_cancel_shortcut(&ah);
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                } else {
                    let history_enabled = crate::settings::get_history_retention(&ah).enabled();

                    let sample_count = samples.len();
                    let file_name = format!("anonen-{}.wav", chrono::Utc::now().timestamp());
                    let wav_path = hm.recordings_dir().join(&file_name);
                    let wav_path_for_verify = wav_path.clone();
                    let samples_for_wav = samples.clone();
                    let wav_handle = if history_enabled {
                        Some(tauri::async_runtime::spawn_blocking(move || {
                            crate::audio_toolkit::save_wav_file(&wav_path, &samples_for_wav)
                        }))
                    } else {
                        None
                    };

                    let transcription_time = Instant::now();
                    let transcription_result = tm.transcribe(samples);

                    if is_cancelled() {
                        log::info!("[pipeline] cancelled during transcription, discarding result");
                        return;
                    }

                    let wav_saved = match wav_handle {
                        None => false,
                        Some(handle) => match handle.await {
                            Ok(Ok(())) => match crate::audio_toolkit::verify_wav_file(
                                &wav_path_for_verify,
                                sample_count,
                            ) {
                                Ok(()) => true,
                                Err(e) => {
                                    error!("WAV verification failed: {}", e);
                                    false
                                }
                            },
                            Ok(Err(e)) => {
                                error!("Failed to save WAV file: {}", e);
                                false
                            }
                            Err(e) => {
                                error!("WAV save task panicked: {}", e);
                                false
                            }
                        },
                    };

                    match transcription_result {
                        Ok(transcription) => {
                            let transcribe_ms = transcription_time.elapsed().as_millis();
                            log::info!(
                                "[pipeline] transcription: {}ms ({} chars)",
                                transcribe_ms,
                                transcription.len(),
                            );

                            record_transcription_stats(&ah, transcribe_ms);

                            if post_process {
                                show_processing_overlay(&ah);
                            }
                            let post_process_time = Instant::now();

                            let Some(processed) = complete_unless_cancelled(
                                process_transcription_output(&ah, &transcription, post_process),
                                is_cancelled,
                            )
                            .await
                            else {
                                log::info!(
                                    "[pipeline] cancelled while post-processing was still running"
                                );
                                return;
                            };
                            let post_process_ms = post_process_time.elapsed().as_millis();
                            if post_process || post_process_ms > 5 {
                                log::info!("[pipeline] post-process: {}ms", post_process_ms);
                            }

                            if is_cancelled() {
                                log::info!(
                                    "[pipeline] cancelled during post-processing, discarding result"
                                );
                                return;
                            }

                            if wav_saved {
                                if let Err(err) = hm.save_entry(
                                    file_name,
                                    transcription,
                                    post_process,
                                    processed.post_processed_text.clone(),
                                    processed.post_process_prompt.clone(),
                                    tm.last_model(),
                                ) {
                                    error!("Failed to save history entry: {}", err);
                                }
                            }

                            if processed.final_text.is_empty() {
                                log::info!(
                                    "[pipeline] done (empty result): total {}ms",
                                    pipeline_start.elapsed().as_millis(),
                                );
                                shortcut::unregister_cancel_shortcut(&ah);
                                if low_volume {
                                    utils::show_low_volume_overlay(&ah);
                                } else {
                                    utils::hide_recording_overlay(&ah);
                                }
                                change_tray_icon(&ah, TrayIconState::Idle);
                            } else {
                                shortcut::unregister_cancel_shortcut(&ah);
                                let ah_clone = ah.clone();
                                let paste_time = Instant::now();
                                let pipeline_elapsed = pipeline_start.elapsed();
                                let final_text = processed.final_text;
                                ah.run_on_main_thread(move || {
                                    if PipelineGeneration::current(&ah_clone) != my_generation {
                                        log::info!(
                                            "[pipeline] paste skipped: generation mismatch (superseded)"
                                        );
                                        if low_volume {
                                            utils::show_low_volume_overlay(&ah_clone);
                                        } else {
                                            utils::hide_recording_overlay(&ah_clone);
                                        }
                                        change_tray_icon(&ah_clone, TrayIconState::Idle);
                                        return;
                                    }
                                    match utils::paste(final_text, ah_clone.clone()) {
                                        Ok(()) => {
                                            log::info!(
                                                "[pipeline] paste: {}ms, total: {}ms",
                                                paste_time.elapsed().as_millis(),
                                                pipeline_elapsed.as_millis()
                                                    + paste_time.elapsed().as_millis(),
                                            );

                                            log::info!(
                                                "[pipeline] user-wait: {}ms (stop pressed → text pasted)",
                                                stop_time.elapsed().as_millis(),
                                            );
                                        }
                                        Err(e) => {
                                            error!("Failed to paste transcription: {}", e);
                                            let _ = ah_clone.emit("paste-error", ());
                                        }
                                    }
                                    if low_volume {
                                        utils::show_low_volume_overlay(&ah_clone);
                                    } else {
                                        utils::hide_recording_overlay(&ah_clone);
                                    }
                                    change_tray_icon(&ah_clone, TrayIconState::Idle);
                                })
                                .unwrap_or_else(|e| {
                                    error!("Failed to run paste on main thread: {:?}", e);
                                    if low_volume {
                                        utils::show_low_volume_overlay(&ah);
                                    } else {
                                        utils::hide_recording_overlay(&ah);
                                    }
                                    change_tray_icon(&ah, TrayIconState::Idle);
                                });
                            }
                        }
                        Err(err) => {
                            if is_cancelled() {
                                log::info!(
                                    "[pipeline] cancelled; suppressing transcription error: {}",
                                    err
                                );
                                return;
                            }
                            shortcut::unregister_cancel_shortcut(&ah);
                            error!("Transcription failed: {}", err);

                            let kept = wav_saved
                                && match hm.save_entry(
                                    file_name,
                                    String::new(),
                                    post_process,
                                    None,
                                    None,
                                    None,
                                ) {
                                    Ok(_) => true,
                                    Err(save_err) => {
                                        error!("Failed to save failed history entry: {}", save_err);
                                        false
                                    }
                                };

                            let failure = crate::cloud_failure::failure_of(&err);
                            if failure.has_own_window_notice() {
                                raise_own_window_notice(&ah, &failure, kept);
                            } else {
                                let _ = ah.emit(
                                    "transcription-error",
                                    TranscriptionErrorPayload {
                                        message: err.to_string(),
                                        is_network_error: failure.is_network(),
                                        audio_saved: kept,
                                        notice: failure.window_notice_id(),
                                    },
                                );
                            }
                            {
                                use tauri_plugin_notification::NotificationExt;
                                let notification =
                                    crate::cloud_failure::failure_notification(&failure, kept);
                                let _ = ah
                                    .notification()
                                    .builder()
                                    .title(notification.title)
                                    .body(notification.body)
                                    .show();
                            }
                            if low_volume {
                                utils::show_low_volume_overlay(&ah);
                            } else {
                                utils::hide_recording_overlay(&ah);
                            }
                            change_tray_icon(&ah, TrayIconState::Idle);
                        }
                    }
                }
            } else {
                debug!("No samples retrieved from recording stop");
                if !is_cancelled() {
                    shortcut::unregister_cancel_shortcut(&ah);
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                }
            }
        });

        debug!(
            "TranscribeAction::stop completed in {:?}",
            stop_time.elapsed()
        );
    }
}

#[derive(Clone, serde::Serialize)]
struct TranscriptionErrorPayload {
    message: String,
    is_network_error: bool,
    audio_saved: bool,

    notice: Option<&'static str>,
}

#[derive(Clone, serde::Serialize)]
struct AudioTooLongPayload {
    audio_saved: bool,
}

pub(crate) fn notify_audio_too_long(app: &AppHandle, audio_saved: bool) {
    let _ = app.emit(
        "anonen-cloud-audio-too-long",
        AudioTooLongPayload { audio_saved },
    );
}

#[derive(Clone, serde::Serialize)]
struct CapExceededPayload {
    which: Option<String>,
    resets_at: Option<String>,
    fallback: Option<String>,
    audio_saved: bool,

    suspended: bool,
}

pub(crate) fn notify_cap_exceeded(
    app: &AppHandle,
    cap: &crate::cloud_failure::CloudCapExceededError,
    audio_saved: bool,
) {
    let _ = app.emit(
        "anonen-cloud-cap-exceeded",
        CapExceededPayload {
            which: cap.which.clone(),
            resets_at: cap.resets_at.clone(),
            fallback: cap.fallback.clone(),
            audio_saved,
            suspended: cap.suspended,
        },
    );
}

pub(crate) fn raise_own_window_notice(
    app: &AppHandle,
    failure: &crate::cloud_failure::Failure<'_>,
    audio_saved: bool,
) {
    use crate::cloud_failure::Failure;
    match failure {
        Failure::AudioTooLong => notify_audio_too_long(app, audio_saved),
        Failure::CapExceeded(cap) => notify_cap_exceeded(app, cap, audio_saved),
        _ => {}
    }
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct TranscriptionStatsPayload {
    model_id: String,
    asr_ms: u64,
}

pub(crate) fn record_transcription_stats(app: &AppHandle, transcribe_ms: u128) {
    let model_id = get_settings(app).selected_model.clone();
    if model_id.is_empty() || transcribe_ms == 0 {
        return;
    }
    let _ = app.emit(
        "transcription-stats-recorded",
        TranscriptionStatsPayload {
            model_id,
            asr_ms: transcribe_ms as u64,
        },
    );
}

struct CancelAction;

impl ShortcutAction for CancelAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        utils::cancel_current_operation(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

struct TestAction;

impl ShortcutAction for TestAction {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Started - {} (App: {})",
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Stopped - {} (App: {})",
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }
}

pub static ACTION_MAP: Lazy<HashMap<String, Arc<dyn ShortcutAction>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "transcribe".to_string(),
        Arc::new(TranscribeAction {
            post_process: false,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "transcribe_with_post_process".to_string(),
        Arc::new(TranscribeAction { post_process: true }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "cancel".to_string(),
        Arc::new(CancelAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    map
});

#[cfg(test)]
mod tests {
    use super::{
        complete_unless_cancelled, is_blank_transcription, should_abandon,
        sleep_unless_abandoned_with, strip_think_block, AudioTooLongPayload, Cancellation,
        CapExceededPayload, TranscriptionErrorPayload,
    };
    use std::future;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn failure_events_carry_whether_the_recording_was_kept() {
        let error = serde_json::to_value(TranscriptionErrorPayload {
            message: "network_error".to_string(),
            is_network_error: true,
            audio_saved: false,
            notice: None,
        })
        .unwrap();
        assert_eq!(
            error,
            serde_json::json!({
                "message": "network_error",
                "is_network_error": true,
                "audio_saved": false,
                "notice": null,
            })
        );
        let named = serde_json::to_value(TranscriptionErrorPayload {
            message: "x".to_string(),
            is_network_error: false,
            audio_saved: true,
            notice: Some("seal_outdated"),
        })
        .unwrap();
        assert_eq!(named["notice"], "seal_outdated");

        let too_long = serde_json::to_value(AudioTooLongPayload { audio_saved: true }).unwrap();
        assert_eq!(too_long, serde_json::json!({ "audio_saved": true }));

        let cap = serde_json::to_value(CapExceededPayload {
            which: Some("week".to_string()),
            resets_at: None,
            fallback: Some("local".to_string()),
            audio_saved: false,
            suspended: false,
        })
        .unwrap();
        assert_eq!(
            cap,
            serde_json::json!({
                "which": "week",
                "resets_at": null,
                "fallback": "local",
                "audio_saved": false,
                "suspended": false,
            })
        );
    }

    #[test]
    fn a_new_recording_abandons_the_pipeline_work_it_replaces() {
        assert!(should_abandon(Cancellation::Pipeline, 7, 8));
        assert!(!should_abandon(Cancellation::Pipeline, 7, 7));
    }

    #[test]
    fn a_history_retry_survives_the_next_recording() {
        assert!(!should_abandon(Cancellation::Independent, 7, 8));
        assert!(!should_abandon(Cancellation::Independent, 7, 7));
    }

    #[test]
    fn an_esc_during_the_backoff_stops_the_retry() {
        let generation = Arc::new(AtomicU64::new(7));
        let g = Arc::clone(&generation);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            g.store(8, Ordering::SeqCst);
        });

        let started = std::time::Instant::now();
        let finished =
            sleep_unless_abandoned_with(Cancellation::Pipeline, 7, Duration::from_secs(2), || {
                generation.load(Ordering::SeqCst)
            });

        assert!(!finished, "待つ間に世代が動いたら、送信まで進まない");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "待ち切らずに戻る（2 秒の待ちを丸ごと払わない）"
        );
    }

    #[test]
    fn a_history_retry_keeps_waiting_through_the_next_recording() {
        let finished = sleep_unless_abandoned_with(
            Cancellation::Independent,
            7,
            Duration::from_millis(10),
            || 8,
        );

        assert!(finished);
    }

    #[test]
    fn an_undisturbed_backoff_waits_out_its_delay() {
        let started = std::time::Instant::now();
        let finished =
            sleep_unless_abandoned_with(Cancellation::Pipeline, 7, Duration::from_millis(80), || 7);

        assert!(finished);
        assert!(started.elapsed() >= Duration::from_millis(80));
    }

    #[test]
    fn blank_transcription_is_detected() {
        assert!(is_blank_transcription(""));
        assert!(is_blank_transcription("   "));
        assert!(is_blank_transcription("\t\n  \r\n"));
    }

    #[test]
    fn non_blank_transcription_is_kept() {
        assert!(!is_blank_transcription("hello"));
        assert!(!is_blank_transcription("  hello  "));
    }

    #[test]
    fn completed_operation_returns_its_output() {
        let result = tauri::async_runtime::block_on(complete_unless_cancelled(
            future::ready("done"),
            || false,
        ));

        assert_eq!(result, Some("done"));
    }

    #[test]
    fn pending_operation_stops_after_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_for_thread = Arc::clone(&cancelled);
        let cancel_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            cancelled_for_thread.store(true, Ordering::Release);
        });

        let result = tauri::async_runtime::block_on(complete_unless_cancelled(
            future::pending::<()>(),
            || cancelled.load(Ordering::Acquire),
        ));

        cancel_thread.join().unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn leading_think_block_is_stripped() {
        assert_eq!(
            strip_think_block("<think>pondering...</think>Cleaned text."),
            "Cleaned text."
        );
        assert_eq!(
            strip_think_block("  \n<think>multi\nline</think>\n  Cleaned text."),
            "Cleaned text."
        );
    }

    #[test]
    fn content_without_think_block_is_unchanged() {
        assert_eq!(strip_think_block("Cleaned text."), "Cleaned text.");
        assert_eq!(
            strip_think_block("Mentions <think> mid-sentence."),
            "Mentions <think> mid-sentence."
        );

        assert_eq!(
            strip_think_block("<think>never closed"),
            "<think>never closed"
        );
    }
}
