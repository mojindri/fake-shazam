use rustfft::num_complex::Complex;
use std::f64::consts::PI;
pub fn fft(input: &[f64]) -> Vec<Complex<f64>> {
    // copy real slice into complex array
    let mut data: Vec<Complex<f64>> = input.iter().map(|&v| Complex::new(v, 0.0)).collect();
    recursive_fft(&mut data);
    data
}
fn recursive_fft(buf: &mut [Complex<f64>]) {
    let n = buf.len();
    if n <= 1 {
        return;
    }
    // split even / odd
    let mut even: Vec<Complex<f64>> = buf.iter().step_by(2).cloned().collect();
    let mut odd: Vec<Complex<f64>> = buf.iter().skip(1).step_by(2).cloned().collect();

    recursive_fft(&mut even);
    recursive_fft(&mut odd);

    // combine
    for k in 0..n / 2 {
        let angle = -2.0 * PI * k as f64 / n as f64;
        let twiddle = Complex::from_polar(1.0, angle) * odd[k];
        buf[k] = even[k] + twiddle;
        buf[k + n / 2] = even[k] - twiddle;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use num_complex::Complex;

    const TOL: f64 = 1e-9;
    fn close(a: Complex<f64>, b: Complex<f64>) -> bool {
        (a - b).norm() < TOL
    }

    /// δ[n] → flat spectrum (all 1 + 0i)
    #[test]
    fn impulse_is_flat() {
        let input = vec![1.0, 0.0, 0.0, 0.0];
        let out = fft(&input);
        for &c in &out {
            assert!(close(c, Complex::new(1.0, 0.0)));
        }
    }

    /// Constant signal → only DC bin (= N), others 0
    #[test]
    fn constant_is_dc() {
        let input = vec![1.0; 4];
        let out = fft(&input);
        assert!(close(out[0], Complex::new(4.0, 0.0)));
        for &c in &out[1..] {
            assert!(close(c, Complex::new(0.0, 0.0)));
        }
    }

    /// Real input: X[k] should equal conj(X[N-k])
    #[test]
    fn conjugate_symmetry() {
        let input = vec![0.0, 1.0, 0.0, -1.0];
        let out = fft(&input);
        assert!(close(out[1], out[3].conj()));
    }
}
