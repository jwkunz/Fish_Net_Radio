use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::{TimeFrequencyImage};

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

impl StreamOperator<TimeFrequencyImage, TimeFrequencyImage> for Acquisition {
    fn flush(&mut self) -> Result<Option<Vec<TimeFrequencyImage>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[TimeFrequencyImage]) -> Result<Option<Vec<TimeFrequencyImage>>, ErrorsJSL> {
        let image = data_in.first().cloned().unwrap_or_default();
        Ok(Some(vec![image]))
    }
}
