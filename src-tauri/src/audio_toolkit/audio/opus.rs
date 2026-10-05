use anyhow::Result;
use audiopus::coder::Encoder;
use audiopus::{Application, Bitrate, Channels, SampleRate};
use log::debug;
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
}
