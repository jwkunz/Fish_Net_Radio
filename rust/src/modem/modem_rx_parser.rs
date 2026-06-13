use crate::modem::modem_configuration::{
    FramingConfig, OutputConfig, PayloadConfig, PreambleConfig,
};
use crate::modem::modem_frame::{
    MacAddress, CRC_BYTES, MAC_ADDRESS_BYTES, REPEATED_LENGTH_HEADER_BYTES,
};
use crate::modem::modem_rx_debug::{emit_debug, RxDebugEvent, RxDebugTx, RxStageId};
use crate::modem::modem_rx_types::{FrameBytes, RxMessage};
use crc::{Crc, CRC_32_ISO_HDLC};
use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use std::sync::Arc;
use std::time::SystemTime;

pub struct FrameParser {
    framing: FramingConfig,
    payload: PayloadConfig,
    output: OutputConfig,
    preamble: PreambleConfig,
    debug_tx: Option<RxDebugTx>,
    seq: u64,
}

struct ParsedFrame {
    destination_mac: MacAddress,
    source_mac: MacAddress,
    payload: Vec<u8>,
    received_crc: u32,
    body_len: usize,
    consumed_bytes: usize,
    length_repaired: bool,
}

impl FrameParser {
    pub fn new(
        framing: FramingConfig,
        payload: PayloadConfig,
        output: OutputConfig,
        preamble: PreambleConfig,
        debug_tx: Option<RxDebugTx>,
    ) -> Self {
        FrameParser {
            framing,
            payload,
            output,
            preamble,
            debug_tx,
            seq: 0,
        }
    }

    fn preamble_bytes(&self) -> Vec<u8> {
        self.preamble
            .bytes
            .iter()
            .map(|byte| byte.as_bytes()[0])
            .collect()
    }

    fn parse_frame(&self, bytes: &[u8]) -> Result<ParsedFrame, String> {
        let preamble = self.preamble_bytes();
        let body_min_len = MAC_ADDRESS_BYTES * 2 + CRC_BYTES;
        let header_start = preamble.len();
        let body_start = header_start + REPEATED_LENGTH_HEADER_BYTES;
        let frame_min_len = body_start + body_min_len;
        if bytes.len() < frame_min_len {
            return Err(format!("frame too short: {}", bytes.len()));
        }
        if !bytes.starts_with(&preamble) {
            return Err("preamble mismatch".to_string());
        }

        let (body_len, length_repaired) = decode_repeated_length(&bytes[header_start..body_start])?;
        if body_len < body_min_len {
            return Err(format!("frame body too short: {}", body_len));
        }

        let consumed_bytes = body_start + body_len;
        if bytes.len() < consumed_bytes {
            return Err(format!(
                "incomplete frame body: expected={} available={}",
                body_len,
                bytes.len().saturating_sub(body_start)
            ));
        }

        let body = &bytes[body_start..consumed_bytes];
        let dest_start = 0;
        let src_start = dest_start + MAC_ADDRESS_BYTES;
        let payload_start = src_start + MAC_ADDRESS_BYTES;
        let crc_start = body.len().saturating_sub(CRC_BYTES);

        let destination_mac = MacAddress(body[dest_start..src_start].try_into().unwrap());
        let source_mac = MacAddress(body[src_start..payload_start].try_into().unwrap());
        let payload = body[payload_start..crc_start].to_vec();
        let received_crc = u32::from_le_bytes(body[crc_start..].try_into().unwrap());

        let alg = Crc::<u32>::new(&CRC_32_ISO_HDLC);
        let mut digest = alg.digest();
        digest.update(&body[dest_start..crc_start]);
        let computed_crc = digest.finalize();

        if received_crc != computed_crc {
            return Err(format!(
                "crc mismatch: received={:#010X} computed={:#010X}",
                received_crc, computed_crc
            ));
        }

        Ok(ParsedFrame {
            destination_mac,
            source_mac,
            payload,
            received_crc,
            body_len,
            consumed_bytes,
            length_repaired,
        })
    }

