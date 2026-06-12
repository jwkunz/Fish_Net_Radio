use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::{TimeFrequencyImage, SymbolStream};

pub struct Tracker;

impl Tracker {
    pub fn new() -> Self {
        Tracker
    }
}

impl StreamOperatorManagement for Tracker {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<TimeFrequencyImage, SymbolStream> for Tracker {
    fn flush(&mut self) -> Result<Option<Vec<SymbolStream>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[TimeFrequencyImage]) -> Result<Option<Vec<SymbolStream>>, ErrorsJSL> {
        let _ = data_in.first();
        Ok(Some(vec![vec![0u8]]))
    }
}
