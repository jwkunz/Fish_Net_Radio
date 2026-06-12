use crate::modem::modem_configuration::{BinBlock, ReceiverConfig};
use crate::modem::modem_rx_debug::{emit_debug, RxDebugEvent, RxDebugTx, RxStageId};
use crate::modem::modem_rx_types::{SpectrumFrame, TimeFrequencyImage};
use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use std::sync::Arc;

pub struct ImageBuilder {
    search_buffer_rows: usize,
    discard_bins: Vec<BinBlock>,
    rows: Vec<SpectrumFrame>,
    debug_tx: Option<RxDebugTx>,
    seq: u64,
}

impl ImageBuilder {
    pub fn new(config: ReceiverConfig, debug_tx: Option<RxDebugTx>) -> Self {
        ImageBuilder {
            search_buffer_rows: config.search_buffer_rows.max(1),
            discard_bins: config.discard_bins,
            rows: Vec::new(),
            debug_tx,
            seq: 0,
        }
    }

    fn zero_discarded_bins(&self, spectrum: &mut SpectrumFrame) {
        for block in &self.discard_bins {
            let start = block.start.min(spectrum.len());
            let end = block.end.min(spectrum.len().saturating_sub(1));
            if start > end {
                continue;
            }
            for bin in spectrum.iter_mut().take(end + 1).skip(start) {
                *bin = Default::default();
            }
        }
    }
}

impl StreamOperatorManagement for ImageBuilder {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        self.rows.clear();
        self.seq = 0;
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<Arc<SpectrumFrame>, Arc<TimeFrequencyImage>> for ImageBuilder {
    fn flush(&mut self) -> Result<Option<Vec<Arc<TimeFrequencyImage>>>, ErrorsJSL> {
        if self.rows.is_empty() {
            return Ok(None);
        }

        Ok(Some(vec![Arc::new(self.rows.clone())]))
    }

    fn process(
        &mut self,
        data_in: &[Arc<SpectrumFrame>],
    ) -> Result<Option<Vec<Arc<TimeFrequencyImage>>>, ErrorsJSL> {
        let mut spectrum = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        self.zero_discarded_bins(&mut spectrum);
        self.rows.push(spectrum);
        if self.rows.len() > self.search_buffer_rows {
            self.rows.remove(0);
        }
        self.seq += 1;

        let image = self.rows.clone();
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Search,
                seq: self.seq,
                name: "image_rows",
                value: image.len() as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Snapshot {
                stage: RxStageId::Search,
                seq: self.seq,
                label: "time_frequency_image",
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
    fn image_builder_rolls_rows_and_zeros_discard_bins() {
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut builder = ImageBuilder::new(test_receiver_config(), Some(debug_tx));
        let spectrum: SpectrumFrame = (0..8)
            .map(|idx| Complex::new(idx as f32, 0.0))
            .collect();

        let outputs = builder.process(&[Arc::new(spectrum)]).unwrap().unwrap();
        assert_eq!(outputs.len(), 1);
        let image = outputs[0].as_ref();
        assert_eq!(image.len(), 1);
        assert_eq!(image[0][2], Complex::default());
        assert_eq!(image[0][3], Complex::default());

        let mut saw_snapshot = false;
        while let Ok(event) = debug_rx.try_recv() {
            if matches!(event, RxDebugEvent::Snapshot { label: "time_frequency_image", .. }) {
                saw_snapshot = true;
            }
        }
        assert!(saw_snapshot);
    }
}
