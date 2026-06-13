use crate::modem::modem_configuration::{PreambleConfig, ReceiverConfig, RxBinBlock};
use crate::modem::modem_rx_debug::{emit_debug, RxDebugEvent, RxDebugTx, RxStageId};
use crate::modem::modem_rx_types::{SpectrumFrame, SymbolStream, TimeFrequencyImage};
use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use std::sync::Arc;

pub struct Tracker {
    low_block: RxBinBlock,
    high_block: RxBinBlock,
    valid_bins: Vec<usize>,
    symbol_rows: usize,
    preamble_bytes: Vec<u8>,
    energy_drop_threshold: f64,
    drop_rows_required: usize,
    debug_tx: Option<RxDebugTx>,
    seq: u64,
    row_seq: i64,
    state: TrackerState,
}

#[derive(Debug, Default)]
enum TrackerState {
    #[default]
    Searching,
    Tracking(TrackingState),
}

#[derive(Debug)]
struct TrackingState {
    bytes: SymbolStream,
    next_symbol_row: i64,
    reference_energy: f64,
    drop_rows: usize,
}

#[derive(Debug)]
struct PreambleCandidate {
    start_row: usize,
    matches: usize,
    bin_distance: usize,
    preview: SymbolStream,
}

impl Tracker {
    pub fn new(
        config: ReceiverConfig,
        preamble: PreambleConfig,
        debug_tx: Option<RxDebugTx>,
    ) -> Self {
        let low_block = config.nominal_rx_bins.low_block;
        let high_block = config.nominal_rx_bins.high_block;
        let valid_bins = Self::valid_bins_for(&low_block, &high_block);
        let preamble_bytes = preamble
            .bytes
            .iter()
            .filter_map(|byte| byte.as_bytes().first().copied())
            .collect();

        Tracker {
            low_block,
            high_block,
            valid_bins,
            symbol_rows: config.symbol_rows.max(1),
            preamble_bytes,
            energy_drop_threshold: config.tracking.energy_drop_threshold,
            drop_rows_required: config.tracking.drop_rows_required.max(1),
            debug_tx,
            seq: 0,
            row_seq: 0,
            state: TrackerState::Searching,
        }
    }

