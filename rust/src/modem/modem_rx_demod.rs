use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::SymbolStream;

pub struct Demodulator;

impl Demodulator {
    pub fn new() -> Self {
        Demodulator
    }
}

impl StreamOperatorManagement for Demodulator {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<SymbolStream, SymbolStream> for Demodulator {
    fn flush(&mut self) -> Result<Option<Vec<SymbolStream>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[SymbolStream]) -> Result<Option<Vec<SymbolStream>>, ErrorsJSL> {
        let symbols = data_in.first().cloned().unwrap_or_default();
        Ok(Some(vec![symbols]))
    }
}
