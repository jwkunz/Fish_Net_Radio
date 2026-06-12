use crate::modem::modem_configuration::ReceiverConfig;
use crate::modem::modem_rx_debug::{emit_debug, RxDebugEvent, RxDebugTx, RxStageId};
use crate::modem::modem_rx_types::{RawComplexFrame, SpectrumFrame};
use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

pub struct FftFrontEnd {
    fft_size: usize,
    fft: Arc<dyn Fft<f32> + Send + Sync>,
    debug_tx: Option<RxDebugTx>,
    seq: u64,
}

impl FftFrontEnd {
    pub fn new(config: ReceiverConfig, debug_tx: Option<RxDebugTx>) -> Self {
        let fft_size = config.fft_size;
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        FftFrontEnd {
            fft_size,
            fft,
            debug_tx,
            seq: 0,
        }
    }
}

impl StreamOperatorManagement for FftFrontEnd {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        self.seq = 0;
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<Arc<RawComplexFrame>, Arc<SpectrumFrame>> for FftFrontEnd {
    fn flush(&mut self) -> Result<Option<Vec<Arc<SpectrumFrame>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(
        &mut self,
        data_in: &[Arc<RawComplexFrame>],
    ) -> Result<Option<Vec<Arc<SpectrumFrame>>>, ErrorsJSL> {
        let input = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        self.seq += 1;

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::FrontEnd,
                seq: self.seq,
                name: "input_len",
                value: input.len() as f64,
            },
        );

        let mut buffer = vec![Complex::<f32>::default(); self.fft_size];
        let copy_len = input.len().min(self.fft_size);
        buffer[..copy_len].copy_from_slice(&input[..copy_len]);

        self.fft.process(&mut buffer);

        let (peak_bin, peak_mag) = buffer
            .iter()
            .enumerate()
            .map(|(idx, sample)| (idx, sample.norm_sqr()))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap_or((0, 0.0));

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::FrontEnd,
                seq: self.seq,
                name: "output_len",
                value: buffer.len() as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::FrontEnd,
                seq: self.seq,
                name: "peak_bin",
                value: peak_bin as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::FrontEnd,
                seq: self.seq,
                name: "peak_magnitude",
                value: peak_mag as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Snapshot {
                stage: RxStageId::FrontEnd,
                seq: self.seq,
                label: "spectrum",
                rows: 1,
                cols: buffer.len(),
            },
        );

        Ok(Some(vec![Arc::new(buffer)]))
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
    use std::f32::consts::TAU;
    use std::sync::mpsc;

    fn test_receiver_config(fft_size: usize) -> ReceiverConfig {
        ReceiverConfig {
            fft_size,
            fft_overlap_samples: 0,
            symbol_rows: 4,
            preamble_rows: 32,
            search_buffer_rows: 36,
            discard_bins: vec![BinBlock { start: 0, end: 0 }],
            nominal_rx_bins: NominalRxBins {
                low_block: RxBinBlock {
                    start: 0,
                    end: fft_size / 2,
                    step: 1,
                },
                high_block: RxBinBlock {
                    start: fft_size / 2,
                    end: fft_size - 1,
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
    fn fft_front_end_emits_spectrum_and_debug_metrics() {
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut fft = FftFrontEnd::new(test_receiver_config(8), Some(debug_tx));

        let tone_bin = 1usize;
        let input: RawComplexFrame = (0..8)
            .map(|n| {
                let phase = -TAU * tone_bin as f32 * n as f32 / 8.0;
                Complex::new(phase.cos(), phase.sin())
            })
            .collect();

        let outputs = fft.process(&[Arc::new(input)]).unwrap().unwrap();
        assert_eq!(outputs.len(), 1);

        let spectrum = outputs[0].as_ref();
        assert_eq!(spectrum.len(), 8);

        let (peak_bin, _) = spectrum
            .iter()
            .enumerate()
            .map(|(idx, sample)| (idx, sample.norm_sqr()))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        assert!(peak_bin == tone_bin || peak_bin == 8 - tone_bin);

        let mut saw_input_metric = false;
        let mut saw_peak_metric = false;
        let mut saw_snapshot = false;

        while let Ok(event) = debug_rx.try_recv() {
            match event {
                RxDebugEvent::Metric { name, .. } if name == "input_len" => saw_input_metric = true,
                RxDebugEvent::Metric { name, .. } if name == "peak_bin" => saw_peak_metric = true,
                RxDebugEvent::Snapshot { label, .. } if label == "spectrum" => saw_snapshot = true,
                _ => {}
            }
        }

        assert!(saw_input_metric);
        assert!(saw_peak_metric);
        assert!(saw_snapshot);
    }
}
