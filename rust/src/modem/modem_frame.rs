use crate::modem::modem_configuration::{CrcConfig, PreambleConfig};
use crc::{Crc, CRC_32_ISO_HDLC};
use std::fmt;
use std::str::FromStr;

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
            bytes[index] = u8::from_str_radix(part, 16)
                .map_err(|_| format!("Invalid MAC octet: {}", part))?;
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
        let mut frame = Vec::with_capacity(
            self.preamble.bytes.len() + self.destination_mac.0.len() + self.source_mac.0.len() + payload.len() + 4,
        );

        for byte in &self.preamble.bytes {
            frame.push(byte.as_bytes()[0]);
        }
        frame.extend_from_slice(&self.destination_mac.0);
        frame.extend_from_slice(&self.source_mac.0);
        frame.extend_from_slice(payload);

        let crc = self.compute_crc(&frame[self.preamble.bytes.len()..]);
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
            covers: vec!["destination_mac".to_string(), "source_mac".to_string(), "payload".to_string()],
        }
    }

    fn default_preamble() -> PreambleConfig {
        PreambleConfig {
            bytes: vec!["F".to_string(), "i".to_string(), "S".to_string(), "h".to_string(), "N".to_string(), "e".to_string(), "T".to_string(), ":".to_string()],
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

        assert!(frame.starts_with(b"FiShNeT:"));
        assert_eq!(frame[8..14], [0xFF; 6]);
        assert_eq!(frame[14..20], [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(frame.len(), 8 + 6 + 6 + payload.len() + 4);
    }
}
