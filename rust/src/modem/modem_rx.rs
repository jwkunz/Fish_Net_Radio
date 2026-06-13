use crate::modem::modem_configuration::{ModemConfiguration, ReceiverConfig};
use crate::modem::modem_rx_acquisition::Acquisition;
use crate::modem::modem_rx_cfar::CfarDetector;
use crate::modem::modem_rx_debug::{emit_debug, RxDebugEvent, RxDebugTx, RxStageId};
use crate::modem::modem_rx_demod::Demodulator;
use crate::modem::modem_rx_fft::FftFrontEnd;
use crate::modem::modem_rx_image::ImageBuilder;
use crate::modem::modem_rx_parser::FrameParser;
use crate::modem::modem_rx_source::RxSource;
use crate::modem::modem_rx_tracker::Tracker;
use crate::modem::modem_rx_types::{
    RawComplexFrame, RxMessage, SpectrumFrame, SymbolStream,
};
use lib_jsl::dsp::discrete::stream_operator::StreamOperator;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::Duration;

pub struct ModemRX {
    config: ModemConfiguration,
}

impl ModemRX {
    pub fn new(config: ModemConfiguration) -> Self {
        ModemRX { config }
    }

    pub fn run(self, message_tx: mpsc::Sender<RxMessage>, running: Arc<AtomicBool>) {
        self.run_with_debug(message_tx, running, None);
    }

    pub fn run_with_debug(
        self,
        message_tx: mpsc::Sender<RxMessage>,
        running: Arc<AtomicBool>,
        debug_tx: Option<RxDebugTx>,
    ) {
        let config = self.config;
        let receiver_config = config.receiver.clone();
        let framing_config = config.framing.clone();
        let payload_config = config.payload.clone();
        let output_config = config.output.clone();
        let preamble_config = config.transmitter.preamble.clone();
        let rx_address = config.gnuradio_instance_address_rx.clone();
        let rx_port = config.gnuradio_instance_port_rx.clone();

        let (source_tx, fft_rx) = mpsc::channel::<Arc<RawComplexFrame>>();
        let (fft_tx, search_rx) = mpsc::channel::<Arc<SpectrumFrame>>();
        let (search_tx, demod_rx) = mpsc::channel::<Arc<SymbolStream>>();

        let source_handle = spawn_source_stage(
            source_tx,
            running.clone(),
            debug_tx.clone(),
            rx_address,
            rx_port,
        );
        let fft_handle = spawn_fft_stage(
            fft_rx,
            fft_tx,
            running.clone(),
            debug_tx.clone(),
            receiver_config.clone(),
        );
        let search_handle = spawn_search_stage(
            search_rx,
            search_tx,
            running.clone(),
            debug_tx.clone(),
            receiver_config.clone(),
        );
        let demod_handle = spawn_demod_stage(
            demod_rx,
            message_tx,
            running.clone(),
            debug_tx,
            receiver_config,
            framing_config,
            payload_config,
            output_config,
            preamble_config,
        );

        let _ = source_handle.join();
        let _ = fft_handle.join();
        let _ = search_handle.join();
        let _ = demod_handle.join();
    }
}

fn spawn_source_stage(
    source_tx: mpsc::Sender<Arc<RawComplexFrame>>,
    running: Arc<AtomicBool>,
    debug_tx: Option<RxDebugTx>,
    address: String,
    port: String,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut source = match RxSource::new(&address, &port) {
            Ok(source) => source,
            Err(err) => {
                emit_debug(
                    &debug_tx,
                    RxDebugEvent::Error {
                        stage: RxStageId::Source,
                        seq: 0,
                        detail: format!("failed to connect source: {}", err),
                    },
                );
                return;
            }
        };
        let mut seq = 0u64;
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStart {
                stage: RxStageId::Source,
                seq,
                detail: format!("source stage started on {}:{}", address, port),
            },
        );
        while running.load(Ordering::SeqCst) {
            match source.process(&[()]) {
                Ok(Some(frames)) => {
                for frame in frames {
                    seq += 1;
                    emit_debug(
                        &debug_tx,
                        RxDebugEvent::Metric {
                            stage: RxStageId::Source,
                            seq,
                            name: "frame_len",
                            value: frame.len() as f64,
                        },
                    );
                    if source_tx.send(frame).is_err() {
                        emit_debug(
                            &debug_tx,
                            RxDebugEvent::Warning {
                                stage: RxStageId::Source,
                                seq,
                                detail: "downstream closed".to_string(),
                            },
                        );
                        emit_debug(
                            &debug_tx,
                            RxDebugEvent::StageStop {
                                stage: RxStageId::Source,
                                seq,
                                detail: "source stage stopped".to_string(),
                            },
                        );
                        return;
                    }
                }
                }
                Ok(None) => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(err) => {
                    emit_debug(
                        &debug_tx,
                        RxDebugEvent::Error {
                            stage: RxStageId::Source,
                            seq,
                            detail: format!("source receive error: {:?}", err),
                        },
                    );
                    thread::sleep(Duration::from_millis(25));
                }
            }
        }
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStop {
                stage: RxStageId::Source,
                seq,
                detail: "source stage stopped".to_string(),
            },
        );
    })
}

