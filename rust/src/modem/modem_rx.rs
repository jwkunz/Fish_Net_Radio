use crate::modem::modem_rx_acquisition::Acquisition;
use crate::modem::modem_rx_cfar::CfarDetector;
use crate::modem::modem_rx_demod::Demodulator;
use crate::modem::modem_rx_fft::FftFrontEnd;
use crate::modem::modem_rx_image::ImageBuilder;
use crate::modem::modem_rx_parser::FrameParser;
use crate::modem::modem_rx_source::RxSource;
use crate::modem::modem_rx_tracker::Tracker;
use crate::modem::modem_rx_types::{RawComplexFrame, RxMessage, SpectrumFrame, SymbolStream, TimeFrequencyImage};
use lib_jsl::dsp::discrete::stream_operator::StreamOperator;
use std::sync::{mpsc, Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;

pub struct ModemRX;

impl ModemRX {
    pub fn new() -> Self {
        ModemRX
    }

    pub fn run(self, message_tx: mpsc::Sender<RxMessage>, running: Arc<AtomicBool>) {
        let (source_tx, fft_rx) = mpsc::channel::<RawComplexFrame>();
        let (fft_tx, image_rx) = mpsc::channel::<SpectrumFrame>();
        let (image_tx, acquisition_rx) = mpsc::channel::<TimeFrequencyImage>();
        let (acq_tx, cfar_rx) = mpsc::channel::<TimeFrequencyImage>();
        let (cfar_tx, tracker_rx) = mpsc::channel::<TimeFrequencyImage>();
        let (tracker_tx, demod_rx) = mpsc::channel::<SymbolStream>();
        let (demod_tx, parser_rx) = mpsc::channel::<SymbolStream>();

        let source_handle = {
            let running = running.clone();
            thread::spawn(move || {
                let mut source = RxSource::new();
                while running.load(Ordering::SeqCst) {
                    if let Ok(Some(frames)) = source.process(&[()]) {
                    for frame in frames {
                        if source_tx.send(frame).is_err() {
                            break;
                        }
                    }
                }
                    thread::sleep(Duration::from_millis(500));
                }
            })
        };

        let fft_handle = spawn_stage(FftFrontEnd::new(), fft_rx, fft_tx, running.clone());
        let image_handle = spawn_stage(ImageBuilder::new(), image_rx, image_tx, running.clone());
        let acquisition_handle = spawn_stage(Acquisition::new(), acquisition_rx, acq_tx, running.clone());
        let cfar_handle = spawn_stage(CfarDetector::new(), cfar_rx, cfar_tx, running.clone());
        let tracker_handle = spawn_stage(Tracker::new(), tracker_rx, tracker_tx, running.clone());
        let demod_handle = spawn_stage(Demodulator::new(), demod_rx, demod_tx, running.clone());

        let parser_handle = {
            let running = running.clone();
            thread::spawn(move || {
                let mut parser = FrameParser::new();
                while running.load(Ordering::SeqCst) {
                    match parser_rx.recv_timeout(Duration::from_secs(1)) {
                        Ok(symbols) => {
                            if let Ok(Some(messages)) = parser.process(&[symbols]) {
                                for message in messages {
                                    let _ = message_tx.send(message);
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    }
                }
            })
        };

        let _ = source_handle.join();
        let _ = fft_handle.join();
        let _ = image_handle.join();
        let _ = acquisition_handle.join();
        let _ = cfar_handle.join();
        let _ = tracker_handle.join();
        let _ = demod_handle.join();
        let _ = parser_handle.join();
    }
}

fn spawn_stage<In, Out, Operator>(
    mut operator: Operator,
    in_rx: mpsc::Receiver<In>,
    out_tx: mpsc::Sender<Out>,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> 
where
    In: Send + 'static,
    Out: Send + 'static,
    Operator: StreamOperator<In, Out> + Send + 'static,
{
    thread::spawn(move || {
        while running.load(Ordering::SeqCst) {
            match in_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(input) => {
                    let buffer = vec![input];
                    if let Ok(Some(outputs)) = operator.process(&buffer) {
                        for output in outputs {
                            let _ = out_tx.send(output);
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            }
        }
    })
}
