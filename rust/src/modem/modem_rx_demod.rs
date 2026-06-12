use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::SymbolStream;
use std::sync::Arc;

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

impl StreamOperator<Arc<SymbolStream>, Arc<SymbolStream>> for Demodulator {
    fn flush(&mut self) -> Result<Option<Vec<Arc<SymbolStream>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[Arc<SymbolStream>]) -> Result<Option<Vec<Arc<SymbolStream>>>, ErrorsJSL> {
        let symbols = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        Ok(Some(vec![Arc::new(symbols)]))
    }
}
