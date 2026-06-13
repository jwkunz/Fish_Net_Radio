use crate::modem::modem_configuration::{CrcConfig, PreambleConfig};
use crc::{Crc, CRC_32_ISO_HDLC};
use std::fmt;
use std::str::FromStr;

pub const LENGTH_FIELD_BYTES: usize = 2;
pub const LENGTH_REPETITIONS: usize = 3;
pub const REPEATED_LENGTH_HEADER_BYTES: usize = LENGTH_FIELD_BYTES * LENGTH_REPETITIONS;
pub const MAC_ADDRESS_BYTES: usize = 6;
pub const CRC_BYTES: usize = 4;

#[derive(Debug, Clone)]
pub struct MacAddress(pub [u8; 6]);

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

impl FromStr for MacAddress {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = input.split(':').collect();
        if parts.len() != 6 {
            return Err("MAC address must contain 6 octets separated by ':'".into());
        }

        let mut bytes = [0u8; 6];
        for (index, part) in parts.iter().enumerate() {
            bytes[index] =
                u8::from_str_radix(part, 16).map_err(|_| format!("Invalid MAC octet: {}", part))?;
        }

        Ok(MacAddress(bytes))
    }
}

#[derive(Debug)]
pub struct FrameBuilder {
    pub destination_mac: MacAddress,
    pub source_mac: MacAddress,
    pub crc_config: CrcConfig,
    pub preamble: PreambleConfig,
}

impl FrameBuilder {
    pub fn new(
        destination_mac: MacAddress,
        source_mac: MacAddress,
        crc_config: CrcConfig,
        preamble: PreambleConfig,
    ) -> Self {
        FrameBuilder {
            destination_mac,
            source_mac,
            crc_config,
            preamble,
        }
    }

    pub fn build_frame(&self, payload: &[u8]) -> Vec<u8> {
        let body_len =
            self.destination_mac.0.len() + self.source_mac.0.len() + payload.len() + CRC_BYTES;
        assert!(
            body_len <= u16::MAX as usize,
            "frame body length exceeds 16-bit length header"
        );

        let mut frame =
            Vec::with_capacity(self.preamble.bytes.len() + REPEATED_LENGTH_HEADER_BYTES + body_len);

        for byte in &self.preamble.bytes {
            frame.push(byte.as_bytes()[0]);
        }

        let length_bytes = (body_len as u16).to_le_bytes();
        for _ in 0..LENGTH_REPETITIONS {
            frame.extend_from_slice(&length_bytes);
        }

        let body_start = frame.len();
        frame.extend_from_slice(&self.destination_mac.0);
        frame.extend_from_slice(&self.source_mac.0);
        frame.extend_from_slice(payload);

        let crc = self.compute_crc(&frame[body_start..]);
        frame.extend_from_slice(&crc.to_le_bytes());

        frame
    }

    fn compute_crc(&self, data: &[u8]) -> u32 {
        // This implementation only supports the standard Ethernet CRC-32 polynomial.
        let alg = Crc::<u32>::new(&CRC_32_ISO_HDLC);
        let mut digest = alg.digest();
        digest.update(data);
        digest.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modem::modem_configuration::CrcConfig;
    use crate::modem::modem_configuration::PreambleConfig;

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

    #[test]
    fn frame_builder_generates_frame() {
        let builder = FrameBuilder::new(
            "FF:FF:FF:FF:FF:FF".parse().unwrap(),
            "00:11:22:33:44:55".parse().unwrap(),
            default_crc_config(),
            default_preamble(),
        );
        let payload = b"Hello";
        let frame = builder.build_frame(payload);
        let preamble_len = 8;
        let body_len = 6 + 6 + payload.len() + CRC_BYTES;

        assert!(frame.starts_with(b"FiShNeT:"));
        assert_eq!(
            frame[preamble_len..preamble_len + 2],
            (body_len as u16).to_le_bytes()
        );
        assert_eq!(
            frame[preamble_len + 2..preamble_len + 4],
            (body_len as u16).to_le_bytes()
        );
        assert_eq!(
            frame[preamble_len + 4..preamble_len + 6],
            (body_len as u16).to_le_bytes()
        );

        let body_start = preamble_len + REPEATED_LENGTH_HEADER_BYTES;
        assert_eq!(frame[body_start..body_start + 6], [0xFF; 6]);
        assert_eq!(
            frame[body_start + 6..body_start + 12],
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]
        );
        assert_eq!(
            frame.len(),
            preamble_len + REPEATED_LENGTH_HEADER_BYTES + body_len
        );
    }
}
