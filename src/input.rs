use anyhow::{Result, bail};
use std::fs::File;
use std::path::PathBuf;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::audio::SignalSpec;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::{get_codecs, get_probe};
use tracing::info;

pub fn load_mp3(path: &PathBuf) -> Result<(Vec<i16>, Option<u32>)> {
    // 1. Open the file and wrap it
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    // 2. Probe the format
    let probed = get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;

    // 3. Pick the default track and create a decoder
    let track = format
        .default_track()
        .ok_or_else(|| anyhow::anyhow!("No default track found"))?;
    let mut decoder = get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    // 4. Prepare a sample buffer and output Vec
    let mut sample_buf: Option<SampleBuffer<i16>> = None;
    let mut out: Vec<i16> = Vec::new();
    let mut mp3_sample_rate = None;
    // 5. Loop over packets, decode, and copy interleaved samples
    loop {
        let packet = match format.next_packet() {
            Ok(pkt) => pkt,
            Err(_) => break, // EOF
        };
        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                // On first packet, init SampleBuffer with the right spec & capacity
                if sample_buf.is_none() {
                    let spec: SignalSpec = *audio_buf.spec();
                    let sample_rate = spec.rate; // u32
                    let channels = spec.channels.count(); // usize
                    let cap = audio_buf.capacity() as u64;
                    mp3_sample_rate = Some(sample_rate);
                    tracing::info!(
                        "Sample rate (from SignalSpec): {} Hz, Channels: {}",
                        sample_rate,
                        channels
                    );

                    sample_buf = Some(SampleBuffer::<i16>::new(cap, spec));
                }
                let buf = sample_buf.as_mut().unwrap();
                buf.copy_interleaved_ref(audio_buf);
                out.extend_from_slice(buf.samples());
            }
            Err(e) => {
                info!("Decoding error, skipping packet: {:?}", e);
                continue;
            }
        }
    }
    let sample_count = out.len();
    let decoded_bytes = sample_count * size_of::<i16>();
    info!(
        "Decoded PCM: {} samples  (≈{} bytes)",
        sample_count, decoded_bytes
    );

    Ok((out, mp3_sample_rate))
}

pub fn low_pass_filter_i16(samples: &[i16], sample_rate: f32, cutoff_hz: f32) -> Vec<i16> {
    let dt = 1.0 / sample_rate;
    let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff_hz);
    let alpha = dt / (rc + dt);
    let mut prev: f32 = 0.0;

    samples
        .iter()
        .map(|&s| {
            // 1. to f32 in [-1.0,1.0]
            let x = s as f32 / i16::MAX as f32;
            // 2. filter: y[n] = y[n-1] + α*(x[n] - y[n-1])
            prev += alpha * (x - prev);
            // 3. back to i16
            (prev * i16::MAX as f32) as i16
        })
        .collect()
}

pub fn down_sample_i16(samples: &[i16], original_rate: f32, target_rate: f32) -> Result<Vec<i16>> {
    if original_rate <= 0.0 || target_rate <= 0.0 {
        bail!("sample rates must be positive");
    }
    if target_rate > original_rate {
        bail!(
            "target rate ({}) > original ({})",
            target_rate,
            original_rate
        );
    }

    // Integer factor (rounded to nearest whole frame)
    let factor = (original_rate / target_rate).round() as usize;
    if factor < 1 {
        bail!("invalid factor {}", factor);
    }

    // Average each block of factor samples
    let mut out = Vec::with_capacity(samples.len().div_ceil(factor));
    for chunk in samples.chunks(factor) {
        let sum: i32 = chunk.iter().map(|&s| s as i32).sum();
        out.push((sum / chunk.len() as i32) as i16);
    }
    Ok(out)
}
