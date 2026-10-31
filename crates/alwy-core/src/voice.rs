//! Voice messages, Element-compatible (MSC3245 v1 + MSC1767 audio block: duration and waveform).
//!
//! Webviews record Opus in WebM (Chromium) while Matrix voice messages are Ogg/Opus, which is
//! what Element and FluffyChat (including iOS, which converts it) expect. Both containers hold
//! the same Opus packets, so we re-wrap them: no re-encoding, no codec library.

use std::time::Duration;

use matrix_sdk::attachment::{AttachmentInfo, BaseAudioInfo};
use matrix_sdk_ui::timeline::{AttachmentConfig, AttachmentSource};

use crate::{Error, Result};

/// The waveform sent with a voice message is resampled to this many bars (what Element uses).
const WAVEFORM_BARS: usize = 100;

impl crate::Core {
    /// Send a recorded voice message. `waveform` holds loudness samples between 0 and 1.
    pub async fn send_voice(
        &self,
        room_id: &str,
        bytes: Vec<u8>,
        mime: &str,
        duration_ms: u64,
        waveform: Vec<f32>,
    ) -> Result<()> {
        let timeline = self.timeline(room_id)?;
        let (bytes, mime) = if mime.starts_with("audio/webm") {
            (webm_to_ogg(&bytes)?, "audio/ogg")
        } else {
            // Already Ogg (Firefox) or something else we pass through (e.g. MP4 from WebKit).
            (bytes, mime.split(';').next().unwrap_or(mime))
        };
        let extension = match mime {
            "audio/ogg" => "ogg",
            "audio/mp4" => "m4a",
            _ => "audio",
        };
        let config = AttachmentConfig {
            info: Some(AttachmentInfo::Voice(BaseAudioInfo {
                duration: Some(Duration::from_millis(duration_ms)),
                size: u32::try_from(bytes.len()).ok().map(Into::into),
                waveform: Some(resample(&waveform, WAVEFORM_BARS)),
            })),
            ..Default::default()
        };
        let mime: mime::Mime = mime
            .parse()
            .map_err(|_| Error::Upload("bad audio type".into()))?;
        timeline
            .send_attachment(
                AttachmentSource::Data {
                    bytes,
                    filename: format!("Voice message.{extension}"),
                },
                mime,
                config,
            )
            .use_send_queue()
            .await
            .map_err(|e| Error::Upload(e.to_string()))
    }
}

/// Average `samples` into `bars` buckets, scaled so the loudest bar is 1.
fn resample(samples: &[f32], bars: usize) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    let out: Vec<f32> = (0..bars.min(samples.len()))
        .map(|i| {
            let n = bars.min(samples.len());
            let chunk = &samples[i * samples.len() / n..(i + 1) * samples.len() / n];
            chunk.iter().copied().sum::<f32>() / chunk.len().max(1) as f32
        })
        .collect();
    let max = out.iter().copied().fold(0.0_f32, f32::max);
    if max <= 0.0 {
        return out;
    }
    out.iter().map(|v| (v / max).clamp(0.0, 1.0)).collect()
}

// ── WebM → Ogg ───────────────────────────────────────────────────────────────

const SEGMENT: u32 = 0x1853_8067;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const CODEC_ID: u32 = 0x86;
const CODEC_PRIVATE: u32 = 0x63A2;
const CLUSTER: u32 = 0x1F43_B675;
const BLOCK_GROUP: u32 = 0xA0;
const BLOCK: u32 = 0xA1;
const SIMPLE_BLOCK: u32 = 0xA3;