    pub fn process_window(
        &mut self,
        image: Arc<TimeFrequencyImage>,
        detected: bool,
    ) -> Result<Option<Arc<SymbolStream>>, ErrorsJSL> {
        self.seq += 1;
        self.row_seq += 1;

        let latest_row_energy = image
            .last()
            .map(|row| self.row_valid_energy(row))
            .unwrap_or(0.0);

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Tracker,
                seq: self.seq,
                name: "latest_row_energy",
                value: latest_row_energy,
            },
        );

        let state = std::mem::take(&mut self.state);
        match state {
            TrackerState::Searching => {
                if detected {
                    self.try_start_tracking(&image)?;
                }
                Ok(None)
            }
            TrackerState::Tracking(mut tracking) => {
                if self.row_is_dropped(latest_row_energy, tracking.reference_energy) {
                    tracking.drop_rows += 1;
                } else {
                    tracking.drop_rows = 0;
                    tracking.reference_energy =
                        update_reference_energy(tracking.reference_energy, latest_row_energy);
                }

                emit_debug(
                    &self.debug_tx,
                    RxDebugEvent::Metric {
                        stage: RxStageId::Tracker,
                        seq: self.seq,
                        name: "drop_rows",
                        value: tracking.drop_rows as f64,
                    },
                );
                emit_debug(
                    &self.debug_tx,
                    RxDebugEvent::Metric {
                        stage: RxStageId::Tracker,
                        seq: self.seq,
                        name: "tracking_reference_energy",
                        value: tracking.reference_energy,
                    },
                );

                if tracking.drop_rows >= self.drop_rows_required {
                    let bytes = tracking.bytes;
                    emit_debug(
                        &self.debug_tx,
                        RxDebugEvent::Message {
                            stage: RxStageId::Tracker,
                            seq: self.seq,
                            summary: format!(
                                "tracking_complete symbols={} {}",
                                bytes.len(),
                                format_byte_preview(&bytes)
                            ),
                        },
                    );
                    emit_debug(
                        &self.debug_tx,
                        RxDebugEvent::Snapshot {
                            stage: RxStageId::Tracker,
                            seq: self.seq,
                            label: "tracked_symbol_stream",
                            rows: 1,
                            cols: bytes.len(),
                        },
                    );
                    self.state = TrackerState::Searching;
                    return Ok(Some(Arc::new(bytes)));
                }

                self.sample_due_symbols(&image, &mut tracking);
                emit_debug(
                    &self.debug_tx,
                    RxDebugEvent::Message {
                        stage: RxStageId::Tracker,
                        seq: self.seq,
                        summary: format!("tracking_symbols={}", tracking.bytes.len()),
                    },
                );
                self.state = TrackerState::Tracking(tracking);
                Ok(None)
            }
        }
    }

    fn try_start_tracking(&mut self, image: &TimeFrequencyImage) -> Result<(), ErrorsJSL> {
        let Some(candidate) = self.best_preamble_candidate(image) else {
            emit_debug(
                &self.debug_tx,
                RxDebugEvent::Warning {
                    stage: RxStageId::Tracker,
                    seq: self.seq,
                    detail: "detection did not contain enough rows for preamble search".to_string(),
                },
            );
            self.state = TrackerState::Searching;
            return Ok(());
        };

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Tracker,
                seq: self.seq,
                name: "preamble_matches",
                value: candidate.matches as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Tracker,
                seq: self.seq,
                name: "preamble_start_row",
                value: candidate.start_row as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Tracker,
                seq: self.seq,
                name: "preamble_bin_distance",
                value: candidate.bin_distance as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Message {
                stage: RxStageId::Tracker,
                seq: self.seq,
                summary: format!(
                    "preamble_candidate start_row={} matches={}/{} {}",
                    candidate.start_row,
                    candidate.matches,
                    self.preamble_bytes.len(),
                    format_byte_preview(&candidate.preview)
                ),
            },
        );

        if candidate.matches != self.preamble_bytes.len() {
            self.state = TrackerState::Searching;
            return Ok(());
        }

        let mut bytes = Vec::new();
        let mut symbol_energies = Vec::new();
        let mut row_idx = candidate.start_row;
        while row_idx < image.len() {
            let (byte, _, _) = self.map_row_to_symbol(&image[row_idx]);
            bytes.push(byte);
            symbol_energies.push(self.row_valid_energy(&image[row_idx]));
            row_idx += self.symbol_rows;
        }

        let last_symbol_idx =
            candidate.start_row + bytes.len().saturating_sub(1) * self.symbol_rows;
        let window_start_row = self.row_seq - image.len() as i64 + 1;
        let next_symbol_row = window_start_row + last_symbol_idx as i64 + self.symbol_rows as i64;
        let reference_energy = mean_positive_energy(&symbol_energies).unwrap_or(1.0);

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Message {
                stage: RxStageId::Tracker,
                seq: self.seq,
                summary: format!(
                    "tracking_started buffered_symbols={} next_symbol_row={} {}",
                    bytes.len(),
                    next_symbol_row,
                    format_byte_preview(&bytes)
                ),
            },
        );

        self.state = TrackerState::Tracking(TrackingState {
            bytes,
            next_symbol_row,
            reference_energy,
            drop_rows: 0,
        });
        Ok(())
    }

    fn sample_due_symbols(&self, image: &TimeFrequencyImage, tracking: &mut TrackingState) {
        let latest_row = self.row_seq;
        let window_start_row = latest_row - image.len() as i64 + 1;

        while tracking.next_symbol_row <= latest_row {
            let image_idx = tracking.next_symbol_row - window_start_row;
            if image_idx >= 0 {
                if let Some(row) = image.get(image_idx as usize) {
                    let (byte, peak_bin, peak_energy) = self.map_row_to_symbol(row);
                    tracking.bytes.push(byte);
                    tracking.reference_energy = update_reference_energy(
                        tracking.reference_energy,
                        self.row_valid_energy(row),
                    );
                    emit_debug(
                        &self.debug_tx,
                        RxDebugEvent::Message {
                            stage: RxStageId::Tracker,
                            seq: self.seq,
                            summary: format!(
                                "sampled_symbol byte={} peak_bin={} peak_energy={:.3}",
                                byte, peak_bin, peak_energy
                            ),
                        },
                    );
                }
            }
            tracking.next_symbol_row += self.symbol_rows as i64;
        }
    }

    fn best_preamble_candidate(&self, image: &TimeFrequencyImage) -> Option<PreambleCandidate> {
        if self.preamble_bytes.is_empty() {
            return None;
        }

        let preamble_span_rows = self.preamble_bytes.len() * self.symbol_rows;
        if image.len() < preamble_span_rows {
            return None;
        }

        let max_start = image.len().saturating_sub(preamble_span_rows);
        let mut best = None;

        for start_row in 0..=max_start {
            let mut matches = 0usize;
            let mut bin_distance = 0usize;
            let mut preview = Vec::with_capacity(self.preamble_bytes.len());

            for (symbol_idx, expected_byte) in self.preamble_bytes.iter().enumerate() {
                let row_idx = start_row + symbol_idx * self.symbol_rows;
                let (byte, peak_bin, _) = self.map_row_to_symbol(&image[row_idx]);
                preview.push(byte);
                if byte == *expected_byte {
                    matches += 1;
                }
                bin_distance += self.expected_bin_distance(*expected_byte, peak_bin);
            }

            let candidate = PreambleCandidate {
                start_row,
                matches,
                bin_distance,
                preview,
            };

            let should_replace = best.as_ref().map_or(true, |current: &PreambleCandidate| {
                candidate.matches > current.matches
                    || (candidate.matches == current.matches
                        && candidate.bin_distance < current.bin_distance)
            });
            if should_replace {
                best = Some(candidate);
            }
        }

        best
    }

    fn row_is_dropped(&self, row_energy: f64, reference_energy: f64) -> bool {
        row_energy < reference_energy * self.energy_drop_threshold
    }

    fn row_valid_energy(&self, row: &SpectrumFrame) -> f64 {
        self.valid_bins
            .iter()
            .filter_map(|&bin| row.get(bin))
            .map(|sample| sample.norm_sqr() as f64)
            .sum()
    }

    fn map_row_to_symbol(&self, row: &SpectrumFrame) -> (u8, usize, f32) {
        let mut row_peak_bin = None;
        let mut row_peak_energy = f32::MIN;

        for &bin in &self.valid_bins {
            if let Some(sample) = row.get(bin) {
                let energy = sample.norm_sqr();
                if energy > row_peak_energy {
                    row_peak_energy = energy;
                    row_peak_bin = Some(bin);
                }
            }
        }

        let chosen_bin = row_peak_bin.unwrap_or(0);
        let peak_energy = row_peak_energy.max(0.0);
        (
            map_bin_to_symbol(chosen_bin, &self.low_block, &self.high_block),
            chosen_bin,
            peak_energy,
        )
    }

    fn expected_bin_distance(&self, symbol: u8, peak_bin: usize) -> usize {
        let symbol = symbol as usize;
        let expected_bin = if symbol <= 127 {
            self.low_block.start + symbol * self.low_block.step.max(1)
        } else {
            self.high_block.start + (symbol - 128) * self.high_block.step.max(1)
        };

        peak_bin.abs_diff(expected_bin)
    }

    fn valid_bins_for(low_block: &RxBinBlock, high_block: &RxBinBlock) -> Vec<usize> {
        let mut bins = Vec::new();
        Self::extend_bins(&mut bins, low_block);
        Self::extend_bins(&mut bins, high_block);
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
        self.row_seq = 0;
        self.state = TrackerState::Searching;
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

        let mut symbols = Vec::new();
        let mut peak_bin = 0usize;
        let mut peak_energy = 0.0_f32;

        for row in &image {
            let (symbol, row_peak_bin, row_peak_energy) = self.map_row_to_symbol(row);
            peak_bin = row_peak_bin;
            peak_energy = row_peak_energy;
            symbols.push(symbol);
        }

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Tracker,
                seq: self.seq,
                name: "valid_bin_count",
                value: self.valid_bins.len() as f64,
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
        let offset = (bin - low_block.start + step / 2) / step;
        return offset.min(127) as u8;
    }

    if bin >= high_block.start && bin <= high_block.end {
        let step = high_block.step.max(1);
        let offset = (bin - high_block.start + step / 2) / step;
        return 128u8.saturating_add(offset.min(127) as u8);
    }

    0
}