    fn destination_is_valid(&self, destination: &MacAddress) -> bool {
        let configured_destination = self.framing.mac.destination_mac.parse::<MacAddress>().ok();
        let broadcast = MacAddress([0xFF; 6]);

        if self.output.validate_destination_mac {
            if Some(destination.clone()) == configured_destination {
                return true;
            }
            if self.output.allow_broadcast || self.framing.mac.allow_broadcast {
                return *destination == broadcast;
            }
            return false;
        }

        true
    }

    fn decode_payload(&self, payload: &[u8]) -> String {
        match self.payload.encoding.to_lowercase().as_str() {
            "utf-8" | "utf8" => String::from_utf8(payload.to_vec())
                .unwrap_or_else(|_| String::from_utf8_lossy(payload).to_string()),
            _ => String::from_utf8_lossy(payload).to_string(),
        }
    }
}

impl PartialEq for MacAddress {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for MacAddress {}

fn decode_repeated_length(bytes: &[u8]) -> Result<(usize, bool), String> {
    if bytes.len() != REPEATED_LENGTH_HEADER_BYTES {
        return Err(format!("invalid repeated length header: {}", bytes.len()));
    }

    let first = u16::from_le_bytes(bytes[0..2].try_into().unwrap());
    let second = u16::from_le_bytes(bytes[2..4].try_into().unwrap());
    let third = u16::from_le_bytes(bytes[4..6].try_into().unwrap());

    let voted = if first == second || first == third {
        first
    } else if second == third {
        second
    } else {
        return Err(format!(
            "length header disagreement: {}, {}, {}",
            first, second, third
        ));
    };

    Ok((voted as usize, first != second || first != third))
}

impl StreamOperatorManagement for FrameParser {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        self.seq = 0;
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<Arc<FrameBytes>, RxMessage> for FrameParser {
    fn flush(&mut self) -> Result<Option<Vec<RxMessage>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(
        &mut self,
        data_in: &[Arc<FrameBytes>],
    ) -> Result<Option<Vec<RxMessage>>, ErrorsJSL> {
        let bytes = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        self.seq += 1;

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Parser,
                seq: self.seq,
                name: "input_bytes",
                value: bytes.len() as f64,
            },
        );