/// Re-wrap the Opus track of a WebM recording as an Ogg/Opus file.
pub fn webm_to_ogg(webm: &[u8]) -> Result<Vec<u8>> {
    let bad = |what: &str| Error::Upload(format!("couldn't read the recording ({what})"));
    let mut head = None;
    let mut is_opus = false;
    let mut packets = Vec::new();

    // Flat walk: we step *into* the few container elements we care about instead of recursing,
    // so the "unknown size" segments and clusters that live recorders write need no special case.
    let mut pos = 0;
    while pos < webm.len() {
        let (id, id_len) = read_id(&webm[pos..]).ok_or_else(|| bad("element id"))?;
        let (size, size_len) =
            read_size(&webm[pos + id_len..]).ok_or_else(|| bad("element size"))?;
        let body = pos + id_len + size_len;
        match id {
            SEGMENT | TRACKS | TRACK_ENTRY | CLUSTER | BLOCK_GROUP => {
                pos = body;
                continue;
            }
            _ => {}
        }
        let size = size.ok_or_else(|| bad("unknown size"))?;
        let end = body
            .checked_add(size)
            .filter(|&e| e <= webm.len())
            .ok_or_else(|| bad("truncated"))?;
        let data = &webm[body..end];
        match id {
            CODEC_ID => is_opus = data == b"A_OPUS",
            CODEC_PRIVATE => head = Some(data.to_vec()),
            SIMPLE_BLOCK | BLOCK => {
                // Track number (vint), 16-bit timecode, flags; recorders don't use lacing.
                let (_, track_len) = read_size(data).ok_or_else(|| bad("block"))?;
                let frame = data.get(track_len + 3..).ok_or_else(|| bad("block"))?;
                if !frame.is_empty() {
                    packets.push(frame);
                }
            }
            _ => {}
        }
        pos = end;
    }

    if !is_opus {
        return Err(bad("not Opus"));
    }
    let head = head
        .filter(|h| h.starts_with(b"OpusHead"))
        .ok_or_else(|| bad("no Opus header"))?;
    write_ogg(&head, &packets)
}

fn write_ogg(head: &[u8], packets: &[&[u8]]) -> Result<Vec<u8>> {
    use ogg::writing::{PacketWriteEndInfo, PacketWriter};

    let serial = 0x616c_7779; // "alwy"
    let mut out = Vec::new();
    {
        let mut writer = PacketWriter::new(&mut out);
        let io = |e: std::io::Error| Error::Upload(e.to_string());
        writer
            .write_packet(head.to_vec(), serial, PacketWriteEndInfo::EndPage, 0)
            .map_err(io)?;
        let mut tags = b"OpusTags".to_vec();
        let vendor = b"AlwyChat";
        tags.extend((vendor.len() as u32).to_le_bytes());
        tags.extend(vendor);
        tags.extend(0u32.to_le_bytes());
        writer
            .write_packet(tags, serial, PacketWriteEndInfo::EndPage, 0)
            .map_err(io)?;

        // Granule position = 48 kHz samples decoded so far (pre-skip included, per RFC 7845).
        let mut granule = 0u64;
        for (i, packet) in packets.iter().enumerate() {
            granule += opus_samples(packet);
            let end = if i + 1 == packets.len() {
                PacketWriteEndInfo::EndStream
            } else {
                PacketWriteEndInfo::NormalPacket
            };
            writer
                .write_packet(packet.to_vec(), serial, end, granule)
                .map_err(io)?;
        }
    }
    Ok(out)
}

/// Samples (at 48 kHz) in an Opus packet, from its TOC byte (RFC 6716 §3.1).
fn opus_samples(packet: &[u8]) -> u64 {
    let Some(&toc) = packet.first() else { return 0 };
    let config = toc >> 3;
    // Frame length in 1/400 s (2.5 ms) units.
    let frame_units: u64 = match config {
        0..=11 => [4, 8, 16, 24][(config % 4) as usize], // SILK: 10, 20, 40, 60 ms
        12..=15 => [4, 8][(config % 2) as usize],        // Hybrid: 10, 20 ms
        _ => [1, 2, 4, 8][(config % 4) as usize],        // CELT: 2.5, 5, 10, 20 ms
    };
    let frames = match toc & 0b11 {
        0 => 1,
        1 | 2 => 2,
        _ => packet.get(1).map_or(0, |b| u64::from(b & 0x3F)),
    };
    frames * frame_units * 120
}

/// EBML element ID (marker bits kept, as IDs are written in specs).
fn read_id(bytes: &[u8]) -> Option<(u32, usize)> {
    let first = *bytes.first()?;
    let len = first.leading_zeros() as usize + 1;
    if len > 4 || bytes.len() < len {
        return None;
    }
    let id = bytes[..len]
        .iter()
        .fold(0u32, |acc, &b| (acc << 8) | u32::from(b));
    Some((id, len))
}

