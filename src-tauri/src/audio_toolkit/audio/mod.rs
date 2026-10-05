mod device;
pub mod opus;
mod recorder;
mod resampler;
mod utils;
mod visualizer;

pub use device::{list_input_devices, list_output_devices, CpalDeviceInfo};
pub use opus::encode_opus_ogg_bytes;
pub use recorder::{
    is_microphone_access_denied, is_no_input_device_error, AudioRecorder, InputLevel,
};
pub use resampler::FrameResampler;
pub use utils::{
    encode_wav_bytes, normalize_quiet_audio, read_wav_samples, save_wav_file, verify_wav_file,
};
pub use visualizer::AudioVisualiser;
