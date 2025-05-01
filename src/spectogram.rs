use crate::fft::fft;
use crate::input::{down_sample_i16, low_pass_filter_i16};
use crate::models::{BANDS, DSP_RATIO, FREQ_BIN_SIZE, HOP_SIZE, MAX_FREQ, Peek, Spectrogram};
use std::f64::consts::PI;

pub fn extract_peaks(
    spectrogram: &Spectrogram,
    audio_duration: f64,
    sample_rate: u32,
) -> Vec<Peek> {
    let target_sr = sample_rate / DSP_RATIO;
    let freq_resolution = target_sr as f64 / FREQ_BIN_SIZE as f64;
    let time_resolution = (HOP_SIZE as f64) / (target_sr as f64);
    let mut peaks = Vec::with_capacity(spectrogram.len() * 2);

    for (time_bin, frequency_bins) in spectrogram.iter().enumerate() {
        let mut band_maxima = Vec::with_capacity(BANDS.len());

        // Find strongest peak in each frequency band
        for band in &BANDS {
            let (max_mag, max_bin) = frequency_bins[band.min..band.max]
                .iter()
                .enumerate()
                .map(|(i, c)| (c.norm(), i))
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
                .unwrap_or((0.0, 0));

            if max_mag > 0.0 {
                band_maxima.push((max_mag, band.min + max_bin));
            }
        }

        // Calculate dynamic threshold (1.5× average of band maxima)
        let avg_mag: f64 =
            band_maxima.iter().map(|(m, _)| m).sum::<f64>() / band_maxima.len().max(1) as f64;
        let threshold = avg_mag * 1.5;

        // Store peaks exceeding threshold
        for (mag, bin) in band_maxima {
            if mag >= threshold {
                let freq_hz = (bin as f64 * freq_resolution).clamp(0.0, 5000.0);
                let time = time_bin as f64 * time_resolution; // Fixed here
                peaks.push(Peek {
                    time, // Use calculated time
                    freq_hz,
                    magnitude: mag,
                });
            }
        }
    }

    // Post-processing
    peaks.sort_unstable_by(|a, b| b.magnitude.partial_cmp(&a.magnitude).unwrap());
    peaks.dedup_by(|a, b| (a.time - b.time).abs() < 0.01 && (a.freq_hz - b.freq_hz).abs() < 50.0);
    peaks.truncate((audio_duration * 15.0) as usize);

    peaks
}
pub fn spectogram(wave_samples: &[i16], sample_rate: u32) -> anyhow::Result<Spectrogram> {
    let smoothed = low_pass_filter_i16(wave_samples, sample_rate as f32, MAX_FREQ);
    let target_sr = sample_rate / DSP_RATIO;
    let ds = down_sample_i16(&smoothed, sample_rate as f32, target_sr as f32)?;

    let num_windows = ds.len() / (FREQ_BIN_SIZE - HOP_SIZE);
    let mut spec: Spectrogram = vec![Vec::new(); num_windows];

    //hamming
    let window: Vec<f64> = (0..FREQ_BIN_SIZE)
        .map(|i| 0.54 - 0.46 * ((2.0 * PI * i as f64) / (FREQ_BIN_SIZE as f64 - 1.0)).cos())
        .collect();

    for i in 0..num_windows {
        let start = i * HOP_SIZE;
        let mut end = start + FREQ_BIN_SIZE;
        if end > ds.len() {
            end = ds.len();
        }

        // Bin füllen (i16 → f64, Rest bleibt 0)
        let mut bin = vec![0.0_f64; FREQ_BIN_SIZE];
        for (k, idx) in (start..end).enumerate() {
            bin[k] = ds[idx] as f64;
        }

        // Hamming anwenden
        for j in 0..FREQ_BIN_SIZE {
            bin[j] *= window[j];
        }
        spec[i] = fft(&bin);
    }

    Ok(spec)
}
