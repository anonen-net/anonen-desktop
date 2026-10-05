export const commands = {
  async changeBinding(
    id: string,
    binding: string,
  ): Promise<Result<BindingResponse, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_binding", { id, binding }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async resetBinding(id: string): Promise<Result<BindingResponse, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("reset_binding", { id }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changePttSetting(enabled: boolean): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_ptt_setting", { enabled }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeAudioFeedbackSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_audio_feedback_setting", { enabled }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeAudioFeedbackVolumeSetting(
    volume: number,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_audio_feedback_volume_setting", {
          volume,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeSoundThemeSetting(theme: string): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_sound_theme_setting", { theme }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeStartHiddenSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_start_hidden_setting", { enabled }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeAutostartSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_autostart_setting", { enabled }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeTranslateToEnglishSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_translate_to_english_setting", {
          enabled,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeSelectedLanguageSetting(
    language: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_selected_language_setting", {
          language,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeOverlayPositionSetting(
    position: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_overlay_position_setting", {
          position,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeDebugModeSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_debug_mode_setting", { enabled }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeWordCorrectionThresholdSetting(
    threshold: number,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_word_correction_threshold_setting", {
          threshold,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeExtraRecordingBufferSetting(
    ms: number,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_extra_recording_buffer_setting", {
          ms,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changePasteDelayMsSetting(ms: number): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_paste_delay_ms_setting", { ms }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changePasteDelayAfterMsSetting(
    ms: number,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_paste_delay_after_ms_setting", { ms }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeReliablePasteSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_reliable_paste_setting", { enabled }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getAvailableTypingTools(): Promise<string[]> {
    return await TAURI_INVOKE("get_available_typing_tools");
  },
  async changeTypingToolSetting(tool: string): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_typing_tool_setting", { tool }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeClipboardHandlingSetting(
    handling: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_clipboard_handling_setting", {
          handling,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeAutoSubmitSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_auto_submit_setting", { enabled }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeAutoSubmitKeySetting(key: string): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_auto_submit_key_setting", { key }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changePostProcessEnabledSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_post_process_enabled_setting", {
          enabled,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeExperimentalEnabledSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_experimental_enabled_setting", {
          enabled,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changePostProcessBaseUrlSetting(
    providerId: string,
    baseUrl: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_post_process_base_url_setting", {
          providerId,
          baseUrl,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changePostProcessApiKeySetting(
    providerId: string,
    apiKey: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_post_process_api_key_setting", {
          providerId,
          apiKey,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changePostProcessModelSetting(
    providerId: string,
    model: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_post_process_model_setting", {
          providerId,
          model,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async setPostProcessProvider(
    providerId: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("set_post_process_provider", { providerId }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeRemoteAsrOpusCompressionSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_remote_asr_opus_compression_setting", {
          enabled,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async resetAllSettings(): Promise<Result<null, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("reset_all_settings") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async fetchPostProcessModels(
    providerId: string,
  ): Promise<Result<string[], string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("fetch_post_process_models", { providerId }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async addPostProcessPrompt(
    name: string,
    prompt: string,
  ): Promise<Result<LLMPrompt, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("add_post_process_prompt", { name, prompt }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async updatePostProcessPrompt(
    id: string,
    name: string,
    prompt: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("update_post_process_prompt", {
          id,
          name,
          prompt,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async deletePostProcessPrompt(id: string): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("delete_post_process_prompt", { id }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async setPostProcessSelectedPrompt(
    id: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("set_post_process_selected_prompt", { id }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async updateCustomWords(words: string[]): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("update_custom_words", { words }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async suspendAllBindings(): Promise<Result<null, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("suspend_all_bindings") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async resumeAllBindings(): Promise<Result<null, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("resume_all_bindings") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeMuteWhileRecordingSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_mute_while_recording_setting", {
          enabled,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeAppendTrailingSpaceSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_append_trailing_space_setting", {
          enabled,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeLazyStreamCloseSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_lazy_stream_close_setting", {
          enabled,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeFillerWordRemovalEnabledSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_filler_word_removal_enabled_setting", {
          enabled,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeAppLanguageSetting(
    language: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_app_language_setting", { language }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeUpdateChecksSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_update_checks_setting", { enabled }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async changeKeyboardImplementationSetting(
    implementation: string,
  ): Promise<Result<ImplementationChangeResult, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_keyboard_implementation_setting", {
          implementation,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async getKeyboardImplementation(): Promise<string> {
    return await TAURI_INVOKE("get_keyboard_implementation");
  },

  async acknowledgeModelDataPolicy(
    fingerprint: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("acknowledge_model_data_policy", {
          fingerprint,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeShowTrayIconSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_show_tray_icon_setting", { enabled }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeWhisperAcceleratorSetting(
    accelerator: WhisperAcceleratorSetting,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_whisper_accelerator_setting", {
          accelerator,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeOrtAcceleratorSetting(
    accelerator: OrtAcceleratorSetting,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_ort_accelerator_setting", {
          accelerator,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async changeWhisperGpuDevice(device: number): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_whisper_gpu_device", { device }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async changeWhisperFlashAttnSetting(
    enabled: boolean,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("change_whisper_flash_attn_setting", {
          enabled,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getAvailableAccelerators(): Promise<AvailableAccelerators> {
    return await TAURI_INVOKE("get_available_accelerators");
  },

  async startHandyKeysRecording(
    bindingId: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("start_handy_keys_recording", { bindingId }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async stopHandyKeysRecording(): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("stop_handy_keys_recording"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async triggerUpdateCheck(): Promise<Result<null, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("trigger_update_check") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async showMainWindowCommand(): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("show_main_window_command"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async cancelOperation(): Promise<void> {
    await TAURI_INVOKE("cancel_operation");
  },
  async isPortable(): Promise<boolean> {
    return await TAURI_INVOKE("is_portable");
  },
  async getAppDirPath(): Promise<Result<string, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("get_app_dir_path") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async getAppSettings(): Promise<Result<AppSettings, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("get_app_settings") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getDefaultSettings(): Promise<Result<AppSettings, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("get_default_settings") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getLogDirPath(): Promise<Result<string, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("get_log_dir_path") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async readRecentLogs(maxLines: number): Promise<Result<string[], string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("read_recent_logs", { maxLines }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async readNativeStderrLog(
    maxLines: number,
  ): Promise<Result<string[], string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("read_native_stderr_log", { maxLines }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async setLogLevel(level: LogLevel): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("set_log_level", { level }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async openRecordingsFolder(): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("open_recordings_folder"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async openLogDir(): Promise<Result<null, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("open_log_dir") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async openAppDataDir(): Promise<Result<null, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("open_app_data_dir") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async initializeEnigo(): Promise<Result<null, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("initialize_enigo") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async initializeShortcuts(): Promise<Result<null, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("initialize_shortcuts") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getAvailableModels(): Promise<Result<ModelInfo[], string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("get_available_models") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getModelInfo(
    modelId: string,
  ): Promise<Result<ModelInfo | null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_model_info", { modelId }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async downloadModel(modelId: string): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("download_model", { modelId }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async cancelDownload(modelId: string): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("cancel_download", { modelId }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async deleteModel(modelId: string): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("delete_model", { modelId }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async setActiveModel(modelId: string): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("set_active_model", { modelId }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getCurrentModel(): Promise<Result<string, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("get_current_model") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async clearActiveModelCommand(): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("clear_active_model_command"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getTranscriptionModelStatus(): Promise<Result<string | null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_transcription_model_status"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async isModelLoading(): Promise<Result<boolean, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("is_model_loading") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async hasAnyModelsAvailable(): Promise<Result<boolean, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("has_any_models_available"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async hasAnyModelsOrDownloads(): Promise<Result<boolean, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("has_any_models_or_downloads"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async updateMicrophoneMode(alwaysOn: boolean): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("update_microphone_mode", { alwaysOn }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getMicrophoneMode(): Promise<Result<boolean, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("get_microphone_mode") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getWindowsMicrophonePermissionStatus(): Promise<WindowsMicrophonePermissionStatus> {
    return await TAURI_INVOKE("get_windows_microphone_permission_status");
  },
  async openMicrophonePrivacySettings(): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("open_microphone_privacy_settings"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getAvailableMicrophones(): Promise<Result<AudioDevice[], string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_available_microphones"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async setSelectedMicrophone(
    deviceName: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("set_selected_microphone", { deviceName }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getSelectedMicrophone(): Promise<Result<string, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_selected_microphone"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getAvailableOutputDevices(): Promise<Result<AudioDevice[], string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_available_output_devices"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async setSelectedOutputDevice(
    deviceName: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("set_selected_output_device", { deviceName }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getSelectedOutputDevice(): Promise<Result<string, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_selected_output_device"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async playTestSound(soundType: string): Promise<void> {
    await TAURI_INVOKE("play_test_sound", { soundType });
  },
  async checkCustomSounds(): Promise<CustomSounds> {
    return await TAURI_INVOKE("check_custom_sounds");
  },
  async setClamshellMicrophone(
    deviceName: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("set_clamshell_microphone", { deviceName }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getClamshellMicrophone(): Promise<Result<string, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_clamshell_microphone"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async isRecording(): Promise<boolean> {
    return await TAURI_INVOKE("is_recording");
  },
  async getLastRecordingLevel(): Promise<number | null> {
    return await TAURI_INVOKE("get_last_recording_level");
  },

  async getLastInputLevel(): Promise<InputLevelReport | null> {
    return await TAURI_INVOKE("get_last_input_level");
  },

  async getInputVolume(): Promise<Result<number, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("get_input_volume") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async setInputVolume(value: number): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("set_input_volume", { value }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getMicrophoneChannels(
    deviceName: string,
  ): Promise<Result<number, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_microphone_channels", { deviceName }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async setSelectedChannel(
    channel: number | null,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("set_selected_channel", { channel }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async setModelUnloadTimeout(timeout: ModelUnloadTimeout): Promise<void> {
    await TAURI_INVOKE("set_model_unload_timeout", { timeout });
  },
  async getModelLoadStatus(): Promise<Result<ModelLoadStatus, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_model_load_status"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async unloadModelManually(): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("unload_model_manually"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getHistoryEntries(
    cursor: number | null,
    limit: number | null,
    savedOnly: boolean,
  ): Promise<Result<PaginatedHistory, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_history_entries", {
          cursor,
          limit,
          savedOnly,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async toggleHistoryEntrySaved(id: number): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("toggle_history_entry_saved", { id }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async getAudioFilePath(fileName: string): Promise<Result<string, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_audio_file_path", { fileName }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async deleteHistoryEntry(id: number): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("delete_history_entry", { id }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async prefetchEnclaveKey(): Promise<Result<null, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("prefetch_enclave_key") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async retryHistoryEntryTranscription(
    id: number,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("retry_history_entry_transcription", { id }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async updateHistoryRetention(
    retention: string,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("update_history_retention", { retention }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async clearAllHistory(): Promise<Result<number, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("clear_all_history") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async getHistoryAudioUsage(): Promise<Result<HistoryAudioUsage, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("get_history_audio_usage"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async anonenCloudAuthStatus(): Promise<
    Result<AnonenCloudAuthStatus, string>
  > {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("anonen_cloud_auth_status"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async anonenCloudRequestOtp(
    email: string,
    captchaToken: string | null,
  ): Promise<Result<null, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("anonen_cloud_request_otp", {
          email,
          captchaToken,
        }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async anonenCloudVerifyOtp(
    email: string,
    token: string,
  ): Promise<Result<AnonenCloudAuthStatus, string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("anonen_cloud_verify_otp", { email, token }),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
  async anonenCloudLogout(): Promise<Result<AnonenCloudAuthStatus, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("anonen_cloud_logout") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async anonenCloudCurrentUsage(): Promise<
    Result<AnonenCloudUsageSnapshot | null, string>
  > {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("anonen_cloud_current_usage"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async anonenCloudCachedSubscriptionStatus(): Promise<
    Result<string | null, string>
  > {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("anonen_cloud_cached_subscription_status"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async anonenCloudFetchModels(): Promise<Result<AsrGatewayModel[], string>> {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("anonen_cloud_fetch_models"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async anonenCloudFetchUsage(): Promise<
    Result<AnonenCloudUsageSnapshot, string>
  > {
    try {
      return {
        status: "ok",
        data: await TAURI_INVOKE("anonen_cloud_fetch_usage"),
      };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },

  async isLaptop(): Promise<Result<boolean, string>> {
    try {
      return { status: "ok", data: await TAURI_INVOKE("is_laptop") };
    } catch (e) {
      if (e instanceof Error) throw e;
      else return { status: "error", error: e as any };
    }
  },
};

export const events = __makeEvents__<{
  historyUpdatePayload: HistoryUpdatePayload;
}>({
  historyUpdatePayload: "history-update-payload",
});

export type AnonenCloudAuthStatus = {
  signed_in: boolean;
  email: string | null;

  user_id: string | null;
};
export type AnonenCloudPlan = {
  name: string;
  week_cap_s: number;
  month_cap_s: number;
};
export type AnonenCloudSubscription = {
  status: string;
  current_period_end: string;
};
export type AnonenCloudUsageSnapshot = {
  usage: AsrGatewayUsage;
  plan: AnonenCloudPlan | null;
  subscription: AnonenCloudSubscription | null;
};
export type AppSettings = {
  bindings: Partial<{ [key in string]: ShortcutBinding }>;
  push_to_talk: boolean;
  audio_feedback: boolean;
  audio_feedback_volume?: number;
  sound_theme?: SoundTheme;
  start_hidden?: boolean;
  autostart_enabled?: boolean;
  update_checks_enabled?: boolean;
  selected_model?: string;
  always_on_microphone?: boolean;
  selected_microphone?: string | null;

  selected_channel?: number | null;
  clamshell_microphone?: string | null;
  selected_output_device?: string | null;
  translate_to_english?: boolean;
  selected_language?: string;
  overlay_position?: OverlayPosition;
  debug_mode?: boolean;
  log_level?: LogLevel;
  custom_words?: string[];
  model_unload_timeout?: ModelUnloadTimeout;
  word_correction_threshold?: number;

  history_enabled?: boolean | null;

  history_retention?: HistoryRetention | null;
  paste_method?: PasteMethod;
  clipboard_handling?: ClipboardHandling;
  auto_submit?: boolean;
  auto_submit_key?: AutoSubmitKey;
  post_process_enabled?: boolean;
  post_process_provider_id?: string;
  post_process_providers?: PostProcessProvider[];
  post_process_api_keys?: SecretMap;
  post_process_models?: Partial<{ [key in string]: string }>;
  post_process_prompts?: LLMPrompt[];
  post_process_selected_prompt_id?: string | null;
  mute_while_recording?: boolean;
  append_trailing_space?: boolean;
  app_language?: string;
  experimental_enabled?: boolean;
  lazy_stream_close?: boolean;
  keyboard_implementation?: KeyboardImplementation;
  show_tray_icon?: boolean;
  paste_delay_ms?: number;
  paste_delay_after_ms?: number;

  reliable_paste?: boolean;
  typing_tool?: TypingTool;
  external_script_path: string | null;
  filler_word_removal_enabled?: boolean;
  custom_filler_words?: string[] | null;
  whisper_accelerator?: WhisperAcceleratorSetting;
  ort_accelerator?: OrtAcceleratorSetting;
  whisper_gpu_device?: number;

  whisper_flash_attn?: boolean;
  extra_recording_buffer_ms?: number;
  remote_asr_opus_compression?: boolean;

  acknowledged_cloud_policies?: string[];
};

export type AsrGatewayModel = {
  id: string;
  display_name: string;
  provider: string;
  description: string;
  accuracy_score?: number;
  speed_score?: number;
  price_per_hour?: number;

  usage_multiplier?: number;

  supported_languages?: string[];
  supports_translation?: boolean;
  provider_label?: string;
  is_recommended?: boolean;

  training_use?: string;

  retention_kind?: string;

  retention_days?: number | null;
};

export type AsrGatewayUsage = {
  week_used_s: number;
  week_cap_s: number;
  week_resets_at: string;
  month_used_s: number;
  month_cap_s: number;
  month_resets_at: string;

  max_request_s?: number | null;

  week_cap_full_s?: number | null;
};
export type AudioDevice = { index: string; name: string; is_default: boolean };
export type AutoSubmitKey = "enter" | "ctrl_enter" | "cmd_enter";
export type AvailableAccelerators = {
  whisper: string[];
  ort: string[];
  gpu_devices: GpuDeviceOption[];
};
export type BindingResponse = {
  success: boolean;
  binding: ShortcutBinding | null;
  error: string | null;
};
export type ClipboardHandling = "dont_modify" | "copy_to_clipboard";
export type CustomSounds = { start: boolean; stop: boolean };
export type EngineType =
  | "Whisper"
  | "Parakeet"
  | "Moonshine"
  | "MoonshineStreaming"
  | "SenseVoice"
  | "GigaAM"
  | "Canary"
  | "Cohere"
  | "Remote";
export type GpuDeviceOption = {
  id: number;
  name: string;
  total_vram_mb: number;
};

export type HistoryAudioUsage = { count: number; bytes: number };
export type HistoryEntry = {
  id: number;
  file_name: string;
  timestamp: number;
  saved: boolean;
  title: string;
  transcription_text: string;
  post_processed_text: string | null;
  post_process_prompt: string | null;
  post_process_requested: boolean;

  model: string | null;

  revisions: Revision[];

  transcribed_at: number | null;

  has_audio: boolean;
};

export type HistoryRetention = "none" | "recent" | "unlimited";
export type HistoryUpdatePayload =
  | { action: "added"; entry: HistoryEntry }
  | { action: "updated"; entry: HistoryEntry }
  | { action: "deleted"; id: number }
  | { action: "toggled"; id: number };

export type ImplementationChangeResult = {
  success: boolean;

  reset_bindings: string[];
};

export type InputLevelReport = {
  clip_ratio: number;

  peak: number;

  clipping: boolean;
};
export type KeyboardImplementation = "tauri" | "handy_keys";
export type LLMPrompt = { id: string; name: string; prompt: string };
export type LogLevel = "trace" | "debug" | "info" | "warn" | "error";
export type ModelInfo = {
  id: string;
  name: string;
  description: string;
  filename: string;
  url: string | null;
  sha256: string | null;
  size_mb: number;
  is_downloaded: boolean;
  is_downloading: boolean;
  partial_size: number;
  is_directory: boolean;
  engine_type: EngineType;
  accuracy_score: number;
  speed_score: number;
  supports_translation: boolean;
  is_recommended: boolean;
  supported_languages: string[];
  supports_language_selection: boolean;
  is_custom: boolean;

  provider_label: string;

  price_per_hour: number;

  usage_multiplier?: number;
};
export type ModelLoadStatus = {
  is_loaded: boolean;
  current_model: string | null;
};
export type ModelUnloadTimeout =
  | "never"
  | "immediately"
  | "min_2"
  | "min_5"
  | "min_10"
  | "min_15"
  | "hour_1"
  | "sec_15";
export type OrtAcceleratorSetting =
  | "auto"
  | "cpu"
  | "cuda"
  | "directml"
  | "rocm";
export type OverlayPosition = "none" | "top" | "bottom";
export type PaginatedHistory = { entries: HistoryEntry[]; has_more: boolean };
export type PasteMethod =
  | "ctrl_v"
  | "direct"
  | "none"
  | "shift_insert"
  | "ctrl_shift_v"
  | "external_script";
export type PermissionAccess = "allowed" | "denied" | "unknown";
export type PostProcessProvider = {
  id: string;
  label: string;
  base_url: string;
  allow_base_url_edit?: boolean;
  models_endpoint?: string | null;
  supports_structured_output?: boolean;
};

export type Revision = {
  text: string;

  model: string | null;

  transcribed_at?: number | null;
};
export type SecretMap = Partial<{ [key in string]: string }>;
export type ShortcutBinding = {
  id: string;
  name: string;
  description: string;
  default_binding: string;
  current_binding: string;
};
export type SoundTheme = "marimba" | "pop" | "custom";
export type TypingTool =
  | "auto"
  | "wtype"
  | "kwtype"
  | "dotool"
  | "ydotool"
  | "xdotool";
export type WhisperAcceleratorSetting = "auto" | "cpu" | "gpu";
export type WindowsMicrophonePermissionStatus = {
  supported: boolean;
  overall_access: PermissionAccess;
  device_access: PermissionAccess;
  app_access: PermissionAccess;
  desktop_app_access: PermissionAccess;
};

import {
  invoke as TAURI_INVOKE,
  Channel as TAURI_CHANNEL,
} from "@tauri-apps/api/core";
import * as TAURI_API_EVENT from "@tauri-apps/api/event";
import { type WebviewWindow as __WebviewWindow__ } from "@tauri-apps/api/webviewWindow";

type __EventObj__<T> = {
  listen: (
    cb: TAURI_API_EVENT.EventCallback<T>,
  ) => ReturnType<typeof TAURI_API_EVENT.listen<T>>;
  once: (
    cb: TAURI_API_EVENT.EventCallback<T>,
  ) => ReturnType<typeof TAURI_API_EVENT.once<T>>;
  emit: null extends T
    ? (payload?: T) => ReturnType<typeof TAURI_API_EVENT.emit>
    : (payload: T) => ReturnType<typeof TAURI_API_EVENT.emit>;
};

export type Result<T, E> =
  | { status: "ok"; data: T }
  | { status: "error"; error: E };

function __makeEvents__<T extends Record<string, any>>(
  mappings: Record<keyof T, string>,
) {
  return new Proxy(
    {} as unknown as {
      [K in keyof T]: __EventObj__<T[K]> & {
        (handle: __WebviewWindow__): __EventObj__<T[K]>;
      };
    },
    {
      get: (_, event) => {
        const name = mappings[event as keyof T];

        return new Proxy((() => {}) as any, {
          apply: (_, __, [window]: [__WebviewWindow__]) => ({
            listen: (arg: any) => window.listen(name, arg),
            once: (arg: any) => window.once(name, arg),
            emit: (arg: any) => window.emit(name, arg),
          }),
          get: (_, command: keyof __EventObj__<any>) => {
            switch (command) {
              case "listen":
                return (arg: any) => TAURI_API_EVENT.listen(name, arg);
              case "once":
                return (arg: any) => TAURI_API_EVENT.once(name, arg);
              case "emit":
                return (arg: any) => TAURI_API_EVENT.emit(name, arg);
            }
          },
        });
      },
    },
  );
}
