use crate::modem::modem_configuration::ReceiverConfig;
use crate::modem::modem_rx_debug::{emit_debug, RxDebugEvent, RxDebugTx, RxStageId};
use crate::modem::modem_rx_types::TimeFrequencyImage;
use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use std::sync::Arc;

pub struct Acquisition {
    preamble_rows: usize,
    debug_tx: Option<RxDebugTx>,
    seq: u64,
}

impl Acquisition {
    pub fn new(config: ReceiverConfig, debug_tx: Option<RxDebugTx>) -> Self {
        Acquisition {
            preamble_rows: config.preamble_rows.max(1),
            debug_tx,
            seq: 0,
        }
    }
}

impl StreamOperatorManagement for Acquisition {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        self.seq = 0;
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<Arc<TimeFrequencyImage>, Arc<TimeFrequencyImage>> for Acquisition {
    fn flush(&mut self) -> Result<Option<Vec<Arc<TimeFrequencyImage>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(
        &mut self,
        data_in: &[Arc<TimeFrequencyImage>],
    ) -> Result<Option<Vec<Arc<TimeFrequencyImage>>>, ErrorsJSL> {
        let image = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        self.seq += 1;

        let row_energies: Vec<f64> = image
            .iter()
            .map(|row| row.iter().map(|sample| sample.norm_sqr() as f64).sum::<f64>())
            .collect();
        let peak_row = row_energies
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        let peak_energy = row_energies.get(peak_row).copied().unwrap_or(0.0);
        let average_energy = if row_energies.is_empty() {
            0.0
        } else {
            row_energies.iter().sum::<f64>() / row_energies.len() as f64
        };
        let correlation_score = if average_energy > 0.0 {
            peak_energy / average_energy
        } else {
            0.0
        };

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Acquisition,
                seq: self.seq,
                name: "rows_seen",
                value: image.len() as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Acquisition,
                seq: self.seq,
                name: "peak_row",
                value: peak_row as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Acquisition,
                seq: self.seq,
                name: "correlation_score",
                value: correlation_score,
            },
        );

        if image.len() < self.preamble_rows {
            return Ok(None);
        }

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Snapshot {
                stage: RxStageId::Acquisition,
                seq: self.seq,
                label: "acquisition_window",
                rows: image.len(),
                cols: image.first().map(|row| row.len()).unwrap_or(0),
            },
        );

        Ok(Some(vec![Arc::new(image)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modem::modem_configuration::{
        BinBlock, CfarConfig, DebugLoggingLevel, DopplerConfig, NominalRxBins, ReceiverConfig,
        RxBinBlock, TrackingConfig,
    };
    use crate::modem::modem_rx_debug::RxDebugEvent;
    use num_complex::Complex;
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
                description: "test".to_string(),
            },
            doppler: DopplerConfig {
                search_bin_range: 0,
                search_row_offset: 0,
                description: "test".to_string(),
            },
            cfar: CfarConfig {
                non_detect_average_rows: 1,
                peak_to_average_ratio: 1,
            },
            tracking: TrackingConfig {
                energy_drop_threshold: 0.1,
                drop_rows_required: 1,
            },
            debug_logging_level: DebugLoggingLevel::Verbose,
        }
    }

    #[test]
    fn acquisition_waits_for_preamble_rows_and_emits_score() {
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut acquisition = Acquisition::new(test_receiver_config(), Some(debug_tx));
        let image = vec![
            vec![Complex::new(0.0, 0.0); 8],
            vec![Complex::new(1.0, 0.0); 8],
        ];

        let outputs = acquisition.process(&[Arc::new(image)]).unwrap().unwrap();
        assert_eq!(outputs.len(), 1);

        let mut saw_score = false;
        let mut saw_window = false;
        while let Ok(event) = debug_rx.try_recv() {
            match event {
                RxDebugEvent::Metric {
                    name: "correlation_score",
                    ..
                } => saw_score = true,
                RxDebugEvent::Snapshot {
                    label: "acquisition_window",
                    ..
                } => saw_window = true,
                _ => {}
            }
        }

        assert!(saw_score);
        assert!(saw_window);
    }
}