/// EBML data size; `None` inside means "unknown" (all value bits set).
fn read_size(bytes: &[u8]) -> Option<(Option<usize>, usize)> {
    let first = *bytes.first()?;
    let len = first.leading_zeros() as usize + 1;
    if len > 8 || bytes.len() < len {
        return None;
    }
    let mask = if len == 8 { 0 } else { 0xFFu8 >> len };
    let mut value = u64::from(first & mask);
    for &b in &bytes[1..len] {
        value = (value << 8) | u64::from(b);
    }
    let unknown = value == (1u64 << (7 * len)) - 1;
    Some(((!unknown).then_some(value as usize), len))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(id: &[u8], data: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.push(0x80 | data.len() as u8); // 1-byte size (tests stay < 127 bytes)
        out.extend(data);
        out
    }

    /// A WebM shaped like Chromium's MediaRecorder output: unknown-size segment and cluster.
    fn recording(packets: &[&[u8]]) -> (Vec<u8>, Vec<u8>) {
        let mut head = b"OpusHead".to_vec();
        head.extend([1, 1, 0x38, 0x01, 0x80, 0xBB, 0, 0, 0, 0, 0]);
        let entry = [
            element(&[0xD7], &[1]),
            element(&[0x86], b"A_OPUS"),
            element(&[0x63, 0xA2], &head),
        ]
        .concat();
        let tracks = element(&[0x16, 0x54, 0xAE, 0x6B], &element(&[0xAE], &entry));
        let mut cluster = vec![
            0x1F, 0x43, 0xB6, 0x75, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ];
        cluster.extend(element(&[0xE7], &[0]));
        for p in packets {
            cluster.extend(element(
                &[0xA3],
                &[[0x81, 0, 0, 0x80].as_slice(), p].concat(),
            ));
        }
        let mut webm = element(&[0x1A, 0x45, 0xDF, 0xA3], &element(&[0x42, 0x82], b"webm"));
        webm.extend([
            0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ]);
        webm.extend(tracks);
        webm.extend(cluster);
        (webm, head)
    }

    #[test]
    fn rewraps_opus_packets_into_ogg() {
        // TOC 0xFC: CELT 20 ms, one frame = 960 samples.
        let packets: [&[u8]; 3] = [&[0xFC, 1, 2], &[0xFC, 3], &[0xFC, 4, 5, 6]];
        let (webm, head) = recording(&packets);
        let ogg = webm_to_ogg(&webm).unwrap();

        let mut reader = ogg::reading::PacketReader::new(std::io::Cursor::new(ogg));
        assert_eq!(reader.read_packet().unwrap().unwrap().data, head);
        assert!(
            reader
                .read_packet()
                .unwrap()
                .unwrap()
                .data
                .starts_with(b"OpusTags")
        );
        let mut last = None;
        for expected in packets {
            let p = reader.read_packet().unwrap().unwrap();
            assert_eq!(p.data, expected);
            last = Some(p);
        }
        let last = last.unwrap();
        assert!(last.last_in_stream());
        assert_eq!(last.absgp_page(), 3 * 960);
    }

    #[test]
    fn opus_packet_durations() {
        assert_eq!(opus_samples(&[0xFC]), 960); // CELT 20 ms
        assert_eq!(opus_samples(&[0x08]), 960); // SILK 20 ms
        assert_eq!(opus_samples(&[0xFD]), 1920); // two 20 ms frames
        assert_eq!(opus_samples(&[0xFF, 0x03]), 2880); // code 3: three frames
    }

    #[test]
    fn rejects_other_codecs() {
        let (mut webm, _) = recording(&[&[0xFC]]);
        let at = webm.windows(6).position(|w| w == b"A_OPUS").unwrap();
        webm[at..at + 6].copy_from_slice(b"A_VORB");
        assert!(webm_to_ogg(&webm).is_err());
    }

    #[test]
    fn waveform_is_resampled_and_normalised() {
        let w = resample(&[0.0, 0.2, 0.4, 0.4], 2);
        assert_eq!(w, vec![0.25, 1.0]);
    }
}