fn update_reference_energy(reference_energy: f64, row_energy: f64) -> f64 {
    if row_energy <= 0.0 {
        reference_energy
    } else if reference_energy <= 0.0 {
        row_energy
    } else {
        reference_energy * 0.95 + row_energy * 0.05
    }
}

fn mean_positive_energy(values: &[f64]) -> Option<f64> {
    let positive: Vec<f64> = values
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
        .collect();
    if positive.is_empty() {
        None
    } else {
        Some(positive.iter().sum::<f64>() / positive.len() as f64)
    }
}

fn format_byte_preview(bytes: &[u8]) -> String {
    let hex = bytes
        .iter()
        .take(16)
        .map(|byte| format!("{:02X}", byte))
        .collect::<Vec<_>>()
        .join(" ");
    let ascii: String = bytes
        .iter()
        .take(16)
        .map(|byte| {
            if byte.is_ascii_graphic() || *byte == b' ' {
                *byte as char
            } else {
                '.'
            }
        })
        .collect();

    format!("hex=[{}] ascii=\"{}\"", hex, ascii)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modem::modem_configuration::{
        BinBlock, CfarConfig, DebugLoggingLevel, DopplerConfig, NominalRxBins, ReceiverConfig,
        TrackingConfig,
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

    fn tracking_receiver_config() -> ReceiverConfig {
        ReceiverConfig {
            fft_size: 256,
            fft_overlap_samples: 0,
            symbol_rows: 1,
            preamble_rows: 2,
            search_buffer_rows: 3,
            discard_bins: vec![],
            nominal_rx_bins: NominalRxBins {
                low_block: RxBinBlock {
                    start: 0,
                    end: 127,
                    step: 1,
                },
                high_block: RxBinBlock {
                    start: 128,
                    end: 255,
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

    fn test_preamble() -> PreambleConfig {
        PreambleConfig {
            bytes: vec!["F".to_string(), "i".to_string()],
            length_bytes: 2,
        }
    }

    fn tracking_preamble() -> PreambleConfig {
        PreambleConfig {
            bytes: vec!["A".to_string(), "B".to_string()],
            length_bytes: 2,
        }
    }

    fn row_for_symbol(symbol: u8) -> SpectrumFrame {
        let mut row = vec![Complex::new(0.0, 0.0); 256];
        row[symbol as usize] = Complex::new(4.0, 0.0);
        row
    }

    #[test]
    fn tracker_maps_peak_bins_to_symbols() {
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut tracker = Tracker::new(test_receiver_config(), test_preamble(), Some(debug_tx));
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

    #[test]
    fn tracker_emits_frame_candidate_after_quiet_gap() {
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut tracker = Tracker::new(
            tracking_receiver_config(),
            tracking_preamble(),
            Some(debug_tx),
        );

        let first = vec![
            row_for_symbol(b'A'),
            row_for_symbol(b'B'),
            row_for_symbol(b'C'),
        ];
        assert!(tracker
            .process_window(Arc::new(first), true)
            .unwrap()
            .is_none());

        let second = vec![
            row_for_symbol(b'B'),
            row_for_symbol(b'C'),
            row_for_symbol(b'D'),
        ];
        assert!(tracker
            .process_window(Arc::new(second), true)
            .unwrap()
            .is_none());

        let quiet = vec![
            row_for_symbol(b'C'),
            row_for_symbol(b'D'),
            vec![Complex::new(0.0, 0.0); 256],
        ];
        let output = tracker
            .process_window(Arc::new(quiet), false)
            .unwrap()
            .unwrap();

        assert_eq!(output.as_ref(), &vec![b'A', b'B', b'C', b'D']);

        let mut saw_start = false;
        let mut saw_complete = false;
        while let Ok(event) = debug_rx.try_recv() {
            if let RxDebugEvent::Message { summary, .. } = event {
                if summary.contains("tracking_started") {
                    saw_start = true;
                }
                if summary.contains("tracking_complete") {
                    saw_complete = true;
                }
            }
        }

        assert!(saw_start);
        assert!(saw_complete);
    }
}
