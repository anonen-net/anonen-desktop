use crate::actions::Cancellation;
use crate::audio_toolkit::{
    apply_custom_words, normalize_transcription_output, remove_filler_words, OutputLanguageEvidence,
};
use crate::managers::anonen_cloud_auth::AnonenCloudAuthManager;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::engine_slot::{EngineSlot, PutBack};
use crate::managers::model::{EngineType, ModelManager};
use crate::managers::usage::UsageManager;
use crate::remote_asr::AudioPayload;
use crate::settings::{
    get_settings, AppSettings, ModelUnloadTimeout, OrtAcceleratorSetting, WhisperAcceleratorSetting,
};
use anyhow::Result;
use log::{debug, error, info, warn};
use serde::Serialize;
use specta::Type;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter, Manager};
use transcribe_rs::{
    onnx::{
        canary::CanaryModel,
        cohere::CohereModel,
        gigaam::GigaAMModel,
        moonshine::{MoonshineModel, MoonshineVariant, StreamingModel},
        parakeet::{ParakeetModel, ParakeetParams, TimestampGranularity},
        sense_voice::{SenseVoiceModel, SenseVoiceParams},
        Quantization,
    },
    whisper_cpp::{WhisperEngine, WhisperInferenceParams, WhisperLoadParams},
    SpeechModel, TranscribeOptions,
};

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic".to_string()
    }
}

