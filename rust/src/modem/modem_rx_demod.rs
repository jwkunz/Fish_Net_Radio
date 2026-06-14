use crate::modem::modem_configuration::ReceiverConfig;
use crate::modem::modem_rx_debug::{emit_debug, RxDebugEvent, RxDebugTx, RxStageId};
use crate::modem::modem_rx_types::{FrameBytes, SymbolStream};
use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use std::sync::Arc;

pub struct Demodulator {
    expected_rows: usize,
    debug_tx: Option<RxDebugTx>,
    seq: u64,
}

impl Demodulator {
    pub fn new(config: ReceiverConfig, debug_tx: Option<RxDebugTx>) -> Self {
        Demodulator {
            expected_rows: config.symbol_rows.max(1),
            debug_tx,
            seq: 0,
        }
    }
}

impl StreamOperatorManagement for Demodulator {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        self.seq = 0;
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<Arc<SymbolStream>, Arc<FrameBytes>> for Demodulator {
    fn flush(&mut self) -> Result<Option<Vec<Arc<FrameBytes>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(
        &mut self,
        data_in: &[Arc<SymbolStream>],
    ) -> Result<Option<Vec<Arc<FrameBytes>>>, ErrorsJSL> {
        let symbols = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        self.seq += 1;

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Demodulator,
                seq: self.seq,
                name: "input_bytes",
                value: symbols.len() as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Demodulator,
                seq: self.seq,
                name: "expected_rows",
                value: self.expected_rows as f64,
            },
        );

        if symbols.is_empty() {
            emit_debug(
                &self.debug_tx,
                RxDebugEvent::Warning {
                    stage: RxStageId::Demodulator,
                    seq: self.seq,
                    detail: "empty symbol stream".to_string(),
                },
            );
            return Ok(None);
        }

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Snapshot {
                stage: RxStageId::Demodulator,
                seq: self.seq,
                label: "frame_bytes",
                rows: 1,
                cols: symbols.len(),
            },
        );

        Ok(Some(vec![Arc::new(symbols)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modem::modem_configuration::{
        BinBlock, CfarConfig, DebugLoggingLevel, NominalRxBins, ReceiverConfig, RxBinBlock,
        TrackingConfig,
    };
    use crate::modem::modem_rx_debug::RxDebugEvent;
    use std::sync::mpsc;

    fn test_receiver_config() -> ReceiverConfig {
        ReceiverConfig {
            fft_size: 8,
            fft_overlap_samples: 0,
            symbol_rows: 4,
            preamble_rows: 2,
            search_buffer_rows: 3,
            discard_bins: vec![BinBlock { start: 2, end: 3 }],
            nominal_rx_bins: NominalRxBins {
                low_block: RxBinBlock {
                    start: 0,
                    end: 3,
                    step: 1,
                },
                high_block: RxBinBlock {
                    start: 4,
                    end: 7,
                    step: 1,
                },
            },
            cfar: CfarConfig {
                non_detect_average_rows: 2,
                peak_to_average_ratio: 2,
            },
            tracking: TrackingConfig {
                energy_drop_threshold: 0.1,
                drop_rows_required: 1,
            },
            debug_logging_level: DebugLoggingLevel::Verbose,
        }
    }

    #[test]
    fn demodulator_emits_frame_bytes_debug() {
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut demod = Demodulator::new(test_receiver_config(), Some(debug_tx));
        let symbols = vec![1u8, 2, 3, 4];

        let outputs = demod
            .process(&[Arc::new(symbols.clone())])
            .unwrap()
            .unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].as_ref(), &symbols);

        let mut saw_input = false;
        let mut saw_snapshot = false;
        while let Ok(event) = debug_rx.try_recv() {
            match event {
                RxDebugEvent::Metric { name, .. } if name == "input_bytes" => saw_input = true,
                RxDebugEvent::Snapshot { label, .. } if label == "frame_bytes" => {
                    saw_snapshot = true
                }
                _ => {}
            }
        }

        assert!(saw_input);
        assert!(saw_snapshot);
    }
}