fn spawn_fft_stage(
    in_rx: mpsc::Receiver<Arc<RawComplexFrame>>,
    out_tx: mpsc::Sender<Arc<SpectrumFrame>>,
    running: Arc<AtomicBool>,
    debug_tx: Option<RxDebugTx>,
    receiver_config: ReceiverConfig,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut fft = FftFrontEnd::new(receiver_config, debug_tx.clone());
        let mut seq = 0u64;
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStart {
                stage: RxStageId::FrontEnd,
                seq,
                detail: "fft front end started".to_string(),
            },
        );
        while running.load(Ordering::SeqCst) {
            match in_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(frame) => {
                    seq += 1;
                    emit_debug(
                        &debug_tx,
                        RxDebugEvent::Metric {
                            stage: RxStageId::FrontEnd,
                            seq,
                            name: "input_frame_len",
                            value: frame.len() as f64,
                        },
                    );
                    if let Ok(Some(spectra)) = fft.process(&[frame]) {
                        for spectrum in spectra {
                            if out_tx.send(spectrum).is_err() {
                                emit_debug(
                                    &debug_tx,
                                    RxDebugEvent::Warning {
                                        stage: RxStageId::FrontEnd,
                                        seq,
                                        detail: "downstream closed".to_string(),
                                    },
                                );
                                emit_debug(
                                    &debug_tx,
                                    RxDebugEvent::StageStop {
                                        stage: RxStageId::FrontEnd,
                                        seq,
                                        detail: "fft front end stopped".to_string(),
                                    },
                                );
                                return;
                            }
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => {
                    emit_debug(
                        &debug_tx,
                        RxDebugEvent::StageStop {
                            stage: RxStageId::FrontEnd,
                            seq,
                            detail: "fft front end stopped".to_string(),
                        },
                    );
                    return;
                }
            }
        }
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStop {
                stage: RxStageId::FrontEnd,
                seq,
                detail: "fft front end stopped".to_string(),
            },
        );
    })
}

