pub mod modem_tx;
pub mod modem_configuration;
pub mod modem_frame;
pub mod modem_waveform;

// Receiver pipeline stages:
// 1) FFT front end and image construction
// 2) Search, CFAR, and tracking
// 3) Demodulation and frame parsing
pub mod modem_rx;
pub mod modem_rx_types;
pub mod modem_rx_source;
pub mod modem_rx_fft;
pub mod modem_rx_image;
pub mod modem_rx_acquisition;
pub mod modem_rx_cfar;
pub mod modem_rx_tracker;
pub mod modem_rx_demod;
pub mod modem_rx_parser;
