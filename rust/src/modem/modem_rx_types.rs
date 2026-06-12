use num_complex::Complex;
use std::time::SystemTime;

pub type RawComplexFrame = Vec<Complex<f32>>;
pub type SpectrumFrame = Vec<Complex<f32>>;
pub type TimeFrequencyImage = Vec<SpectrumFrame>;
pub type SymbolStream = Vec<u8>;
pub type FrameBytes = Vec<u8>;

#[derive(Debug, Clone)]
pub struct RxMessage {
    pub source_mac: String,
    pub payload: String,
    pub received_at: SystemTime,
}
