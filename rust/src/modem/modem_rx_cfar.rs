use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::TimeFrequencyImage;
use std::sync::Arc;

pub struct CfarDetector;

impl CfarDetector {
    pub fn new() -> Self {
        CfarDetector
    }
}

impl StreamOperatorManagement for CfarDetector {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<Arc<TimeFrequencyImage>, Arc<TimeFrequencyImage>> for CfarDetector {
    fn flush(&mut self) -> Result<Option<Vec<Arc<TimeFrequencyImage>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[Arc<TimeFrequencyImage>]) -> Result<Option<Vec<Arc<TimeFrequencyImage>>>, ErrorsJSL> {
        let image = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        Ok(Some(vec![Arc::new(image)]))
    }
}
