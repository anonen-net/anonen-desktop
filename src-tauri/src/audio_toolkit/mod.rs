pub mod audio;
pub mod constants;
pub mod text;
pub mod utils;
pub mod vad;

pub use audio::{
    encode_opus_ogg_bytes, encode_wav_bytes, is_microphone_access_denied, is_no_input_device_error,
    list_input_devices, list_output_devices, normalize_quiet_audio, read_wav_samples,
    save_wav_file, verify_wav_file, AudioRecorder, CpalDeviceInfo, InputLevel,
};
pub use text::{
    apply_custom_words, normalize_transcription_output, remove_filler_words, OutputLanguageEvidence,
};
pub use utils::get_cpal_host;
pub use vad::{SileroVad, VoiceActivityDetector};
