use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::{SpectrumFrame, TimeFrequencyImage};
use std::sync::Arc;

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

impl StreamOperator<Arc<SpectrumFrame>, Arc<TimeFrequencyImage>> for ImageBuilder {
    fn flush(&mut self) -> Result<Option<Vec<Arc<TimeFrequencyImage>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[Arc<SpectrumFrame>]) -> Result<Option<Vec<Arc<TimeFrequencyImage>>>, ErrorsJSL> {
        let spectrum = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        let image: TimeFrequencyImage = vec![spectrum];
        Ok(Some(vec![Arc::new(image)]))
    }
}