fn spawn_search_stage(
    in_rx: mpsc::Receiver<Arc<SpectrumFrame>>,
    out_tx: mpsc::Sender<Arc<SymbolStream>>,
    running: Arc<AtomicBool>,
    debug_tx: Option<RxDebugTx>,
    receiver_config: ReceiverConfig,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut image_builder = ImageBuilder::new(receiver_config.clone(), debug_tx.clone());
        let mut acquisition = Acquisition::new(receiver_config.clone(), debug_tx.clone());
        let mut cfar = CfarDetector::new(receiver_config.clone(), debug_tx.clone());
        let mut tracker = Tracker::new(receiver_config, debug_tx.clone());
        let mut seq = 0u64;
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStart {
                stage: RxStageId::Search,
                seq,
                detail: "search stage started".to_string(),
            },
        );
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStart {
                stage: RxStageId::Acquisition,
                seq,
                detail: "acquisition started".to_string(),
            },
        );
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStart {
                stage: RxStageId::Cfar,
                seq,
                detail: "cfar started".to_string(),
            },
        );
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStart {
                stage: RxStageId::Tracker,
                seq,
                detail: "tracker started".to_string(),
            },
        );
        while running.load(Ordering::SeqCst) {
            match in_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(spectrum) => {
                    seq += 1;
                    if let Ok(Some(images)) = image_builder.process(&[spectrum]) {
                        for image_frame in images {
                            emit_debug(
                                &debug_tx,
                                RxDebugEvent::Snapshot {
                                    stage: RxStageId::Search,
                                    seq,
                                    label: "time_frequency_image",
                                    rows: image_frame.len(),
                                    cols: image_frame.first().map(|row| row.len()).unwrap_or(0),
                                },
                            );
                            if let Ok(Some(acquired)) = acquisition.process(&[image_frame]) {
                                for image in acquired {
                                    emit_debug(
                                        &debug_tx,
                                        RxDebugEvent::Snapshot {
                                            stage: RxStageId::Acquisition,
                                            seq,
                                            label: "acquisition_output",
                                            rows: image.len(),
                                            cols: image.first().map(|row| row.len()).unwrap_or(0),
                                        },
                                    );
                                    if let Ok(Some(detections)) = cfar.process(&[image]) {
                                        for image in detections {
                                            emit_debug(
                                                &debug_tx,
                                                RxDebugEvent::Metric {
                                                    stage: RxStageId::Cfar,
                                                    seq,
                                                    name: "cfar_image_rows",
                                                    value: image.len() as f64,
                                                },
                                            );
                                            if let Ok(Some(symbols)) = tracker.process(&[image]) {
                                                for symbol_stream in symbols {
                                                    emit_debug(
                                                        &debug_tx,
                                                        RxDebugEvent::Metric {
                                                            stage: RxStageId::Tracker,
                                                            seq,
                                                            name: "symbol_count",
                                                            value: symbol_stream.len() as f64,
                                                        },
                                                    );
                                                    if out_tx.send(symbol_stream).is_err() {
                                                        emit_debug(
                                                            &debug_tx,
                                                            RxDebugEvent::Warning {
                                                                stage: RxStageId::Tracker,
                                                                seq,
                                                                detail: "downstream closed".to_string(),
                                                            },
                                                        );
                                                        emit_debug(
                                                            &debug_tx,
                                                            RxDebugEvent::StageStop {
                                                                stage: RxStageId::Search,
                                                                seq,
                                                                detail: "search stage stopped".to_string(),
                                                            },
                                                        );
                                                        return;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => {
                    emit_debug(
                        &debug_tx,
                        RxDebugEvent::StageStop {
                            stage: RxStageId::Search,
                            seq,
                            detail: "search stage stopped".to_string(),
                        },
                    );
                    return;
                }
            }
        }
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStop {
                stage: RxStageId::Tracker,
                seq,
                detail: "tracker stopped".to_string(),
            },
        );
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStop {
                stage: RxStageId::Cfar,
                seq,
                detail: "cfar stopped".to_string(),
            },
        );
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStop {
                stage: RxStageId::Acquisition,
                seq,
                detail: "acquisition stopped".to_string(),
            },
        );
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStop {
                stage: RxStageId::Search,
                seq,
                detail: "search stage stopped".to_string(),
            },
        );
    })
}

fn spawn_demod_stage(
    in_rx: mpsc::Receiver<Arc<SymbolStream>>,
    message_tx: mpsc::Sender<RxMessage>,
    running: Arc<AtomicBool>,
    debug_tx: Option<RxDebugTx>,
    receiver_config: ReceiverConfig,
    framing_config: crate::modem::modem_configuration::FramingConfig,
    payload_config: crate::modem::modem_configuration::PayloadConfig,
    output_config: crate::modem::modem_configuration::OutputConfig,
    preamble_config: crate::modem::modem_configuration::PreambleConfig,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut demodulator = Demodulator::new(receiver_config, debug_tx.clone());
        let mut parser = FrameParser::new(
            framing_config,
            payload_config,
            output_config,
            preamble_config,
            debug_tx.clone(),
        );
        let mut seq = 0u64;
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStart {
                stage: RxStageId::Demodulator,
                seq,
                detail: "demod stage started".to_string(),
            },
        );
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStart {
                stage: RxStageId::Parser,
                seq,
                detail: "parser started".to_string(),
            },
        );
        while running.load(Ordering::SeqCst) {
            match in_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(symbols) => {
                    seq += 1;
                    emit_debug(
                        &debug_tx,
                        RxDebugEvent::Metric {
                            stage: RxStageId::Demodulator,
                            seq,
                            name: "input_symbols",
                            value: symbols.len() as f64,
                        },
                    );
                    if let Ok(Some(decoded)) = demodulator.process(&[symbols]) {
                        for frame_bytes in decoded {
                            emit_debug(
                                &debug_tx,
                                RxDebugEvent::Metric {
                                    stage: RxStageId::Demodulator,
                                    seq,
                                    name: "decoded_bytes",
                                    value: frame_bytes.len() as f64,
                                },
                            );
                            if let Ok(Some(messages)) = parser.process(&[frame_bytes]) {
                                for message in messages {
                                    emit_debug(
                                        &debug_tx,
                                        RxDebugEvent::Message {
                                            stage: RxStageId::Parser,
                                            seq,
                                            summary: format!(
                                                "source={} payload_len={}",
                                                message.source_mac,
                                                message.payload.len()
                                            ),
                                        },
                                    );
                                    if message_tx.send(message).is_err() {
                                        emit_debug(
                                            &debug_tx,
                                            RxDebugEvent::Warning {
                                                stage: RxStageId::Parser,
                                                seq,
                                                detail: "message sink closed".to_string(),
                                            },
                                        );
                                        emit_debug(
                                            &debug_tx,
                                            RxDebugEvent::StageStop {
                                                stage: RxStageId::Demodulator,
                                                seq,
                                                detail: "demod stage stopped".to_string(),
                                            },
                                        );
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => {
                    emit_debug(
                        &debug_tx,
                        RxDebugEvent::StageStop {
                            stage: RxStageId::Demodulator,
                            seq,
                            detail: "demod stage stopped".to_string(),
                        },
                    );
                    return;
                }
            }
        }
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStop {
                stage: RxStageId::Parser,
                seq,
                detail: "parser stopped".to_string(),
            },
        );
        emit_debug(
            &debug_tx,
            RxDebugEvent::StageStop {
                stage: RxStageId::Demodulator,
                seq,
                detail: "demod stage stopped".to_string(),
            },
        );
    })
}
