use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::{SpectrumFrame, TimeFrequencyImage};

pub struct ImageBuilder;

impl ImageBuilder {
    pub fn new() -> Self {
        ImageBuilder
    }
}

impl StreamOperatorManagement for ImageBuilder {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<SpectrumFrame, TimeFrequencyImage> for ImageBuilder {
    fn flush(&mut self) -> Result<Option<Vec<TimeFrequencyImage>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[SpectrumFrame]) -> Result<Option<Vec<TimeFrequencyImage>>, ErrorsJSL> {
        let spectrum = data_in.first().cloned().unwrap_or_default();
        Ok(Some(vec![vec![spectrum]]))
    }
}
