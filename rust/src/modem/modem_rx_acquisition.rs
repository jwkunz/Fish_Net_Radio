use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::{TimeFrequencyImage};
use std::sync::Arc;

pub struct Acquisition;

impl Acquisition {
    pub fn new() -> Self {
        Acquisition
    }
}

impl StreamOperatorManagement for Acquisition {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<Arc<TimeFrequencyImage>, Arc<TimeFrequencyImage>> for Acquisition {
    fn flush(&mut self) -> Result<Option<Vec<Arc<TimeFrequencyImage>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[Arc<TimeFrequencyImage>]) -> Result<Option<Vec<Arc<TimeFrequencyImage>>>, ErrorsJSL> {
        let image = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        Ok(Some(vec![Arc::new(image)]))
    }
}
