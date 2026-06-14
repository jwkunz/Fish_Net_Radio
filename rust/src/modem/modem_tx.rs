use crate::modem::modem_configuration::ModemConfiguration;
use crate::modem::modem_frame::{FrameBuilder, MacAddress};
use crate::modem::modem_waveform::WaveformGenerator;
use num_complex::Complex;
use std::str::FromStr;

pub struct ModemTX {
    pub config: ModemConfiguration,
    frame_builder: FrameBuilder,
    waveform_generator: WaveformGenerator,
}

impl ModemTX {
    pub fn new(config: ModemConfiguration) -> Result<Self, Box<dyn std::error::Error>> {
        let mac_config = &config.framing.mac;
        let destination_mac = MacAddress::from_str(&mac_config.destination_mac)
            .map_err(|err| format!("Invalid destination MAC: {}", err))?;
        let source_mac = MacAddress::from_str(&mac_config.source_mac)
            .map_err(|err| format!("Invalid source MAC: {}", err))?;

        let frame_builder = FrameBuilder::new(
            destination_mac,
            source_mac,
            config.transmitter.preamble.clone(),
        );

        let waveform_generator = WaveformGenerator::new(
            config.transmitter.ifft_size,
            (
                config.transmitter.valid_bins.low_block.start,
                config.transmitter.valid_bins.low_block.end,
            ),
            (
                config.transmitter.valid_bins.high_block.start,
                config.transmitter.valid_bins.high_block.end,
            ),
        );

        Ok(ModemTX {
            config,
            frame_builder,
            waveform_generator,
        })
    }

    pub fn create_packet(&mut self, payload: &[u8]) -> Vec<Complex<f32>> {
        let frame = self.frame_builder.build_frame(payload);
        self.waveform_generator.generate_waveform(&frame)
    }
}