        let parsed = match self.parse_frame(&bytes) {
            Ok(parsed) => parsed,
            Err(detail) => {
                emit_debug(
                    &self.debug_tx,
                    RxDebugEvent::Error {
                        stage: RxStageId::Parser,
                        seq: self.seq,
                        detail,
                    },
                );
                return Ok(None);
            }
        };

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Parser,
                seq: self.seq,
                name: "body_len",
                value: parsed.body_len as f64,
            },
        );
        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Metric {
                stage: RxStageId::Parser,
                seq: self.seq,
                name: "length_header_repaired",
                value: if parsed.length_repaired { 1.0 } else { 0.0 },
            },
        );

        if parsed.consumed_bytes < bytes.len() {
            emit_debug(
                &self.debug_tx,
                RxDebugEvent::Warning {
                    stage: RxStageId::Parser,
                    seq: self.seq,
                    detail: format!(
                        "trimmed trailing demod bytes: {}",
                        bytes.len() - parsed.consumed_bytes
                    ),
                },
            );
        }

        if !self.destination_is_valid(&parsed.destination_mac) {
            emit_debug(
                &self.debug_tx,
                RxDebugEvent::Warning {
                    stage: RxStageId::Parser,
                    seq: self.seq,
                    detail: format!("destination {} rejected", parsed.destination_mac),
                },
            );
            return Ok(None);
        }

        let payload = self.decode_payload(&parsed.payload);
        let source_mac_text = if self.output.include_source_mac {
            parsed.source_mac.to_string()
        } else {
            String::new()
        };

        let message = RxMessage {
            source_mac: source_mac_text,
            payload: if self.output.deliver_payload {
                payload.clone()
            } else {
                String::new()
            },
            received_at: SystemTime::now(),
        };

        emit_debug(
            &self.debug_tx,
            RxDebugEvent::Message {
                stage: RxStageId::Parser,
                seq: self.seq,
                summary: format!(
                    "destination={} source={} payload_len={} crc={:#010X} consumed_bytes={}",
                    parsed.destination_mac,
                    parsed.source_mac,
                    parsed.payload.len(),
                    parsed.received_crc,
                    parsed.consumed_bytes
                ),
            },
        );

        Ok(Some(vec![message]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modem::modem_configuration::{
        BinBlock, CfarConfig, CrcConfig, DebugLoggingLevel, DopplerConfig, FramingConfig,
        MacConfig, ModemConfiguration, NominalRxBins, OutputConfig, PayloadConfig, PreambleConfig,
        ReceiverConfig, RxBinBlock, TrackingConfig,
    };
    use crate::modem::modem_frame::FrameBuilder;
    use crate::modem::modem_rx_debug::RxDebugEvent;
    use std::sync::mpsc;

    fn default_crc_config() -> CrcConfig {
        CrcConfig {
            r#type: "CRC-32".to_string(),
            polynomial: "0x04C11DB7".to_string(),
            init: "0xFFFFFFFF".to_string(),
            xor_out: "0xFFFFFFFF".to_string(),
            reflect_in: true,
            reflect_out: true,
            covers: vec![
                "destination_mac".to_string(),
                "source_mac".to_string(),
                "payload".to_string(),
            ],
        }
    }

    fn default_preamble() -> PreambleConfig {
        PreambleConfig {
            bytes: vec![
                "F".to_string(),
                "i".to_string(),
                "S".to_string(),
                "h".to_string(),
                "N".to_string(),
                "e".to_string(),
                "T".to_string(),
                ":".to_string(),
            ],
            length_bytes: 8,
        }
    }

    fn test_config() -> ModemConfiguration {
        ModemConfiguration {
            name: "test".to_string(),
            description: "test".to_string(),
            gnuradio_instance_address_tx: "127.0.0.1".to_string(),
            gnuradio_instance_port_tx: "20002".to_string(),
            gnuradio_instance_address_rx: "127.0.0.1".to_string(),
            gnuradio_instance_port_rx: "20001".to_string(),
            sample_rate_hz: 1_000_000.0,
            bits_per_symbol: 8,
            symbol_duration_samples: 512,
            raw_data_rate_bps: 15_625.0,
            spectral_efficiency: 0.0,
            transmitter: crate::modem::modem_configuration::TransmitterConfig {
                ifft_size: 512,
                symbol_samples: 512,
                idle_fill_samples: 16_384,
                output_format: "complex_f32".to_string(),
                valid_bins: crate::modem::modem_configuration::ValidBinBlocks {
                    low_block: crate::modem::modem_configuration::BinBlock { start: 8, end: 135 },
                    high_block: crate::modem::modem_configuration::BinBlock {
                        start: 376,
                        end: 503,
                    },
                },
                bin_mapping: crate::modem::modem_configuration::BinMapping {
                    scheme: "contiguous".to_string(),
                    low_symbol_range: crate::modem::modem_configuration::SymbolRange {
                        start: 0,
                        end: 127,
                    },
                    high_symbol_range: crate::modem::modem_configuration::SymbolRange {
                        start: 128,
                        end: 255,
                    },
                    description: "test".to_string(),
                },
                preamble: default_preamble(),
            },
            framing: FramingConfig {
                mac: MacConfig {
                    destination_length_bytes: 6,
                    source_length_bytes: 6,
                    allow_broadcast: true,
                    destination_mac: "FF:FF:FF:FF:FF:FF".to_string(),
                    source_mac: "00:11:22:33:44:55".to_string(),
                },
                crc: default_crc_config(),
            },
            receiver: ReceiverConfig {
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
            },
            payload: PayloadConfig {
                encoding: "utf-8".to_string(),
                interpret_as: "string".to_string(),
            },
            output: OutputConfig {
                deliver_payload: true,
                include_source_mac: true,
                validate_destination_mac: true,
                allow_broadcast: true,
            },
            notes: None,
        }
    }

    #[test]
    fn parser_validates_frame_and_emits_message() {
        let config = test_config();
        let builder = FrameBuilder::new(
            "FF:FF:FF:FF:FF:FF".parse().unwrap(),
            "00:11:22:33:44:55".parse().unwrap(),
            config.framing.crc.clone(),
            config.transmitter.preamble.clone(),
        );
        let frame = builder.build_frame(b"Hello");
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut parser = FrameParser::new(
            config.framing.clone(),
            config.payload.clone(),
            config.output.clone(),
            config.transmitter.preamble.clone(),
            Some(debug_tx),
        );

        let outputs = parser.process(&[Arc::new(frame)]).unwrap().unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].payload, "Hello");
        assert_eq!(outputs[0].source_mac, "00:11:22:33:44:55");

        let mut saw_message = false;
        while let Ok(event) = debug_rx.try_recv() {
            if let RxDebugEvent::Message { summary, .. } = event {
                if summary.contains("payload_len=5") {
                    saw_message = true;
                }
            }
        }

        assert!(saw_message);
    }

    #[test]
    fn parser_repairs_single_bad_length_copy() {
        let config = test_config();
        let builder = FrameBuilder::new(
            "FF:FF:FF:FF:FF:FF".parse().unwrap(),
            "00:11:22:33:44:55".parse().unwrap(),
            config.framing.crc.clone(),
            config.transmitter.preamble.clone(),
        );
        let mut frame = builder.build_frame(b"Hello");
        let first_length_byte = config.transmitter.preamble.bytes.len();
        frame[first_length_byte] ^= 0x01;
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut parser = FrameParser::new(
            config.framing.clone(),
            config.payload.clone(),
            config.output.clone(),
            config.transmitter.preamble.clone(),
            Some(debug_tx),
        );

        let outputs = parser.process(&[Arc::new(frame)]).unwrap().unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].payload, "Hello");

        let mut saw_repair = false;
        while let Ok(event) = debug_rx.try_recv() {
            if let RxDebugEvent::Metric { name, value, .. } = event {
                if name == "length_header_repaired" && value == 1.0 {
                    saw_repair = true;
                }
            }
        }

        assert!(saw_repair);
    }

    #[test]
    fn parser_trims_trailing_demod_bytes_using_length_header() {
        let config = test_config();
        let builder = FrameBuilder::new(
            "FF:FF:FF:FF:FF:FF".parse().unwrap(),
            "00:11:22:33:44:55".parse().unwrap(),
            config.framing.crc.clone(),
            config.transmitter.preamble.clone(),
        );
        let mut frame = builder.build_frame(b"Hello");
        frame.push(*frame.last().unwrap());
        let (debug_tx, debug_rx) = mpsc::channel();
        let mut parser = FrameParser::new(
            config.framing.clone(),
            config.payload.clone(),
            config.output.clone(),
            config.transmitter.preamble.clone(),
            Some(debug_tx),
        );

        let outputs = parser.process(&[Arc::new(frame)]).unwrap().unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].payload, "Hello");

        let mut saw_trim_warning = false;
        while let Ok(event) = debug_rx.try_recv() {
            if let RxDebugEvent::Warning { detail, .. } = event {
                if detail.contains("trimmed trailing demod bytes: 1") {
                    saw_trim_warning = true;
                }
            }
        }

        assert!(saw_trim_warning);
    }
}
