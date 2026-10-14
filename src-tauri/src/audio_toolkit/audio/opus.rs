use anyhow::{ensure, Result};
use audiopus::coder::{Decoder, Encoder};
use audiopus::{Application, Bitrate, Channels, SampleRate};
use log::debug;
use ogg::reading::PacketReader;
use ogg::writing::{PacketWriteEndInfo, PacketWriter};
use std::io::Cursor;

const SAMPLE_RATE: usize = 16000;
const FRAME_MS: usize = 20;
const FRAME_SIZE: usize = SAMPLE_RATE * FRAME_MS / 1000;

pub fn encode_opus_ogg_bytes(samples: &[f32]) -> Result<Vec<u8>> {
    let mut encoder = Encoder::new(SampleRate::Hz16000, Channels::Mono, Application::Audio)?;
    encoder.set_bitrate(Bitrate::BitsPerSecond(48_000))?;

    let mut cursor = Cursor::new(Vec::<u8>::with_capacity(samples.len() / 4));
    {
        let mut writer = PacketWriter::new(&mut cursor);
        let serial: u32 = 1;

        writer.write_packet(build_opus_head(), serial, PacketWriteEndInfo::EndPage, 0)?;

        writer.write_packet(build_opus_tags(), serial, PacketWriteEndInfo::EndPage, 0)?;

        let frame_count = samples.len().div_ceil(FRAME_SIZE);
        let mut enc_buf = vec![0u8; 4000];
        let mut granule_pos: u64 = 0;

        let granule_increment: u64 = (FRAME_SIZE as u64) * 48000 / (SAMPLE_RATE as u64);

        for i in 0..frame_count {
            let start = i * FRAME_SIZE;
            let end = (start + FRAME_SIZE).min(samples.len());

            let frame: &[f32] = if end - start == FRAME_SIZE {
                &samples[start..end]
            } else {
                &[]
            };

            let len = if frame.is_empty() {
                let mut padded = vec![0.0f32; FRAME_SIZE];
                padded[..end - start].copy_from_slice(&samples[start..end]);
                encoder.encode_float(&padded, &mut enc_buf)?
            } else {
                encoder.encode_float(frame, &mut enc_buf)?
            };

            granule_pos += granule_increment;

            let info = if i == frame_count - 1 {
                PacketWriteEndInfo::EndStream
            } else {
                PacketWriteEndInfo::NormalPacket
            };

            writer.write_packet(enc_buf[..len].to_vec(), serial, info, granule_pos)?;
        }
    }

    let result = cursor.into_inner();
    debug!(
        "Opus/OGG encoding: {} samples → {} bytes (ratio {:.1}×)",
        samples.len(),
        result.len(),
        (samples.len() * 2) as f64 / result.len() as f64,
    );
    Ok(result)
}

pub fn opus_round_trip(samples: &[f32]) -> Result<Vec<f32>> {
    let mut decoded = decode_opus_ogg_bytes(&encode_opus_ogg_bytes(samples)?)?;
    decoded.resize(samples.len(), 0.0);
    Ok(decoded)
}

fn decode_opus_ogg_bytes(bytes: &[u8]) -> Result<Vec<f32>> {
    let mut reader = PacketReader::new(Cursor::new(bytes));

    let head = reader.read_packet_expected()?;
    ensure!(
        head.data.len() >= 19 && head.data.starts_with(b"OpusHead"),
        "not an Ogg/Opus stream"
    );
    let pre_skip_48k = u16::from_le_bytes([head.data[10], head.data[11]]) as usize;
    reader.read_packet_expected()?;

    let mut decoder = Decoder::new(SampleRate::Hz16000, Channels::Mono)?;

    let mut frame = vec![0.0f32; SAMPLE_RATE * 120 / 1000];
    let mut decoded = Vec::new();
    while let Some(packet) = reader.read_packet()? {
        let n = decoder.decode_float(Some(&packet.data[..]), &mut frame[..], false)?;
        decoded.extend_from_slice(&frame[..n]);
    }

    let pre_skip = (pre_skip_48k * SAMPLE_RATE / 48000).min(decoded.len());
    decoded.drain(..pre_skip);
    Ok(decoded)
}

fn build_opus_head() -> Vec<u8> {
    let mut h = Vec::with_capacity(19);
    h.extend_from_slice(b"OpusHead");
    h.push(1);
    h.push(1);
    h.extend_from_slice(&312u16.to_le_bytes());
    h.extend_from_slice(&16000u32.to_le_bytes());
    h.extend_from_slice(&0i16.to_le_bytes());
    h.push(0);
    h
}

fn build_opus_tags() -> Vec<u8> {
    let vendor = b"Anonen";
    let mut t = Vec::with_capacity(8 + 4 + vendor.len() + 4);
    t.extend_from_slice(b"OpusTags");
    t.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    t.extend_from_slice(vendor);
    t.extend_from_slice(&0u32.to_le_bytes());
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_short_audio() {
        let samples = vec![0.0f32; 16000];
        let result = encode_opus_ogg_bytes(&samples).expect("encoding should succeed");
        assert!(!result.is_empty());
        assert!(result.len() < samples.len() * 2);
    }

    #[test]
    fn encode_non_frame_aligned() {
        let samples = vec![0.1f32; 500];
        let result = encode_opus_ogg_bytes(&samples).expect("encoding should succeed");
        assert!(!result.is_empty());
    }

    #[test]
    fn round_trip_keeps_the_length() {
        for len in [1, 319, 320, 500, 16_000] {
            let samples = vec![0.1f32; len];
            let out = opus_round_trip(&samples).expect("round trip should succeed");
            assert_eq!(out.len(), len);
        }
    }

    #[test]
    fn round_trip_lines_up_with_the_input() {
        let tone = |hz: f32, i: usize| {
            (2.0 * std::f32::consts::PI * hz * i as f32 / SAMPLE_RATE as f32).sin()
        };
        let samples: Vec<f32> = (0..SAMPLE_RATE)
            .map(|i| 0.3 * tone(440.0, i) + 0.2 * tone(1000.0, i))
            .collect();

        let out = opus_round_trip(&samples).expect("round trip should succeed");

        let range = FRAME_SIZE..SAMPLE_RATE - FRAME_SIZE;
        let dot: f32 = range.clone().map(|i| samples[i] * out[i]).sum();
        let input_energy: f32 = range.clone().map(|i| samples[i] * samples[i]).sum();
        let output_energy: f32 = range.map(|i| out[i] * out[i]).sum();
        let correlation = dot / (input_energy.sqrt() * output_energy.sqrt());
        assert!(correlation > 0.9, "correlation {correlation}");
    }
}
