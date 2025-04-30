// spectogram.rs

use crate::fft::fft;
use crate::input::{Maxies, down_sample_i16, low_pass_filter_i16};
use anyhow::Result;
use num_complex::Complex;
use std::f64::consts::PI;

const DSP_RATIO: u32 = 4;
const MAX_FREQ: f32 = 5_000.0;
const FREQ_BIN_SIZE: usize = 1024;
const HOP_SIZE: usize = FREQ_BIN_SIZE / 32;

pub type Spectrogram = Vec<Vec<Complex<f64>>>;

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

        // FFT und speichern
        spec[i] = fft(&bin);
    }

    Ok(spec)
}

#[derive(Clone, Copy)]
struct Band {
    min: usize,
    max: usize,
}
pub struct Peek {
    pub time: f64,
    pub freq: Complex<f64>,
}

/// Extract peaks above average magnitude per band
pub fn extract_peaks(spectrogram: &Spectrogram, audio_duration: f64) -> Vec<Peek> {
    if spectrogram.is_empty() {
        return vec![];
    }
    let bands = [
        crate::input::Band { min: 0, max: 10 },
        crate::input::Band { min: 10, max: 20 },
        crate::input::Band { min: 20, max: 40 },
        crate::input::Band { min: 40, max: 80 },
        crate::input::Band { min: 80, max: 160 },
        crate::input::Band { min: 160, max: 512 },
    ];
    let mut peaks = Vec::new();
    let bin_duration = audio_duration / spectrogram.len() as f64;
    for (bin_idx, bin) in spectrogram.iter().enumerate() {
        let mut bin_band_maxes = Vec::new();
        for band in &bands {
            let mut max_mag = 0.0;
            let mut max_freq = Complex::default();
            let mut freq_idx = 0;
            for (index, freq) in bin[band.min as usize..band.max as usize].iter().enumerate() {
                let magnitude = freq.norm();
                if magnitude > max_mag {
                    max_mag = magnitude;
                    max_freq = *freq;
                    freq_idx = band.min + index as i16;
                }
            }
            bin_band_maxes.push(Maxies {
                max_msg: max_mag,
                max_freq,
                freq_ids: freq_idx,
            });
        }
        if bin_band_maxes.is_empty() {
            continue;
        }
        // Calculate the average magnitude of the maxima
        let total_mag: f64 = bin_band_maxes.iter().map(|m| m.max_msg).sum();
        let avg = total_mag / bin_band_maxes.len() as f64;
        for max in &bin_band_maxes {
            if max.max_msg > avg {
                let peak_time_in_bin = (max.freq_ids as f64) * bin_duration / bin.len() as f64;
                let peak_time = bin_idx as f64 * bin_duration + peak_time_in_bin;
                peaks.push(Peek {
                    time: peak_time,
                    freq: max.max_freq,
                });
            }
        }
    }
    peaks
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use approx::assert_relative_eq;
    use num_complex::Complex;
    use std::f64::consts::PI;

    #[test]
    fn silence_is_zero() {
        let samples = vec![0i16; FREQ_BIN_SIZE * 2];
        let spec = spectogram(&samples, 8000).unwrap();
        for frame in spec {
            for &b in &frame {
                assert!(b.norm() < 1e-9);
            }
        }
    }

    #[test]
    fn constant_is_dc() {
        let samples = vec![32767i16; FREQ_BIN_SIZE * 2];
        let spec = spectogram(&samples, 8000).unwrap();
        for frame in spec {
            assert!(frame[0].re > 0.99 * (FREQ_BIN_SIZE as f64));
            for &c in &frame[1..] {
                assert!(c.norm() < 1e-6);
            }
        }
    }

    #[test]
    fn all_zero_input_produces_zero_spectrum() -> Result<()> {
        let spec = spectogram(&vec![0i16; 4096], 44100)?;
        for row in spec {
            for bin in row {
                assert!(bin.norm() < 1e-9);
            }
        }
        Ok(())
    }

    #[test]
    fn pure_sine_wave_peaks_at_correct_bin() -> Result<()> {
        const SR: u32 = 44100;
        const F: f32 = 1000.0;
        const D: f32 = 0.1;
        let ns = (SR as f32 * D) as usize;
        let mut samples = Vec::with_capacity(ns);
        for i in 0..ns {
            let t = i as f64 / SR as f64;
            samples.push((i16::MAX as f64 * (2.0 * PI * F as f64 * t).sin()) as i16);
        }
        let spec = spectogram(&samples, SR)?;
        let expected = (F * FREQ_BIN_SIZE as f32 / (SR as f32 / DSP_RATIO as f32)) as usize;
        let first = &spec[0];
        let (peak, _) = first
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.norm().partial_cmp(&b.1.norm()).unwrap())
            .unwrap();
        assert!(peak.abs_diff(expected) <= 1);
        Ok(())
    }

    #[test]
    fn empty_spectrogram_returns_empty() {
        let peaks = extract_peaks(&vec![], 1.0);
        assert!(peaks.is_empty());
    }

    #[test]
    fn single_bin_with_clear_peak() {
        let mut bin = vec![Complex::default(); 512];
        bin[5] = Complex::new(10.0, 0.0);
        let peaks = extract_peaks(&vec![bin], 1.0);
        assert_eq!(peaks.len(), 1);
        assert_relative_eq!(peaks[0].freq.norm(), 10.0);
        assert_relative_eq!(peaks[0].time, 5.0 / 512.0);
    }

    #[test]
    fn peaks_above_average_are_selected() {
        let mut bin = vec![Complex::default(); 512];
        bin[5] = Complex::new(10.0, 0.0);
        bin[15] = Complex::new(2.0, 0.0);
        let peaks = extract_peaks(&vec![bin], 1.0);
        assert_eq!(peaks.len(), 1);
        assert_relative_eq!(peaks[0].freq.norm(), 10.0);
    }

    #[test]
    fn multiple_peaks_in_different_bands() {
        let mut bin = vec![Complex::default(); 512];
        bin[5] = Complex::new(15.0, PI / 4.0);
        bin[300] = Complex::new(20.0, PI / 2.0);
        let peaks = extract_peaks(&vec![bin], 1.0);
        assert_eq!(peaks.len(), 2);
        assert_relative_eq!(peaks[0].freq.norm(), 15.020547602370165);
        assert_relative_eq!(peaks[1].freq.norm(), 20.061590193707787);
    }
}
