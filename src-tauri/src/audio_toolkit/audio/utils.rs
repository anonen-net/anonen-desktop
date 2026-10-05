use anyhow::Result;
use hound::{WavReader, WavSpec, WavWriter};
use log::debug;
use std::io::Cursor;
use std::path::Path;

pub fn read_wav_samples<P: AsRef<Path>>(file_path: P) -> Result<Vec<f32>> {
    let reader = WavReader::open(file_path.as_ref())?;
    let samples = reader
        .into_samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
        .collect::<Result<Vec<f32>, _>>()?;
    Ok(samples)
}

pub fn verify_wav_file<P: AsRef<Path>>(file_path: P, expected_samples: usize) -> Result<()> {
    let reader = WavReader::open(file_path.as_ref())?;
    let actual_samples = reader.len() as usize;
    if actual_samples != expected_samples {
        anyhow::bail!(
            "WAV sample count mismatch: expected {}, got {}",
            expected_samples,
            actual_samples
        );
    }
    Ok(())
}

pub fn save_wav_file<P: AsRef<Path>>(file_path: P, samples: &[f32]) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(file_path.as_ref(), spec)?;

    for sample in samples {
        let sample_i16 = (sample * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16)?;
    }

    writer.finalize()?;
    debug!("Saved WAV file: {:?}", file_path.as_ref());
    Ok(())
}

pub fn encode_wav_bytes(samples: &[f32]) -> Result<Vec<u8>> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut writer = WavWriter::new(&mut cursor, spec)?;
        for sample in samples {
            let sample_i16 = (sample * i16::MAX as f32) as i16;
            writer.write_sample(sample_i16)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

pub fn normalize_quiet_audio(samples: &mut [f32]) -> f32 {
    const BOOST_BELOW: f32 = 0.04;

    const TARGET_RMS: f32 = 0.08;

    const MIN_RMS: f32 = 0.005;

    const MAX_GAIN: f32 = 8.0;

    const CEILING: f32 = 0.95;

    if samples.is_empty() {
        return 1.0;
    }
    let mut sum_sq = 0.0f64;
    let mut peak = 0.0f32;
    for &s in samples.iter() {
        sum_sq += f64::from(s) * f64::from(s);
        let a = s.abs();
        if a > peak {
            peak = a;
        }
    }
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
    if rms < MIN_RMS || rms >= BOOST_BELOW || peak <= 0.0 {
        return 1.0;
    }
    let wanted = TARGET_RMS / rms;

    if wanted <= 1.0 {
        return 1.0;
    }
    let gain = wanted.min(MAX_GAIN).min(CEILING / peak);
    if gain <= 1.0 {
        return 1.0;
    }
    for s in samples.iter_mut() {
        *s *= gain;
    }
    gain
}

#[cfg(test)]
mod normalize_tests {
    use super::normalize_quiet_audio;

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>() / x.len() as f64).sqrt() as f32
    }

    fn sine(amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (i as f32 * 0.1).sin()).collect()
    }

    #[test]
    fn quiet_audio_is_brought_up_to_the_target() {
        let mut x = sine(0.02, 4000);
        let gain = normalize_quiet_audio(&mut x);
        assert!(gain > 1.0, "gain={gain}");

        assert!((rms(&x) - 0.08).abs() < 0.02, "rms={}", rms(&x));
    }

    #[test]
    fn audio_in_the_normal_range_is_left_alone() {
        let mut x = sine(0.09, 4000);
        let before = x.clone();
        assert_eq!(normalize_quiet_audio(&mut x), 1.0);
        assert_eq!(x, before);
    }

    #[test]
    fn loud_audio_is_left_alone() {
        let mut x = sine(0.9, 4000);
        let before = x.clone();
        assert_eq!(normalize_quiet_audio(&mut x), 1.0);
        assert_eq!(x, before);
    }

    #[test]
    fn silence_is_not_amplified() {
        let mut x = vec![0.0f32; 4000];
        assert_eq!(normalize_quiet_audio(&mut x), 1.0);
        assert!(x.iter().all(|&s| s == 0.0));

        let mut n = sine(0.002, 4000);
        assert_eq!(normalize_quiet_audio(&mut n), 1.0);
    }

    #[test]
    fn boost_never_pushes_the_peak_into_clipping() {
        let mut x = vec![0.001f32; 4000];
        x[0] = 0.5;
        x[1] = -0.5;
        normalize_quiet_audio(&mut x);
        let peak = x.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak <= 0.95 + 1e-6, "peak={peak}");
    }

    #[test]
    fn empty_input_is_safe() {
        let mut x: Vec<f32> = Vec::new();
        assert_eq!(normalize_quiet_audio(&mut x), 1.0);
    }
}
