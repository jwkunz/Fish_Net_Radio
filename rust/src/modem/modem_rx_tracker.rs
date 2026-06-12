use crate::modem::modem_configuration::{ReceiverConfig, RxBinBlock};
use crate::modem::modem_rx_debug::{emit_debug, RxDebugEvent, RxDebugTx, RxStageId};
use crate::modem::modem_rx_types::{SymbolStream, TimeFrequencyImage};
use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use std::sync::Arc;

pub struct Tracker {
    low_block: RxBinBlock,
    high_block: RxBinBlock,
    debug_tx: Option<RxDebugTx>,
    seq: u64,
}

impl Tracker {
    pub fn new(config: ReceiverConfig, debug_tx: Option<RxDebugTx>) -> Self {
        Tracker {
            low_block: config.nominal_rx_bins.low_block,
            high_block: config.nominal_rx_bins.high_block,
            debug_tx,
            seq: 0,
        }
    }

    fn valid_bins(&self) -> Vec<usize> {
        let mut bins = Vec::new();
        Self::extend_bins(&mut bins, &self.low_block);
        Self::extend_bins(&mut bins, &self.high_block);
        bins
    }

    fn extend_bins(target: &mut Vec<usize>, block: &RxBinBlock) {
        let mut bin = block.start;
        while bin <= block.end {
            target.push(bin);
            bin = bin.saturating_add(block.step.max(1));
            if bin == usize::MAX {
                break;
            }
        }
    }
}

impl StreamOperatorManagement for Tracker {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        self.seq = 0;
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<Arc<TimeFrequencyImage>, Arc<SymbolStream>> for Tracker {
    fn flush(&mut self) -> Result<Option<Vec<Arc<SymbolStream>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(
        &mut self,
        data_in: &[Arc<TimeFrequencyImage>],
    ) -> Result<Option<Vec<Arc<SymbolStream>>>, ErrorsJSL> {
        let image = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        self.seq += 1;

        let valid_bins = self.valid_bins();
        let mut symbols = Vec::new();
        let mut peak_bin = 0usize;
        let mut peak_energy = 0.0_f32;

        for row in &image {
            let mut row_peak_bin = None;
            let mut row_peak_energy = f32::MIN;
            for &bin in &valid_bins {
                if let Some(sample) = row.get(bin) {
                    let energy = sample.norm_sqr();
                    if energy > row_peak_energy {
                        row_peak_energy = energy;
                        row_peak_bin = Some(bin);
                    }
                }
            }

            let chosen_bin = row_peak_bin.unwrap_or(0);
            peak_bin = chosen_bin;
            peak_energy = row_peak_energy.max(0.0);
            symbols.push(map_bin_to_symbol(chosen_bin, &self.low_block, &self.high_block));
        }

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Tracker,
                seq: self.seq,
                name: "valid_bin_count",
                value: valid_bins.len() as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Tracker,
                seq: self.seq,
                name: "peak_bin",
                value: peak_bin as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Tracker,
                seq: self.seq,
                name: "peak_energy",
                value: peak_energy as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Message {
                stage: RxStageId::Tracker,
                seq: self.seq,
                summary: format!("tracked_symbols={}", symbols.len()),
            },
        );

        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(vec![Arc::new(symbols)]))
        }
    }
}

fn map_bin_to_symbol(bin: usize, low_block: &RxBinBlock, high_block: &RxBinBlock) -> u8 {
    if bin >= low_block.start && bin <= low_block.end {
        let step = low_block.step.max(1);
        let offset = (bin - low_block.start) / step;
        return offset.min(127) as u8;
    }

    if bin >= high_block.start && bin <= high_block.end {
        let step = high_block.step.max(1);
        let offset = (bin - high_block.start) / step;
        return 128u8.saturating_add(offset.min(127) as u8);
    }

    0
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
    fn tracker_maps_peak_bins_to_symbols() {
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut tracker = Tracker::new(test_receiver_config(), Some(debug_tx));
        let mut row = vec![Complex::new(0.0, 0.0); 8];
        row[5] = Complex::new(4.0, 0.0);
        let image = vec![row];

        let outputs = tracker.process(&[Arc::new(image)]).unwrap().unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].as_ref(), &vec![129u8]);

        let mut saw_message = false;
        while let Ok(event) = debug_rx.try_recv() {
            if let RxDebugEvent::Message { summary, .. } = event {
                if summary.contains("tracked_symbols=1") {
                    saw_message = true;
                }
            }
        }

        assert!(saw_message);
    }
}