fn fail_open_text_transform<F>(raw: String, transform: F) -> String
where
    F: FnOnce(String) -> String,
{
    let fallback = raw.clone();
    match catch_unwind(AssertUnwindSafe(|| transform(raw))) {
        Ok(processed) => processed,
        Err(payload) => {
            error!(
                "Optional transcription text post-processing panicked: {}; using the raw transcription",
                panic_payload_message(payload.as_ref())
            );
            fallback
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelStateEvent {
    pub event_type: String,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub error: Option<String>,
}

use crate::cloud_failure::{
    classify_terminal, worth_one_token_refresh, CloudAudioTooLongError, CloudAuthFailedError,
    CloudCapExceededError, CloudModelInvalidError, CloudSubscriptionRequiredError, Terminal,
};

use crate::sealed::policy::{
    decide_upload, on_gateway_error, CloudSealError, NoKeyReason, RecordingSealState,
    SealErrorAction, UploadPolicy, UploadRoute,
};

enum LoadedEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetModel),
    Moonshine(MoonshineModel),
    MoonshineStreaming(StreamingModel),
    SenseVoice(SenseVoiceModel),
    GigaAM(GigaAMModel),
    Canary(CanaryModel),
    Cohere(CohereModel),

    Remote,
}

const MODEL_LOAD_WAIT_TIMEOUT: Duration = Duration::from_secs(300);

pub struct LoadingGuard {
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
}

impl Drop for LoadingGuard {
    fn drop(&mut self) {
        let mut is_loading = match self.is_loading.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        *is_loading = false;
        self.loading_condvar.notify_all();
    }
}

#[derive(Clone)]
pub struct TranscriptionManager {
    engine: Arc<Mutex<EngineSlot<LoadedEngine>>>,
    model_manager: Arc<ModelManager>,
    app_handle: AppHandle,
    current_model_id: Arc<Mutex<Option<String>>>,
    last_activity: Arc<AtomicU64>,
    shutdown_signal: Arc<AtomicBool>,
    watcher_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,

    http_client: Arc<OnceLock<reqwest::blocking::Client>>,

    last_models_version: Arc<Mutex<Option<String>>>,

    last_model: Arc<Mutex<Option<String>>>,

    enclave_keys: Arc<crate::sealed::keys::EnclaveKeyStore>,
}

impl TranscriptionManager {
    pub fn new(app_handle: &AppHandle, model_manager: Arc<ModelManager>) -> Result<Self> {
        let manager = Self {
            engine: Arc::new(Mutex::new(EngineSlot::Empty)),
            model_manager,
            app_handle: app_handle.clone(),
            current_model_id: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(AtomicU64::new(Self::now_ms())),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            watcher_handle: Arc::new(Mutex::new(None)),
            is_loading: Arc::new(Mutex::new(false)),
            loading_condvar: Arc::new(Condvar::new()),
            http_client: Arc::new(OnceLock::new()),
            last_models_version: Arc::new(Mutex::new(None)),
            last_model: Arc::new(Mutex::new(None)),
            enclave_keys: Arc::new(crate::sealed::keys::EnclaveKeyStore::new(
                crate::sealed::attestation::DEFAULT_AUDIENCE,
                UploadPolicy::from_build().accepted_image_digests,
            )),
        };

        {
            let app_handle_cloned = app_handle.clone();
            let manager_cloned = manager.clone();
            let shutdown_signal = manager.shutdown_signal.clone();
            let handle = thread::spawn(move || {
                debug!("Idle watcher thread started");
                while !shutdown_signal.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(10));

                    if shutdown_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    let settings = get_settings(&app_handle_cloned);
                    let timeout = settings.model_unload_timeout;

                    if timeout == ModelUnloadTimeout::Immediately {
                        continue;
                    }

                    let is_recording = app_handle_cloned
                        .try_state::<Arc<AudioRecordingManager>>()
                        .map_or(false, |a| a.is_recording());
                    if is_recording {
                        manager_cloned.touch_activity();
                        continue;
                    }

                    if manager_cloned.is_engine_borrowed() {
                        manager_cloned.touch_activity();
                        continue;
                    }

                    if let Some(limit_seconds) = timeout.to_seconds() {
                        let last = manager_cloned.last_activity.load(Ordering::Relaxed);
                        let now_ms = TranscriptionManager::now_ms();
                        let idle_ms = now_ms.saturating_sub(last);
                        let limit_ms = limit_seconds * 1000;

                        if idle_ms > limit_ms {
                            if manager_cloned.is_model_loaded() {
                                let unload_start = std::time::Instant::now();
                                info!(
                                    "Model idle for {}s (limit: {}s), unloading",
                                    idle_ms / 1000,
                                    limit_seconds
                                );
                                match manager_cloned.unload_model() {
                                    Ok(()) => {
                                        let unload_duration = unload_start.elapsed();
                                        info!(
                                            "Model unloaded due to inactivity (took {}ms)",
                                            unload_duration.as_millis()
                                        );
                                    }
                                    Err(e) => {
                                        error!("Failed to unload idle model: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
                debug!("Idle watcher thread shutting down gracefully");
            });
            *manager.watcher_handle.lock().unwrap() = Some(handle);
        }

        Ok(manager)
    }

    fn lock_engine(&self) -> MutexGuard<'_, EngineSlot<LoadedEngine>> {
        self.engine.lock().unwrap_or_else(|poisoned| {
            warn!("Engine mutex was poisoned by a previous panic, recovering");
            poisoned.into_inner()
        })
    }

    pub fn is_model_loaded(&self) -> bool {
        self.lock_engine().is_loaded()
    }

    fn is_engine_borrowed(&self) -> bool {
        self.lock_engine().is_borrowed()
    }

    pub fn is_local_model_loaded(&self) -> bool {
        let engine = self.lock_engine();
        match &*engine {
            EngineSlot::Empty | EngineSlot::Ready(LoadedEngine::Remote) => false,
            EngineSlot::Ready(_) => true,

            EngineSlot::Borrowed { .. } => true,
        }
    }

    pub fn try_start_loading(&self) -> Option<LoadingGuard> {
        let mut is_loading = self.is_loading.lock().unwrap();
        if *is_loading {
            return None;
        }
        *is_loading = true;
        Some(LoadingGuard {
            is_loading: self.is_loading.clone(),
            loading_condvar: self.loading_condvar.clone(),
        })
    }

    pub fn unload_model(&self) -> Result<()> {
        let unload_start = std::time::Instant::now();
        debug!("Starting to unload model");

        {
            let mut engine = self.lock_engine();

            engine.clear();
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = None;
        }

        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "unloaded".to_string(),
                model_id: None,
                model_name: None,
                error: None,
            },
        );

        let unload_duration = unload_start.elapsed();
        debug!(
            "Model unloaded manually (took {}ms)",
            unload_duration.as_millis()
        );
        Ok(())
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn now_seconds() -> i64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    fn touch_activity(&self) {
        self.last_activity.store(Self::now_ms(), Ordering::Relaxed);
    }

    pub fn maybe_unload_immediately(&self, context: &str) {
        let settings = get_settings(&self.app_handle);
        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately
            && self.is_model_loaded()
        {
            info!("Immediately unloading model after {}", context);
            if let Err(e) = self.unload_model() {
                warn!("Failed to immediately unload model: {}", e);
            }
        }
    }

    pub fn load_model(&self, model_id: &str) -> Result<()> {
        let load_start = std::time::Instant::now();
        debug!("Starting to load model: {}", model_id);

        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_started".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: None,
                error: None,
            },
        );

        if model_id.starts_with(crate::managers::model::ANONEN_CLOUD_MODEL_PREFIX)
            && self.model_manager.get_model_info(model_id).is_none()
        {
            info!(
                "[anonen-cloud] card for {} not registered yet; loading Remote marker anyway",
                model_id
            );
            {
                let mut engine = self.lock_engine();
                engine.install(LoadedEngine::Remote);
            }
            {
                let mut current_model = self.current_model_id.lock().unwrap();
                *current_model = Some(model_id.to_string());
            }
            self.touch_activity();
            let _ = self.app_handle.emit(
                "model-state-changed",
                ModelStateEvent {
                    event_type: "loading_completed".to_string(),
                    model_id: Some(model_id.to_string()),
                    model_name: None,
                    error: None,
                },
            );
            return Ok(());
        }

        let model_info = self
            .model_manager
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if !model_info.is_downloaded {
            let error_msg = "Model not downloaded";
            let _ = self.app_handle.emit(
                "model-state-changed",
                ModelStateEvent {
                    event_type: "loading_failed".to_string(),
                    model_id: Some(model_id.to_string()),
                    model_name: Some(model_info.name.clone()),
                    error: Some(error_msg.to_string()),
                },
            );
            return Err(anyhow::anyhow!(error_msg));
        }

        let model_path = self.model_manager.get_model_path(model_id)?;

        let emit_loading_failed = |error_msg: &str| {
            let _ = self.app_handle.emit(
                "model-state-changed",
                ModelStateEvent {
                    event_type: "loading_failed".to_string(),
                    model_id: Some(model_id.to_string()),
                    model_name: Some(model_info.name.clone()),
                    error: Some(error_msg.to_string()),
                },
            );
        };

        let loaded_engine = match model_info.engine_type {
            EngineType::Whisper => {
                let params = whisper_load_params(&self.app_handle, &model_path);

                let _attempt = params
                    .use_gpu
                    .then(|| gpu_attempt_marker(&self.app_handle))
                    .flatten();
                let engine = WhisperEngine::load_with_params(&model_path, params).map_err(|e| {
                    let error_msg = format!("Failed to load whisper model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Whisper(engine)
            }
            EngineType::Parakeet => {
                let engine =
                    ParakeetModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                        let error_msg =
                            format!("Failed to load parakeet model {}: {}", model_id, e);
                        emit_loading_failed(&error_msg);
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::Parakeet(engine)
            }
            EngineType::Moonshine => {
                let engine = MoonshineModel::load(
                    &model_path,
                    MoonshineVariant::Base,
                    &Quantization::default(),
                )
                .map_err(|e| {
                    let error_msg = format!("Failed to load moonshine model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Moonshine(engine)
            }
            EngineType::MoonshineStreaming => {
                let engine = StreamingModel::load(&model_path, 0, &Quantization::default())
                    .map_err(|e| {
                        let error_msg = format!(
                            "Failed to load moonshine streaming model {}: {}",
                            model_id, e
                        );
                        emit_loading_failed(&error_msg);
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::MoonshineStreaming(engine)
            }
            EngineType::SenseVoice => {
                let engine =
                    SenseVoiceModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                        let error_msg =
                            format!("Failed to load SenseVoice model {}: {}", model_id, e);
                        emit_loading_failed(&error_msg);
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::SenseVoice(engine)
            }
            EngineType::GigaAM => {
                let engine = GigaAMModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                    let error_msg = format!("Failed to load gigaam model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::GigaAM(engine)
            }
            EngineType::Canary => {
                let engine = CanaryModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                    let error_msg = format!("Failed to load canary model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Canary(engine)
            }
            EngineType::Cohere => {
                let engine = CohereModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                    let error_msg = format!("Failed to load cohere model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Cohere(engine)
            }

            EngineType::Remote => LoadedEngine::Remote,
        };

        {
            let mut engine = self.lock_engine();

            engine.install(loaded_engine);
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = Some(model_id.to_string());
        }

        self.touch_activity();

        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_completed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );

        let load_duration = load_start.elapsed();
        debug!(
            "Successfully loaded transcription model: {} (took {}ms)",
            model_id,
            load_duration.as_millis()
        );
        Ok(())
    }

    pub fn initiate_model_load(&self) {
        let Some(guard) = self.try_start_loading() else {
            return;
        };
        if self.is_model_loaded() {
            return;
        }

        let self_clone = self.clone();
        thread::spawn(move || {
            let _guard = guard;
            let settings = get_settings(&self_clone.app_handle);
            if let Err(e) = self_clone.load_model(&settings.selected_model) {
                error!("Failed to load model: {}", e);
            }
        });
    }

    pub fn prefetch_enclave_key(&self) {
        use crate::managers::model::ANONEN_CLOUD_MODEL_PREFIX;

        let settings = get_settings(&self.app_handle);

        if !settings
            .selected_model
            .starts_with(ANONEN_CLOUD_MODEL_PREFIX)
        {
            return;
        }
        let base_url = self.model_manager.anonen_cloud_base_url();

        let client_cell = self.http_client.clone();
        let enclave_keys = self.enclave_keys.clone();
        thread::spawn(move || {
            let client = client_cell.get_or_init(|| {
                crate::remote_asr::build_shared_client()
                    .expect("Failed to build shared HTTP client")
            });
            let source = crate::sealed::keys_http::HttpAttestationSource::new(client, &base_url);
            enclave_keys.prefetch(&source, Self::now_seconds());
        });
    }

    pub fn warm_connection(&self) {
        use crate::managers::model::ANONEN_CLOUD_MODEL_PREFIX;

        let settings = get_settings(&self.app_handle);
        let selected = &settings.selected_model;
        info!("[warm] selected_model={}", selected);

        let is_anonen_cloud = selected.starts_with(ANONEN_CLOUD_MODEL_PREFIX);

        let (base_url, is_gateway, upstream_model_id) = if is_anonen_cloud {
            let gw_url = self.model_manager.anonen_cloud_base_url();
            let model_id = selected
                .strip_prefix(ANONEN_CLOUD_MODEL_PREFIX)
                .map(|s| s.to_string());
            (gw_url, true, model_id)
        } else {
            let model_info = self.model_manager.get_model_info(selected);
            let is_remote = model_info
                .as_ref()
                .map_or(false, |m| matches!(m.engine_type, EngineType::Remote));
            if !is_remote {
                return;
            }
            match self.model_manager.get_remote_config(selected) {
                Some(cfg) => (cfg.base_url, true, Some(cfg.model_id)),

                None => {
                    warn!(
                        "[warm] skipped: {} has no registered remote config",
                        selected
                    );
                    return;
                }
            }
        };
        if base_url.trim().is_empty() {
            return;
        }
        if is_gateway {
            let signed_in = self
                .app_handle
                .state::<AnonenCloudAuthManager>()
                .status()
                .signed_in;
            if !signed_in {
                info!("[warm] skipped: not signed in");
                return;
            }
        }

        let client_cell = self.http_client.clone();
        let app_handle = self.app_handle.clone();
        let enclave_keys = self.enclave_keys.clone();
        thread::spawn(move || {
            let t = std::time::Instant::now();
            let client = client_cell.get_or_init(|| {
                crate::remote_asr::build_shared_client()
                    .expect("Failed to build shared HTTP client")
            });

            if is_gateway {
                let auth = app_handle.state::<AnonenCloudAuthManager>();
                match auth.get_access_token() {
                    Ok(token) => {
                        let warm_model = upstream_model_id
                            .as_deref()
                            .map(str::trim)
                            .filter(|m| !m.is_empty());
                        if let Some(warm_model) = warm_model {
                            let warm_url = format!("{}/v1/warm", base_url.trim_end_matches('/'));
                            let mut body = serde_json::Map::new();
                            body.insert(
                                "model".to_string(),
                                serde_json::Value::String(warm_model.to_string()),
                            );
                            match crate::remote_asr::identify(client.post(&warm_url))
                                .bearer_auth(&token)
                                .json(&body)
                                .timeout(std::time::Duration::from_secs(5))
                                .send()
                            {
                                Ok(resp) => {
                                    info!(
                                        "[warm] POST {} → {} in {}ms",
                                        warm_url,
                                        resp.status(),
                                        t.elapsed().as_millis(),
                                    );

                                    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                                        auth.invalidate_access_token();
                                    }
                                }
                                Err(e) => warn!(
                                    "[warm] POST /v1/warm failed in {}ms: {}",
                                    t.elapsed().as_millis(),
                                    e,
                                ),
                            }
                        } else {
                            info!("[warm] POST /v1/warm skipped: no model selected (R23)");
                        }
                    }

                    Err(e) => warn!(
                        "[warm] token pre-refresh failed: {}",
                        crate::utils::quoted_body(&e)
                    ),
                }

                let source =
                    crate::sealed::keys_http::HttpAttestationSource::new(client, &base_url);
                enclave_keys.prefetch(&source, Self::now_seconds());
            } else {
                let warm_url = format!("{}/models", base_url.trim_end_matches('/'));
                let _ = client
                    .head(&warm_url)
                    .timeout(std::time::Duration::from_secs(2))
                    .send();
                info!("[warm] HEAD {} in {}ms", base_url, t.elapsed().as_millis(),);
            }
        });
    }

    pub fn get_current_model(&self) -> Option<String> {
        let current_model = self.current_model_id.lock().unwrap();
        current_model.clone()
    }

    pub fn transcribe(&self, audio: Vec<f32>) -> Result<String> {
        self.transcribe_with_options(audio, None, Cancellation::Pipeline)
    }

    pub fn transcribe_with_options(
        &self,
        audio: Vec<f32>,
        opus_override: Option<bool>,
        cancellation: Cancellation,
    ) -> Result<String> {
        #[cfg(debug_assertions)]
        if std::env::var("ANONEN_FORCE_TRANSCRIPTION_FAILURE").is_ok() {
            return Err(anyhow::anyhow!(
                "Simulated transcription failure (ANONEN_FORCE_TRANSCRIPTION_FAILURE)"
            ));
        }

        *self.last_model.lock().unwrap() = None;

        self.touch_activity();

        let st = std::time::Instant::now();

        debug!("Audio vector length: {}", audio.len());

        if audio.is_empty() {
            debug!("Empty audio vector");
            self.maybe_unload_immediately("empty audio");
            return Ok(String::new());
        }

        {
            let deadline = std::time::Instant::now() + MODEL_LOAD_WAIT_TIMEOUT;
            let mut is_loading = self
                .is_loading
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while *is_loading {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    return Err(anyhow::anyhow!(
                        "Timed out waiting for the model to finish loading."
                    ));
                }
                let (guard, _) = self
                    .loading_condvar
                    .wait_timeout(is_loading, remaining)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                is_loading = guard;
            }

            let engine_guard = self.lock_engine();
            if !engine_guard.is_loaded() {
                return Err(anyhow::anyhow!("Model is not loaded for transcription."));
            }
        }

        let settings = get_settings(&self.app_handle);

        let validated_language = if settings.selected_language == "auto" {
            "auto".to_string()
        } else {
            let is_supported = self
                .model_manager
                .get_model_info(&settings.selected_model)
                .map(|info| {
                    info.supported_languages.is_empty()
                        || info
                            .supported_languages
                            .contains(&settings.selected_language)
                })
                .unwrap_or(true);

            if is_supported {
                settings.selected_language.clone()
            } else {
                warn!(
                    "Language '{}' not supported by current model, falling back to auto-detect",
                    settings.selected_language
                );
                "auto".to_string()
            }
        };

        let is_remote = matches!(self.lock_engine().ready(), Some(LoadedEngine::Remote));

        let result_text: String = if is_remote {
            self.transcribe_remote(&audio, &settings, opus_override, cancellation)?
        } else {
            let borrowed_id = self.get_current_model();
            let mut engine_guard = self.lock_engine();

            let borrowed = engine_guard.borrow_engine(borrowed_id.clone());

            let already_busy = engine_guard.is_borrowed();
            let mut engine = match borrowed {
                Some(e) => e,
                None if already_busy => {
                    return Err(anyhow::anyhow!(
                        "Another transcription is still running on this model. Please try again."
                    ));
                }
                None => {
                    return Err(anyhow::anyhow!(
                        "Model failed to load after auto-load attempt. Please check your model settings."
                    ));
                }
            };

            drop(engine_guard);

            let gpu_log_dir = transcribe_rs::accel::get_whisper_accelerator()
                .use_gpu()
                .then(|| crate::portable::app_log_dir(&self.app_handle).ok())
                .flatten();

            let transcribe_result = catch_unwind(AssertUnwindSafe(
                || -> Result<transcribe_rs::TranscriptionResult> {
                    match &mut engine {
                        LoadedEngine::Whisper(whisper_engine) => {
                            let whisper_language = if validated_language == "auto" {
                                None
                            } else {
                                let normalized = if validated_language == "zh-Hans"
                                    || validated_language == "zh-Hant"
                                {
                                    "zh".to_string()
                                } else {
                                    validated_language.clone()
                                };
                                Some(normalized)
                            };

                            let params = WhisperInferenceParams {
                                language: whisper_language,
                                translate: settings.translate_to_english,
                                initial_prompt: if settings.custom_words.is_empty() {
                                    None
                                } else {
                                    Some(settings.custom_words.join(", "))
                                },
                                ..Default::default()
                            };

                            let _attempt = gpu_log_dir
                                .as_deref()
                                .and_then(crate::gpu_guard::GpuAttemptMarker::begin);

                            whisper_engine
                                .transcribe_with(&audio, &params)
                                .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))
                        }
                        LoadedEngine::Parakeet(parakeet_engine) => {
                            let params = ParakeetParams {
                                timestamp_granularity: Some(TimestampGranularity::Segment),
                                ..Default::default()
                            };
                            parakeet_engine
                                .transcribe_with(&audio, &params)
                                .map_err(|e| {
                                    anyhow::anyhow!("Parakeet transcription failed: {}", e)
                                })
                        }
                        LoadedEngine::Moonshine(moonshine_engine) => moonshine_engine
                            .transcribe(&audio, &TranscribeOptions::default())
                            .map_err(|e| anyhow::anyhow!("Moonshine transcription failed: {}", e)),
                        LoadedEngine::MoonshineStreaming(streaming_engine) => streaming_engine
                            .transcribe(&audio, &TranscribeOptions::default())
                            .map_err(|e| {
                                anyhow::anyhow!("Moonshine streaming transcription failed: {}", e)
                            }),
                        LoadedEngine::SenseVoice(sense_voice_engine) => {
                            let language = match validated_language.as_str() {
                                "zh" | "zh-Hans" | "zh-Hant" => Some("zh".to_string()),
                                "en" => Some("en".to_string()),
                                "ja" => Some("ja".to_string()),
                                "ko" => Some("ko".to_string()),
                                "yue" => Some("yue".to_string()),
                                _ => None,
                            };
                            let params = SenseVoiceParams {
                                language,
                                use_itn: Some(true),
                            };
                            sense_voice_engine
                                .transcribe_with(&audio, &params)
                                .map_err(|e| {
                                    anyhow::anyhow!("SenseVoice transcription failed: {}", e)
                                })
                        }
                        LoadedEngine::GigaAM(gigaam_engine) => gigaam_engine
                            .transcribe(&audio, &TranscribeOptions::default())
                            .map_err(|e| anyhow::anyhow!("GigaAM transcription failed: {}", e)),
                        LoadedEngine::Canary(canary_engine) => {
                            let lang = if validated_language == "auto" {
                                None
                            } else {
                                Some(validated_language.clone())
                            };
                            let options = TranscribeOptions {
                                language: lang,
                                translate: settings.translate_to_english,
                                ..Default::default()
                            };
                            canary_engine
                                .transcribe(&audio, &options)
                                .map_err(|e| anyhow::anyhow!("Canary transcription failed: {}", e))
                        }
                        LoadedEngine::Cohere(cohere_engine) => {
                            let lang = if validated_language == "auto" {
                                None
                            } else if validated_language == "zh-Hans"
                                || validated_language == "zh-Hant"
                            {
                                Some("zh".to_string())
                            } else {
                                Some(validated_language.clone())
                            };
                            let options = TranscribeOptions {
                                language: lang,
                                ..Default::default()
                            };
                            cohere_engine
                                .transcribe(&audio, &options)
                                .map_err(|e| anyhow::anyhow!("Cohere transcription failed: {}", e))
                        }
                        LoadedEngine::Remote => {
                            unreachable!("remote ASR is handled before this match")
                        }
                    }
                },
            ));

            match transcribe_result {
                Ok(inner_result) => {
                    let current = self.get_current_model();
                    let outcome = self.lock_engine().put_back(&borrowed_id, engine, &current);
                    if outcome != PutBack::Restored {
                        info!(
                            "Engine from {:?} discarded after transcription: {:?}",
                            borrowed_id, outcome
                        );
                    }
                    inner_result?.text
                }
                Err(panic_payload) => {
                    let panic_msg = panic_payload_message(panic_payload.as_ref());
                    error!(
                        "Transcription engine panicked: {}. Model has been unloaded.",
                        panic_msg
                    );

                    self.lock_engine().abandon_borrow(&borrowed_id);

                    {
                        let mut current_model = self
                            .current_model_id
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        *current_model = None;
                    }

                    let _ = self.app_handle.emit(
                        "model-state-changed",
                        ModelStateEvent {
                            event_type: "unloaded".to_string(),
                            model_id: None,
                            model_name: None,
                            error: Some(format!("Engine panicked: {}", panic_msg)),
                        },
                    );

                    return Err(anyhow::anyhow!(
                        "Transcription engine panicked: {}. The model has been unloaded and will reload on next attempt.",
                        panic_msg
                    ));
                }
            }
        };

        if !is_remote {
            *self.last_model.lock().unwrap() = Some(settings.selected_model.clone());
        }

        let is_whisper = self
            .model_manager
            .get_model_info(&settings.selected_model)
            .map(|info| matches!(info.engine_type, EngineType::Whisper))
            .unwrap_or(false);

        let output_language = if settings.translate_to_english && !is_remote {
            OutputLanguageEvidence::TranslatedToEnglish
        } else if validated_language == "auto" {
            OutputLanguageEvidence::Unknown
        } else {
            OutputLanguageEvidence::UserSelected(validated_language.clone())
        };

        let filtered_result = fail_open_text_transform(result_text, |result_text| {
            let corrected_result = if !settings.custom_words.is_empty() && !is_whisper {
                apply_custom_words(
                    &result_text,
                    &settings.custom_words,
                    settings.word_correction_threshold,
                )
            } else {
                result_text
            };

            let without_fillers = remove_filler_words(
                &corrected_result,
                &output_language,
                &settings.custom_filler_words,
                settings.filler_word_removal_enabled,
            );
            normalize_transcription_output(&without_fillers)
        });

        let et = std::time::Instant::now();
        let translation_note = if settings.translate_to_english {
            " (translated)"
        } else {
            ""
        };
        info!(
            "Transcription completed in {}ms{}",
            (et - st).as_millis(),
            translation_note
        );

        let final_result = filtered_result;

        if final_result.is_empty() {
            debug!("Transcription result is empty");
        } else {
            debug!(
                "Transcription result: {} chars",
                final_result.chars().count()
            );
        }

        self.maybe_unload_immediately("transcription");

        Ok(final_result)
    }

    fn transcribe_via_anonen_cloud(
        &self,
        audio: &[f32],
        settings: &AppSettings,
        base_url: &str,
        cloud_model_id: &str,
        opus_override: Option<bool>,
        cancellation: Cancellation,
    ) -> Result<String> {
        let t_total = std::time::Instant::now();

        let use_opus = opus_override.unwrap_or(settings.remote_asr_opus_compression);
        let (encoded_bytes, payload_mime, payload_ext, payload_fmt) = if use_opus {
            match crate::audio_toolkit::encode_opus_ogg_bytes(audio) {
                Ok(bytes) => (bytes, "audio/ogg", "ogg", "Opus"),
                Err(e) => {
                    warn!("Opus encoding failed, falling back to WAV: {}", e);
                    let wav = crate::audio_toolkit::encode_wav_bytes(audio)
                        .map_err(|e| anyhow::anyhow!("Failed to encode audio as WAV: {}", e))?;
                    (wav, "audio/wav", "wav", "WAV")
                }
            }
        } else {
            let wav = crate::audio_toolkit::encode_wav_bytes(audio)
                .map_err(|e| anyhow::anyhow!("Failed to encode audio as WAV: {}", e))?;
            (wav, "audio/wav", "wav", "WAV")
        };
        let payload_bytes = encoded_bytes.len();

        let language = if settings.selected_language == "auto" {
            None
        } else {
            let hint = if settings.selected_language == "zh-Hans"
                || settings.selected_language == "zh-Hant"
            {
                "zh".to_string()
            } else {
                settings.selected_language.clone()
            };
            let accepted = self
                .model_manager
                .get_model_info(&settings.selected_model)
                .map(|info| info.supported_languages.contains(&hint))
                .unwrap_or(false);
            if accepted {
                Some(hint)
            } else {
                debug!(
                    "[anonen-cloud] language '{}' is not selectable on this model — sending auto",
                    settings.selected_language
                );
                None
            }
        };

        let model_for_request = Some(cloud_model_id.to_string());

        info!(
            "[anonen-cloud] transcribe: {} bytes {}, model={:?}, lang={:?}",
            payload_bytes, payload_fmt, model_for_request, language
        );

        const MAX_ATTEMPTS: u32 = 3;
        let backoff_ms: [u64; 3] = [0, 1000, 2000];
        let mut auth_retried = false;
        let mut seal = RecordingSealState::default();

        let pipeline_generation = crate::actions::PipelineGeneration::current(&self.app_handle);

        let policy = UploadPolicy::from_build();

        let t_key = std::time::Instant::now();
        let key_was_cached = self.enclave_keys.has_valid_cache(Self::now_seconds());

        let enclave = {
            let keys = self.enclave_keys.clone();
            let client_cell = self.http_client.clone();
            let base = base_url.to_string();
            let now = Self::now_seconds();
            crate::actions::run_cancellable_from(
                &self.app_handle,
                cancellation,
                pipeline_generation,
                move || {
                    let client = client_cell.get_or_init(|| {
                        crate::remote_asr::build_shared_client()
                            .expect("Failed to build shared HTTP client")
                    });
                    let source =
                        crate::sealed::keys_http::HttpAttestationSource::new(client, &base);
                    keys.acquire(&source, now)
                },
            )
            .map_err(|e| {
                info!("[anonen-cloud] cancelled while verifying the enclave key");
                e
            })?
        };
        info!(
            "[sealed] key ready in {}ms (cache={}, got={})",
            t_key.elapsed().as_millis(),
            if key_was_cached { "hit" } else { "miss" },
            enclave.is_some(),
        );
        let reason = if self.enclave_keys.last_refusal_was_unknown_image() {
            NoKeyReason::UnknownImage
        } else {
            NoKeyReason::Other
        };

        let mut route = match decide_upload(&policy, &mut seal, enclave, reason) {
            Ok(route) => route,
            Err(CloudSealError::Outdated) => {
                warn!("[anonen-cloud] the gateway's image is not on this build's accepted list; not sending — this app is behind");
                return Err(anyhow::Error::new(CloudSealError::Outdated));
            }
            Err(e) => {
                warn!("[anonen-cloud] no verified enclave key; not sending (SEALED_REQUIRED)");
                return Err(anyhow::Error::new(e));
            }
        };

        let audio_secs = (audio.len() as f64 / 16000.0).ceil() as u64;
        let request_timeout = std::time::Duration::from_secs((30 + audio_secs).min(120));

        for attempt in 0..MAX_ATTEMPTS {
            let request_id = uuid::Uuid::new_v4().to_string();
            let payload = AudioPayload {
                bytes: encoded_bytes.clone(),
                mime_type: payload_mime,
                file_extension: payload_ext,
                format_name: payload_fmt,
            };

            if attempt > 0 {
                let delay = backoff_ms.get(attempt as usize).copied().unwrap_or(2000);
                info!(
                    "[anonen-cloud] retry #{} after {}ms, new req_id={}",
                    attempt, delay, request_id
                );

                if !crate::actions::sleep_unless_abandoned(
                    &self.app_handle,
                    cancellation,
                    pipeline_generation,
                    std::time::Duration::from_millis(delay),
                ) {
                    info!("[anonen-cloud] session cancelled/superseded; aborting retries");
                    return Err(anyhow::anyhow!("cancelled before retry"));
                }
            }

            let client_cell = self.http_client.clone();
            let base = base_url.to_string();
            let req_id = request_id.clone();
            let lang = language.clone();
            let model = model_for_request.clone();
            let app_handle = self.app_handle.clone();
            let route_for_send = route.clone();

            let result = crate::actions::run_cancellable_from(
                &self.app_handle,
                cancellation,
                pipeline_generation,
                move || {
                    let client = client_cell.get_or_init(|| {
                        crate::remote_asr::build_shared_client()
                            .expect("Failed to build shared HTTP client")
                    });
                    let auth = app_handle.state::<AnonenCloudAuthManager>();
                    let access_token = match auth.get_access_token() {
                        Ok(t) => t,

                        Err(e) => {
                            warn!(
                                "[anonen-cloud] could not get an access token: {}",
                                crate::utils::quoted_body(&e)
                            );
                            return Err(crate::cloud_failure::token_error(e));
                        }
                    };
                    match &route_for_send {
                        UploadRoute::Sealed(enclave) => {
                            crate::remote_asr::transcribe_via_asr_gateway_v1_sealed(
                                client,
                                &base,
                                &access_token,
                                payload,
                                &req_id,
                                model.as_deref(),
                                lang.as_deref(),
                                request_timeout,
                                enclave,
                            )
                        }
                        #[cfg(feature = "plaintext-dev")]
                        UploadRoute::Plaintext(permit) => {
                            crate::remote_asr::transcribe_via_asr_gateway_v1(
                                *permit,
                                client,
                                &base,
                                &access_token,
                                payload,
                                &req_id,
                                model.as_deref(),
                                lang.as_deref(),
                                request_timeout,
                            )
                        }
                    }
                },
            )?;

            match result {
                Ok(ok) => {
                    let usage_mgr = self.app_handle.state::<UsageManager>();
                    usage_mgr.update_from_transcribe(&self.app_handle, ok.usage.clone());
                    if let Some(version) = &ok.models_version {
                        self.note_models_version(version);
                    }

                    if let Some(used) = ok.model.as_deref() {
                        *self.last_model.lock().unwrap() = Some(used.to_string());
                    }
                    info!(
                        "[anonen-cloud] transcribe ok: duration_s={} total={}ms",
                        ok.duration_s,
                        t_total.elapsed().as_millis()
                    );
                    return Ok(ok.text);
                }
                Err(err) => {
                    let status = err.http_status;
                    let code = &err.code;

                    match on_gateway_error(&mut seal, status, code) {
                        SealErrorAction::RefetchKeyAndRetry => {
                            warn!("[anonen-cloud] enclave was replaced; re-acquiring the key");
                            let client = self.http_client.get_or_init(|| {
                                crate::remote_asr::build_shared_client()
                                    .expect("Failed to build shared HTTP client")
                            });
                            let source = crate::sealed::keys_http::HttpAttestationSource::new(
                                client, base_url,
                            );
                            let renewed = self.enclave_keys.reacquire(&source, Self::now_seconds());

                            let reason = if self.enclave_keys.last_refusal_was_unknown_image() {
                                NoKeyReason::UnknownImage
                            } else {
                                NoKeyReason::Other
                            };
                            match decide_upload(&policy, &mut seal, renewed, reason) {
                                Ok(next) => {
                                    route = next;
                                    continue;
                                }
                                Err(e) => {
                                    warn!("[anonen-cloud] could not re-acquire a key after 409");
                                    return Err(anyhow::Error::new(e));
                                }
                            }
                        }
                        SealErrorAction::Stop(e) => {
                            warn!(
                                "[anonen-cloud] {} (sealed_once={}); stopping this recording",
                                code,
                                seal.sealed_once()
                            );
                            return Err(anyhow::Error::new(e));
                        }
                        SealErrorAction::StopWithGatewayError | SealErrorAction::NotSealing => {}
                    }

                    if worth_one_token_refresh(&err) && !auth_retried {
                        warn!(
                            "[anonen-cloud] {} ({}), invalidating cached token and retrying once",
                            status, code
                        );
                        self.app_handle
                            .state::<AnonenCloudAuthManager>()
                            .invalidate_access_token();
                        auth_retried = true;
                        continue;
                    }

                    if matches!(status, 502 | 503 | 408) && attempt + 1 < MAX_ATTEMPTS {
                        warn!("[anonen-cloud] {} ({}), will retry", status, code);
                        continue;
                    }

                    if status == 0 && code == "network_error" && attempt + 1 < MAX_ATTEMPTS {
                        warn!("[anonen-cloud] network error, will retry with backoff");
                        continue;
                    }

                    if status == 429 && code == "rate_limited" && attempt + 1 < MAX_ATTEMPTS {
                        warn!("[anonen-cloud] rate_limited, will retry with backoff");
                        continue;
                    }

                    return Err(match classify_terminal(&err) {
                        Terminal::SubscriptionRequired => {
                            crate::managers::usage::clear_cached_entitlement(&self.app_handle);
                            self.app_handle.state::<UsageManager>().clear();

                            let _ = self
                                .app_handle
                                .emit("anonen-cloud-checkout-required", serde_json::json!({}));
                            anyhow::Error::new(CloudSubscriptionRequiredError)
                        }

                        Terminal::CapExceeded(cap) => anyhow::Error::new(cap),
                        Terminal::AudioTooLong => {
                            warn!(
                                "[anonen-cloud] 413 {}: recording exceeded the per-request cap \
                                 (client-side auto-stop should have prevented this)",
                                if code.is_empty() {
                                    "audio_too_long"
                                } else {
                                    code
                                }
                            );
                            anyhow::Error::new(CloudAudioTooLongError)
                        }
                        Terminal::ModelRetired => {
                            warn!(
                                "[anonen-cloud] 400 {}: selected model no longer served",
                                code
                            );
                            let _ = self
                                .app_handle
                                .emit("anonen-cloud-model-invalid", serde_json::json!({}));
                            anyhow::Error::new(CloudModelInvalidError)
                        }

                        Terminal::AuthFailed => anyhow::Error::new(CloudAuthFailedError),
                        Terminal::Network(e) => anyhow::Error::new(e),
                        Terminal::Gateway(e) => anyhow::Error::new(e),
                    });
                }
            }
        }
        Err(anyhow::anyhow!(
            "あのねん: all {} attempts failed",
            MAX_ATTEMPTS
        ))
    }

    pub fn last_model(&self) -> Option<String> {
        self.last_model.lock().unwrap().clone()
    }

    fn note_models_version(&self, version: &str) {
        let changed = {
            let mut last = self.last_models_version.lock().unwrap();
            let changed = last.as_deref().map_or(false, |prev| prev != version);
            *last = Some(version.to_string());
            changed
        };
        if changed {
            info!(
                "[anonen-cloud] models_version changed to {}; refreshing catalog",
                version
            );
            self.refresh_cloud_catalog_async();
        }
    }

    fn refresh_cloud_catalog_async(&self) {
        let app_handle = self.app_handle.clone();
        let model_manager = self.model_manager.clone();
        thread::spawn(move || {
            let base_url = model_manager.anonen_cloud_base_url();
            let client = match reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    warn!("[anonen-cloud] catalog refresh: client build failed: {}", e);
                    return;
                }
            };
            let models = match crate::remote_asr::fetch_gateway_models(&client, &base_url) {
                Ok(m) => m,
                Err(e) => {
                    warn!("[anonen-cloud] catalog refresh failed: {}", e);
                    return;
                }
            };
            model_manager.register_cloud_models(&models);

            let selected = get_settings(&app_handle).selected_model;
            let selection_missing = selected
                .starts_with(crate::managers::model::ANONEN_CLOUD_MODEL_PREFIX)
                && model_manager.get_remote_config(&selected).is_none();
            let recommended_model_id = models.iter().find(|m| m.is_recommended).map(|m| {
                format!(
                    "{}{}",
                    crate::managers::model::ANONEN_CLOUD_MODEL_PREFIX,
                    m.id
                )
            });
            if selection_missing {
                warn!(
                    "[anonen-cloud] selected model {} dropped out of the refreshed catalog",
                    selected
                );
            }
            let _ = app_handle.emit(
                "anonen-cloud-models-updated",
                serde_json::json!({
                    "selection_missing": selection_missing,
                    "recommended_model_id": recommended_model_id,
                }),
            );
        });
    }

    fn transcribe_remote(
        &self,
        audio: &[f32],
        settings: &AppSettings,
        opus_override: Option<bool>,
        cancellation: Cancellation,
    ) -> Result<String> {
        let mut cfg = self
            .model_manager
            .get_remote_config(&settings.selected_model);

        if cfg.is_none()
            && settings
                .selected_model
                .starts_with(crate::managers::model::ANONEN_CLOUD_MODEL_PREFIX)
        {
            info!("[anonen-cloud] model card not registered yet, fetching /v1/models");
            let base_url = self.model_manager.anonen_cloud_base_url();

            let fetched =
                std::thread::spawn(move || -> Option<Vec<crate::remote_asr::AsrGatewayModel>> {
                    let client = reqwest::blocking::Client::builder()
                        .timeout(std::time::Duration::from_secs(10))
                        .build()
                        .ok()?;
                    crate::remote_asr::fetch_gateway_models(&client, &base_url).ok()
                })
                .join()
                .ok()
                .flatten();
            let fetch_gave_models = fetched.as_ref().map_or(false, |m| !m.is_empty());
            if let Some(models) = fetched {
                self.model_manager.register_cloud_models(&models);
                cfg = self
                    .model_manager
                    .get_remote_config(&settings.selected_model);
            }

            if cfg.is_none() {
                if fetch_gave_models {
                    let _ = self
                        .app_handle
                        .emit("anonen-cloud-model-invalid", serde_json::json!({}));
                    return Err(anyhow::Error::new(CloudModelInvalidError));
                }

                return Err(anyhow::anyhow!(
                    "あのねん: モデル一覧を取得できませんでした。しばらくしてからもう一度お試しください"
                ));
            }
        }

        let Some(config) = cfg else {
            return Err(anyhow::anyhow!(
                "no registered remote endpoint for '{}' — refusing to send audio",
                settings.selected_model
            ));
        };
        self.transcribe_via_anonen_cloud(
            audio,
            settings,
            &config.base_url,
            &config.model_id,
            opus_override,
            cancellation,
        )
    }
}

fn whisper_load_params(app: &tauri::AppHandle, model_path: &std::path::Path) -> WhisperLoadParams {
    use transcribe_rs::accel;

    let mut params = WhisperLoadParams {
        use_gpu: accel::get_whisper_accelerator().use_gpu(),
        gpu_device: accel::get_whisper_gpu_device(),
        flash_attn: get_settings(app).whisper_flash_attn,
    };

    if params.use_gpu {
        if let Some(reason) = whisper_gpu_host_block_reason() {
            warn!("loading whisper on CPU: {}", reason);
            params.use_gpu = false;
        }
    }

    if params.use_gpu {
        if let Some(notice) = whisper_gpu_shortfall(model_path, params.gpu_device) {
            warn!(
                "Not enough GPU memory for this model ({} MB needed, {} MB available); using CPU",
                notice.needed_mb, notice.available_mb
            );
            params.use_gpu = false;
            if crate::gpu_guard::VRAM_NOTICE.take() {
                let _ = app.emit(crate::gpu_guard::GPU_FALLBACK_EVENT, notice);
            }
        }
    } else if crate::gpu_guard::gpu_disabled_this_run()
        && crate::gpu_guard::PREVIOUS_CRASH_NOTICE.take()
    {
        let _ = app.emit(
            crate::gpu_guard::GPU_FALLBACK_EVENT,
            crate::gpu_guard::GpuFallbackNotice {
                reason: crate::gpu_guard::REASON_PREVIOUS_CRASH,
                needed_mb: 0,
                available_mb: 0,
            },
        );
    }

    params
}

fn whisper_gpu_shortfall(
    model_path: &std::path::Path,
    gpu_device: i32,
) -> Option<crate::gpu_guard::GpuFallbackNotice> {
    use transcribe_rs::whisper_cpp::gpu::{list_gpu_devices, GpuKind};

    if cached_gpu_devices().is_empty() {
        return None;
    }

    let model_bytes = std::fs::metadata(model_path).ok()?.len();

    let devices = list_gpu_devices();
    let device = if gpu_device >= 0 {
        devices.iter().find(|d| d.id == gpu_device)
    } else {
        devices
            .iter()
            .max_by_key(|d| (d.kind == GpuKind::Dedicated, d.total_vram))
    }?;

    match crate::gpu_guard::whisper_gpu_fit(
        model_bytes,
        device.free_vram as u64,
        device.total_vram as u64,
    ) {
        crate::gpu_guard::GpuFit::TooSmall {
            needed_mb,
            available_mb,
        } => Some(crate::gpu_guard::GpuFallbackNotice {
            reason: crate::gpu_guard::REASON_VRAM,
            needed_mb,
            available_mb,
        }),
        crate::gpu_guard::GpuFit::Fits | crate::gpu_guard::GpuFit::Unknown => None,
    }
}

fn gpu_attempt_marker(app: &tauri::AppHandle) -> Option<crate::gpu_guard::GpuAttemptMarker> {
    let log_dir = crate::portable::app_log_dir(app).ok()?;
    crate::gpu_guard::GpuAttemptMarker::begin(&log_dir)
}

pub fn apply_accelerator_settings(app: &tauri::AppHandle) {
    use transcribe_rs::accel;

    let settings = get_settings(app);

    let host_block = whisper_gpu_host_block_reason();
    if let Some(reason) = host_block {
        warn!(
            "disabling whisper GPU acceleration and using CPU: {}",
            reason
        );
    }

    let after_crash = crate::gpu_guard::gpu_disabled_this_run();
    let gpu_disabled = host_block.is_some() || after_crash;
    let whisper_pref =
        match effective_whisper_accelerator(settings.whisper_accelerator, gpu_disabled) {
            WhisperAcceleratorSetting::Auto => accel::WhisperAccelerator::Auto,
            WhisperAcceleratorSetting::Cpu => accel::WhisperAccelerator::CpuOnly,
            WhisperAcceleratorSetting::Gpu => accel::WhisperAccelerator::Gpu,
        };
    let whisper_gpu_device = if gpu_disabled {
        accel::GPU_DEVICE_AUTO
    } else {
        settings.whisper_gpu_device
    };
    accel::set_whisper_accelerator(whisper_pref);
    accel::set_whisper_gpu_device(whisper_gpu_device);
    info!(
        "Whisper accelerator set to: {}, gpu_device: {}",
        whisper_pref,
        if whisper_gpu_device == accel::GPU_DEVICE_AUTO {
            "auto".to_string()
        } else {
            whisper_gpu_device.to_string()
        }
    );

    let ort_pref = match settings.ort_accelerator {
        OrtAcceleratorSetting::Auto => accel::OrtAccelerator::Auto,
        OrtAcceleratorSetting::Cpu => accel::OrtAccelerator::CpuOnly,
        OrtAcceleratorSetting::Cuda => accel::OrtAccelerator::Cuda,
        OrtAcceleratorSetting::DirectMl => accel::OrtAccelerator::DirectMl,
        OrtAcceleratorSetting::Rocm => accel::OrtAccelerator::Rocm,
    };
    accel::set_ort_accelerator(ort_pref);
    info!("ORT accelerator set to: {}", ort_pref);
}

#[derive(Serialize, Clone, Debug, Type)]
pub struct GpuDeviceOption {
    pub id: i32,
    pub name: String,
    pub total_vram_mb: usize,
}

static GPU_DEVICES: OnceLock<Vec<GpuDeviceOption>> = OnceLock::new();

fn whisper_gpu_host_block_reason() -> Option<&'static str> {
    #[cfg(target_arch = "x86_64")]
    let has_fma3 = std::arch::is_x86_feature_detected!("fma");
    #[cfg(not(target_arch = "x86_64"))]
    let has_fma3 = true;

    crate::gpu_guard::gpu_host_block_reason(
        has_fma3,
        crate::utils::is_windows_x64_emulated_on_arm64(),
    )
}

fn whisper_gpu_disabled_for_host() -> bool {
    whisper_gpu_host_block_reason().is_some()
}

fn effective_whisper_accelerator(
    setting: WhisperAcceleratorSetting,
    gpu_disabled: bool,
) -> WhisperAcceleratorSetting {
    if gpu_disabled {
        WhisperAcceleratorSetting::Cpu
    } else {
        setting
    }
}

fn available_whisper_accelerators(gpu_disabled: bool) -> Vec<String> {
    if gpu_disabled {
        vec!["cpu".to_string()]
    } else {
        vec!["auto".to_string(), "cpu".to_string(), "gpu".to_string()]
    }
}

fn cached_gpu_devices() -> &'static [GpuDeviceOption] {
    use transcribe_rs::whisper_cpp::gpu::list_gpu_devices;

    GPU_DEVICES.get_or_init(|| {
        if let Some(reason) = whisper_gpu_host_block_reason() {
            warn!("skipping GPU device enumeration: {}", reason);
            return Vec::new();
        }

        list_gpu_devices()
            .into_iter()
            .map(|d| GpuDeviceOption {
                id: d.id,
                name: d.name,
                total_vram_mb: d.total_vram / (1024 * 1024),
            })
            .collect()
    })
}

#[derive(Serialize, Clone, Debug, Type)]
pub struct AvailableAccelerators {
    pub whisper: Vec<String>,
    pub ort: Vec<String>,
    pub gpu_devices: Vec<GpuDeviceOption>,
}

pub fn get_available_accelerators() -> AvailableAccelerators {
    use transcribe_rs::accel::OrtAccelerator;

    let ort_options: Vec<String> = OrtAccelerator::available()
        .into_iter()
        .map(|a| a.to_string())
        .collect();

    let whisper_options = available_whisper_accelerators(whisper_gpu_disabled_for_host());

    AvailableAccelerators {
        whisper: whisper_options,
        ort: ort_options,
        gpu_devices: cached_gpu_devices().to_vec(),
    }
}

impl Drop for TranscriptionManager {
    fn drop(&mut self) {
        if Arc::strong_count(&self.engine) > 1 {
            return;
        }

        self.shutdown_signal.store(true, Ordering::Relaxed);

        let mut guard = match self.watcher_handle.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if let Some(handle) = guard.take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join idle watcher thread: {:?}", e);
            } else {
                debug!("Idle watcher thread joined successfully");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        available_whisper_accelerators, effective_whisper_accelerator, fail_open_text_transform,
    };
    use crate::settings::WhisperAcceleratorSetting;

    #[test]
    fn normal_hosts_preserve_every_whisper_accelerator_setting() {
        for setting in [
            WhisperAcceleratorSetting::Auto,
            WhisperAcceleratorSetting::Cpu,
            WhisperAcceleratorSetting::Gpu,
        ] {
            assert_eq!(effective_whisper_accelerator(setting, false), setting);
        }
        assert_eq!(
            available_whisper_accelerators(false),
            ["auto", "cpu", "gpu"]
        );
    }

    #[test]
    fn emulated_x64_on_arm64_forces_every_whisper_setting_to_cpu() {
        for setting in [
            WhisperAcceleratorSetting::Auto,
            WhisperAcceleratorSetting::Cpu,
            WhisperAcceleratorSetting::Gpu,
        ] {
            assert_eq!(
                effective_whisper_accelerator(setting, true),
                WhisperAcceleratorSetting::Cpu
            );
        }
        assert_eq!(available_whisper_accelerators(true), ["cpu"]);
    }

    #[test]
    fn optional_text_transform_falls_back_to_raw_text_after_panic() {
        let raw = "原始轉錄。".to_string();
        let result = fail_open_text_transform(raw.clone(), |_| {
            panic!("simulated optional cleanup failure")
        });

        assert_eq!(result, raw);
    }
}
