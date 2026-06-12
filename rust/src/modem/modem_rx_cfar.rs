use crate::modem::modem_configuration::ReceiverConfig;
use crate::modem::modem_rx_debug::{emit_debug, RxDebugEvent, RxDebugTx, RxStageId};
use crate::modem::modem_rx_types::TimeFrequencyImage;
use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use std::collections::VecDeque;
use std::sync::Arc;

pub struct CfarDetector {
    non_detect_average_rows: usize,
    peak_to_average_ratio: f64,
    noise_floor: VecDeque<f64>,
    debug_tx: Option<RxDebugTx>,
    seq: u64,
}

impl CfarDetector {
    pub fn new(config: ReceiverConfig, debug_tx: Option<RxDebugTx>) -> Self {
        CfarDetector {
            non_detect_average_rows: config.cfar.non_detect_average_rows.max(1),
            peak_to_average_ratio: config.cfar.peak_to_average_ratio as f64,
            noise_floor: VecDeque::new(),
            debug_tx,
            seq: 0,
        }
    }
}

impl StreamOperatorManagement for CfarDetector {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        self.seq = 0;
        self.noise_floor.clear();
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<Arc<TimeFrequencyImage>, Arc<TimeFrequencyImage>> for CfarDetector {
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
        let peak_energy = row_energies
            .iter()
            .copied()
            .fold(0.0_f64, f64::max);
        let average_energy = if row_energies.is_empty() {
            0.0
        } else {
            row_energies.iter().sum::<f64>() / row_energies.len() as f64
        };

        let noise_floor = if self.noise_floor.is_empty() {
            average_energy.max(1.0)
        } else {
            self.noise_floor.iter().sum::<f64>() / self.noise_floor.len() as f64
        };
        let ratio = if noise_floor > 0.0 {
            peak_energy / noise_floor
        } else {
            0.0
        };
        let detected = ratio >= self.peak_to_average_ratio;

        if !detected && average_energy > 0.0 {
            if self.noise_floor.len() >= self.non_detect_average_rows {
                self.noise_floor.pop_front();
            }
            self.noise_floor.push_back(average_energy);
        }

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Cfar,
                seq: self.seq,
                name: "noise_floor",
                value: noise_floor,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Cfar,
                seq: self.seq,
                name: "peak_to_average",
                value: ratio,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Message {
                stage: RxStageId::Cfar,
                seq: self.seq,
                summary: format!("detected={}", detected),
            },
        );

        if detected {
            emit_debug(
                &self.debug_tx,
                RxDebugEvent::Snapshot {
                    stage: RxStageId::Cfar,
                    seq: self.seq,
                    label: "cfar_detection",
                    rows: image.len(),
                    cols: image.first().map(|row| row.len()).unwrap_or(0),
                },
            );
            Ok(Some(vec![Arc::new(image)]))
        } else {
            Ok(None)
        }
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
    fn cfar_detects_high_energy_image() {
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut cfar = CfarDetector::new(test_receiver_config(), Some(debug_tx));
        let quiet = vec![vec![Complex::new(1.0, 0.0); 8]];
        let loud = vec![vec![Complex::new(5.0, 0.0); 8]];

        assert!(cfar.process(&[Arc::new(quiet)]).unwrap().is_none());
        let outputs = cfar.process(&[Arc::new(loud)]).unwrap().unwrap();
        assert_eq!(outputs.len(), 1);

        let mut saw_detection_message = false;
        while let Ok(event) = debug_rx.try_recv() {
            if let RxDebugEvent::Message { summary, .. } = event {
                if summary.contains("detected=true") {
                    saw_detection_message = true;
                }
            }
        }

        assert!(saw_detection_message);
    }
}
