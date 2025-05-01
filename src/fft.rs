use rustfft::{FftPlanner, num_complex::Complex};

pub fn fft(input: &[f64]) -> Vec<Complex<f64>> {
    let mut buffer: Vec<Complex<f64>> = input.iter().map(|&v| Complex::new(v, 0.0)).collect();
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(buffer.len());
    fft.process(&mut buffer);
    buffer
}