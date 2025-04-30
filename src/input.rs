use crate::fft::fft;
use crate::models::Couple;
use anyhow::{Result, bail};
use num_complex::ComplexFloat;
use rustfft::num_complex::Complex;
use std::cmp::min;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::fs::File;
use std::path::PathBuf;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::audio::SignalSpec;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::{get_codecs, get_probe};
use tracing::info;

const DSP_RATIO: u32 = 4; // use u32 if sample_rate is u32
const FREQ_BIN_SIZE: usize = 1024; // usize is handy for FFT buffer sizes
const MAX_FREQ: f32 = 5_000.0; // make this f32 so no cast later
const HOP_SIZE: usize = FREQ_BIN_SIZE / 32;

pub fn load_mp3(path: &PathBuf) -> Result<(Vec<i16>, Option<u32>)> {
    // 1. Open the file and wrap it
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // 2. Probe the format
    let probed = get_probe().format(
        &Default::default(),
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
                // Copy into our interleaved buffer
                buf.copy_interleaved_ref(audio_buf);
                //info!("Packet decoded: {} samples", buf.samples().len());
                // Append to output
                let preview: Vec<i16> = buf.samples().iter().take(10).cloned().collect();
                //debug!("Sample preview: {:?}", preview);

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
type Spectrogram = Vec<Vec<Complex<f64>>>;

#[derive(Clone, Copy)]
pub struct Maxies {
    pub(crate) max_msg: f64,
    pub(crate) max_freq: Complex<f64>,
    pub(crate) freq_ids: i16,
}
pub struct Band {
    pub(crate) min: i16,
    pub(crate) max: i16,
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

#[cfg(test)]
mod tests {
    use super::{down_sample_i16, low_pass_filter_i16};
    use approx::assert_relative_eq;
    use std::i16;

    /// 1) If you feed in all zeros, you should get zeros back.
    #[test]
    fn zeros_input_gives_zeros() {
        let samples = vec![0i16; 8];
        let filtered = low_pass_filter_i16(&samples, 48_000.0, 1_000.0);
        assert!(
            filtered.iter().all(|&s| s == 0),
            "Expected all zeros, got {:?}",
            filtered
        );
    }

    /// 2) A “step” input with alpha = 0.5 (cutoff = 1/(2π) Hz at fs = 1 Hz)
    ///    produces the classic 0.5, 0.25, 0.125… exponential decay.
    #[test]
    fn step_response_alpha_half() {
        // Choose sample_rate=1 Hz, cutoff=1/(2π) → alpha = 0.5 exactly
        let fs = 1.0;
        let cutoff = 1.0 / (2.0 * std::f32::consts::PI);
        // Input: [1.0, 0, 0, 0] in i16 form:
        let input = vec![i16::MAX, 0, 0, 0];
        let filtered = low_pass_filter_i16(&input, fs, cutoff);

        // Expected y[n]: 0.5, 0.25, 0.125, 0.0625  → times i16::MAX
        let exp = vec![
            (0.5 * i16::MAX as f32) as i16,    // ≈16383
            (0.25 * i16::MAX as f32) as i16,   // ≈ 8191
            (0.125 * i16::MAX as f32) as i16,  // ≈ 4095
            (0.0625 * i16::MAX as f32) as i16, // ≈ 2047
        ];

        assert_eq!(
            filtered, exp,
            "Step response did not match: got {:?}, want {:?}",
            filtered, exp
        );
    }

    /// 3) A constant “all max” input with alpha=0.5 should ramp up:
    ///     y[0]=0.5, y[1]=0.75, y[2]=0.875, y[3]=0.9375
    #[test]
    fn constant_input_ramps_up() {
        let fs = 1.0;
        let cutoff = 1.0 / (2.0 * std::f32::consts::PI);
        let input = vec![i16::MAX; 4]; // [1, 1, 1, 1] in [-1,1]
        let filtered = low_pass_filter_i16(&input, fs, cutoff);

        // Build expected by hand:
        let mut exp = Vec::new();
        let mut y = 0.0;
        let alpha = 0.5;
        for _ in 0..4 {
            y += alpha * (1.0 - y);
            exp.push((y * i16::MAX as f32) as i16);
        }

        assert_eq!(
            filtered, exp,
            "Constant input ramp did not match: got {:?}, want {:?}",
            filtered, exp
        );
    }

    macro_rules! ok {
        ($expr:expr) => {
            match $expr {
                Ok(v) => v,
                Err(e) => panic!("unexpected Err: {e}"),
            }
        };
    }
    /* ------------------------------------------------------------------ */
    /* 1) Factor 1  =>  output identical to input                         */
    /* ------------------------------------------------------------------ */
    #[test]
    fn factor_one_is_identity() {
        let input: Vec<i16> = (-3..=3).collect(); // [-3,-2,-1,0,1,2,3]
        let out = ok!(down_sample_i16(&input, 48_000.0, 48_000.0));
        assert_eq!(out, input, "factor-1 should return the same data");
    }
    /* ------------------------------------------------------------------ */
    /* 2) Factor 2  =>  average every two samples                         */
    /*    input 0,1,2,3,4,5,6,7  →  (0+1)/2, (2+3)/2, … = 0,2,4,6         */
    /* ------------------------------------------------------------------ */
    #[test]
    fn factor_two_averages_pairs() {
        let input: Vec<i16> = (0..8).collect(); // 0..7
        let out = ok!(down_sample_i16(&input, 48_000.0, 24_000.0));
        let want = vec![0, 2, 4, 6];
        assert_eq!(out, want);
    }
    /* ------------------------------------------------------------------ */
    /* 3) Handles tail shorter than factor (len 3, factor 2)              */
    /*    input 1,3,5  →  chunk[0..2]=(1+3)/2=2,  chunk[2..3]=5           */
    /* ------------------------------------------------------------------ */
    #[test]
    fn handles_non_multiple_length() {
        let input = vec![1i16, 3, 5];
        let out = ok!(down_sample_i16(&input, 44_100.0, 22_050.0)); // factor ≈2
        assert_eq!(out, vec![2, 5]);
    }

    /* ------------------------------------------------------------------ */
    /* 4) Error when target_rate > original_rate                          */
    /* ------------------------------------------------------------------ */
    #[test]
    fn error_on_upsample() {
        let input = vec![0i16; 4];
        let res: anyhow::Result<Vec<i16>> = down_sample_i16(&input, 22_050.0, 44_100.0);
        assert!(res.is_err(), "Should refuse up-sampling");
    }

    use super::*;
    use num_complex::Complex;

    const SR: u32 = 8_000; // small, easy-math sample-rate
    const TOL: f64 = 1e-9;

    fn is_near(a: Complex<f64>, b: Complex<f64>) -> bool {
        (a - b).norm() < TOL
    }

    #[test]
    fn impulse_is_flat() {
        // Use a very high cutoff frequency to bypass the low-pass filter
        const MAX_FREQ_HIGH: f32 = 1_000_000.0; // Higher than Nyquist frequency
        const MIN_RAW_SAMPLES: usize = FREQ_BIN_SIZE * DSP_RATIO as usize;

        let mut samples = vec![0i16; MIN_RAW_SAMPLES];
        samples[0] = i16::MAX;

        // Bypass the low-pass filter or set a very high cutoff
        let smoothed = low_pass_filter_i16(&samples, SR as f32, MAX_FREQ_HIGH);
        // Ensure downsampling factor is 1 to avoid altering the signal
        let target_sr = SR; // No downsampling
        let ds = down_sample_i16(&smoothed, SR as f32, target_sr as f32).unwrap();

        // Compute FFT on the original impulse (now filtered as pass-through)
        let mut bin = vec![0.0; FREQ_BIN_SIZE];
        bin[0] = ds[0] as f64;
        // Apply Hamming window (only affects first element here)
        let window: Vec<f64> = (0..FREQ_BIN_SIZE)
            .map(|i| 0.54 - 0.46 * (2.0 * PI * i as f64 / (FREQ_BIN_SIZE as f64 - 1.0)).cos())
            .collect();
        bin[0] *= window[0];

        let fft_result = fft(&bin);
        let ref_val = fft_result[0];
        for &c in &fft_result {
            assert!((c - ref_val).norm() < 1e-9, "FFT of impulse is not flat");
        }
    }

    #[test]
    fn downsampling_averages_correctly() -> anyhow::Result<()> {
        // Input: 8 samples, downsampling factor 2
        let input = vec![1000i16, 2000, 3000, 4000, 5000, 6000, 7000, 8000];
        let original_rate = 8000;
        let target_rate = 4000;

        let downsampled = down_sample_i16(&input, original_rate as f32, target_rate as f32)?;

        assert_eq!(downsampled.len(), 4, "Downsampled length mismatch");
        assert_eq!(
            downsampled[0],
            (1000 + 2000) / 2,
            "First chunk average wrong"
        );
        assert_eq!(
            downsampled[1],
            (3000 + 4000) / 2,
            "Second chunk average wrong"
        );
        assert_eq!(
            downsampled[2],
            (5000 + 6000) / 2,
            "Third chunk average wrong"
        );
        assert_eq!(
            downsampled[3],
            (7000 + 8000) / 2,
            "Fourth chunk average wrong"
        );

        Ok(())
    }
    #[test]
    fn low_pass_filter_attenuates_high_frequencies() {
        // Create high-frequency square wave (alternating max/min values)
        let square_wave = vec![i16::MAX, i16::MIN, i16::MAX, i16::MIN, i16::MAX, i16::MIN];
        let cutoff = 100.0; // Low cutoff frequency
        let sample_rate = 44100;

        let filtered = low_pass_filter_i16(&square_wave, sample_rate as f32, cutoff);

        // After filtering, peaks should be attenuated and values should trend toward zero
        assert!(
            filtered[0].abs() > filtered[2].abs(),
            "No high-frequency attenuation"
        );
        assert!(
            filtered[2].abs() > filtered[4].abs(),
            "Filter settling not visible"
        );
        assert!(
            (*filtered.last().unwrap() as f32).abs()     // i16 → f32
                < (i16::MAX as f32) * 0.1, // i16 → f32 before × 0.1
            "Final filtered value not sufficiently attenuated"
        );
    }
    #[test]
    fn window_function_application_correct() {
        const TEST_SIZE: usize = 8;
        let mut samples = vec![1.0; TEST_SIZE]; // Constant input
        let window: Vec<f64> = (0..TEST_SIZE)
            .map(|i| 0.54 - 0.46 * (2.0 * PI * i as f64 / (TEST_SIZE as f64 - 1.0)).cos())
            .collect();

        // Apply window
        for j in 0..TEST_SIZE {
            samples[j] *= window[j];
        }

        // Verify window application
        for (original, windowed) in window.iter().zip(samples) {
            assert!(
                (windowed - original).abs() < 1e-9,
                "Window application failed: {} vs {}",
                windowed,
                original
            );
        }
    }
}
