use crate::modem::modem_rx_acquisition::Acquisition;
use crate::modem::modem_rx_cfar::CfarDetector;
use crate::modem::modem_rx_demod::Demodulator;
use crate::modem::modem_rx_fft::FftFrontEnd;
use crate::modem::modem_rx_image::ImageBuilder;
use crate::modem::modem_rx_parser::FrameParser;
use crate::modem::modem_rx_source::RxSource;
use crate::modem::modem_rx_tracker::Tracker;
use crate::modem::modem_rx_types::{
    RawComplexFrame, RxMessage, SymbolStream, TimeFrequencyImage,
};
use lib_jsl::dsp::discrete::stream_operator::StreamOperator;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::Duration;

pub struct ModemRX;

impl ModemRX {
    pub fn new() -> Self {
        ModemRX
    }

    pub fn run(self, message_tx: mpsc::Sender<RxMessage>, running: Arc<AtomicBool>) {
        let (source_tx, fft_rx) = mpsc::channel::<Arc<RawComplexFrame>>();
        let (fft_tx, search_rx) = mpsc::channel::<Arc<TimeFrequencyImage>>();
        let (search_tx, demod_rx) = mpsc::channel::<Arc<SymbolStream>>();

        let source_handle = spawn_source_stage(source_tx, running.clone());
        let fft_handle = spawn_fft_stage(fft_rx, fft_tx, running.clone());
        let search_handle = spawn_search_stage(search_rx, search_tx, running.clone());
        let demod_handle = spawn_demod_stage(demod_rx, message_tx, running.clone());

        let _ = source_handle.join();
        let _ = fft_handle.join();
        let _ = search_handle.join();
        let _ = demod_handle.join();
    }
}

fn spawn_source_stage(
    source_tx: mpsc::Sender<Arc<RawComplexFrame>>,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut source = RxSource::new();
        while running.load(Ordering::SeqCst) {
            if let Ok(Some(frames)) = source.process(&[()]) {
                for frame in frames {
                    if source_tx.send(frame).is_err() {
                        return;
                    }
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    })
}

fn spawn_fft_stage(
    in_rx: mpsc::Receiver<Arc<RawComplexFrame>>,
    out_tx: mpsc::Sender<Arc<TimeFrequencyImage>>,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut fft = FftFrontEnd::new();
        let mut image = ImageBuilder::new();
        while running.load(Ordering::SeqCst) {
            match in_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(frame) => {
                    if let Ok(Some(spectra)) = fft.process(&[frame]) {
                        for spectrum in spectra {
                            if let Ok(Some(images)) = image.process(&[spectrum]) {
                                for image_frame in images {
                                    if out_tx.send(image_frame).is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return,
            }
        }
    })
}

fn spawn_search_stage(
    in_rx: mpsc::Receiver<Arc<TimeFrequencyImage>>,
    out_tx: mpsc::Sender<Arc<SymbolStream>>,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut acquisition = Acquisition::new();
        let mut cfar = CfarDetector::new();
        let mut tracker = Tracker::new();
        while running.load(Ordering::SeqCst) {
            match in_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(image_frame) => {
                    if let Ok(Some(acquired)) = acquisition.process(&[image_frame]) {
                        for image in acquired {
                            if let Ok(Some(detections)) = cfar.process(&[image]) {
                                for image in detections {
                                    if let Ok(Some(symbols)) = tracker.process(&[image]) {
                                        for symbol_stream in symbols {
                                            if out_tx.send(symbol_stream).is_err() {
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return,
            }
        }
    })
}

fn spawn_demod_stage(
    in_rx: mpsc::Receiver<Arc<SymbolStream>>,
    message_tx: mpsc::Sender<RxMessage>,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut demodulator = Demodulator::new();
        let mut parser = FrameParser::new();
        while running.load(Ordering::SeqCst) {
            match in_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(symbols) => {
                    if let Ok(Some(decoded)) = demodulator.process(&[symbols]) {
                        for symbol_stream in decoded {
                            if let Ok(Some(messages)) = parser.process(&[symbol_stream]) {
                                for message in messages {
                                    if message_tx.send(message).is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return,
            }
        }
    })
}
