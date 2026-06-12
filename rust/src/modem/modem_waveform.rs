use num_complex::Complex;
use rustfft::{Fft, FftPlanner, num_complex::Complex as RustComplex};
use std::sync::Arc;

pub struct WaveformGenerator {
    pub ifft_size: usize,
    pub valid_bins_low: (usize, usize),
    pub valid_bins_high: (usize, usize),
    fft: Arc<dyn Fft<f32>>,
}

impl WaveformGenerator {
    pub fn new(ifft_size: usize, low_block: (usize, usize), high_block: (usize, usize)) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_inverse(ifft_size);

        WaveformGenerator {
            ifft_size,
            valid_bins_low: low_block,
            valid_bins_high: high_block,
            fft,
        }
    }

    pub fn generate_symbol(&self, symbol_value: u8) -> Vec<Complex<f32>> {
        let mut freq_buffer = vec![RustComplex::new(0.0, 0.0); self.ifft_size];
        let bin_index = self.symbol_to_bin(symbol_value);

        if bin_index < self.ifft_size {
            freq_buffer[bin_index] = RustComplex::new(1.0, 0.0);
        }

        self.fft.process(&mut freq_buffer);

        let scale = 1.0 / self.ifft_size as f32;
        freq_buffer
            .into_iter()
            .map(|c| Complex::new(c.re * scale, c.im * scale))
            .collect()
    }

    fn symbol_to_bin(&self, symbol_value: u8) -> usize {
        let symbol = symbol_value as usize;

        if symbol <= 127 {
            self.valid_bins_low.0 + symbol
        } else {
            self.valid_bins_high.0 + (symbol - 128)
        }
    }

    pub fn generate_waveform(&self, symbols: &[u8]) -> Vec<Complex<f32>> {
        let mut waveform = Vec::with_capacity(symbols.len() * self.ifft_size);
        for symbol in symbols {
            waveform.extend(self.generate_symbol(*symbol));
        }
        waveform
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_symbol_length_is_ifft_size() {
        let generator = WaveformGenerator::new(512, (8, 135), (376, 503));
        let symbol_wave = generator.generate_symbol(0);
        assert_eq!(symbol_wave.len(), 512);
    }

    #[test]
    fn generate_waveform_concat_symbols() {
        let generator = WaveformGenerator::new(512, (8, 135), (376, 503));
        let waveform = generator.generate_waveform(&[0, 128, 255]);
        assert_eq!(waveform.len(), 512 * 3);
    }
}
