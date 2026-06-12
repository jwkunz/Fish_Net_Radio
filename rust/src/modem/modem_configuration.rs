use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct ModemConfiguration {
    pub name: String,
    pub description: String,
    pub gnuradio_instance_address_tx: String,
    pub gnuradio_instance_port_tx: String,
    #[serde(alias = "sample_rate")]
    pub sample_rate_hz: f64,
    pub bits_per_symbol: usize,
    pub symbol_duration_samples: usize,
    pub raw_data_rate_bps: f64,
    pub spectral_efficiency: f64,
    pub transmitter: TransmitterConfig,
    pub framing: FramingConfig,
    pub receiver: ReceiverConfig,
    pub payload: PayloadConfig,
    pub output: OutputConfig,
    #[serde(default)]
    pub notes: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TransmitterConfig {
    pub ifft_size: usize,
    pub symbol_samples: usize,
    pub output_format: String,
    pub valid_bins: ValidBinBlocks,
    pub bin_mapping: BinMapping,
    pub preamble: PreambleConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ValidBinBlocks {
    pub low_block: BinBlock,
    pub high_block: BinBlock,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BinBlock {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BinMapping {
    pub scheme: String,
    pub low_symbol_range: SymbolRange,
    pub high_symbol_range: SymbolRange,
    pub description: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SymbolRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PreambleConfig {
    pub bytes: Vec<String>,
    pub length_bytes: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FramingConfig {
    pub mac: MacConfig,
    pub crc: CrcConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MacConfig {
    pub destination_length_bytes: usize,
    pub source_length_bytes: usize,
    pub allow_broadcast: bool,
    pub destination_mac: String,
    pub source_mac: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CrcConfig {
    pub r#type: String,
    pub polynomial: String,
    pub init: String,
    pub xor_out: String,
    pub reflect_in: bool,
    pub reflect_out: bool,
    pub covers: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReceiverConfig {
    pub fft_size: usize,
    pub fft_overlap_samples: usize,
    pub symbol_rows: usize,
    pub preamble_rows: usize,
    pub search_buffer_rows: usize,
    pub discard_bins: Vec<BinBlock>,
    pub nominal_rx_bins: NominalRxBins,
    pub doppler: DopplerConfig,
    pub cfar: CfarConfig,
    pub tracking: TrackingConfig,
    #[serde(default)]
    pub debug_logging_level: DebugLoggingLevel,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DebugLoggingLevel {
    Off,
    Basic,
    Verbose,
}

impl Default for DebugLoggingLevel {
    fn default() -> Self {
        DebugLoggingLevel::Verbose
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct NominalRxBins {
    pub low_block: RxBinBlock,
    pub high_block: RxBinBlock,
    pub description: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RxBinBlock {
    pub start: usize,
    pub end: usize,
    pub step: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DopplerConfig {
    pub search_bin_range: usize,
    pub search_row_offset: usize,
    pub description: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CfarConfig {
    pub non_detect_average_rows: usize,
    pub peak_to_average_ratio: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TrackingConfig {
    pub energy_drop_threshold: f64,
    pub drop_rows_required: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PayloadConfig {
    pub encoding: String,
    pub interpret_as: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OutputConfig {
    pub deliver_payload: bool,
    pub include_source_mac: bool,
    pub validate_destination_mac: bool,
    pub allow_broadcast: bool,
}

impl ModemConfiguration {
    pub fn from_yaml_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let contents = fs::read_to_string(path)?;
        let config: ModemConfiguration = serde_yaml::from_str(&contents)?;
        Ok(config)
    }
}
