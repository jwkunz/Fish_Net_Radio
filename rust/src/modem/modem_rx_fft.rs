use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::{RawComplexFrame, SpectrumFrame};
use std::sync::Arc;

pub struct FftFrontEnd;

impl FftFrontEnd {
    pub fn new() -> Self {
        FftFrontEnd
    }
}

impl StreamOperatorManagement for FftFrontEnd {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<Arc<RawComplexFrame>, Arc<SpectrumFrame>> for FftFrontEnd {
    fn flush(&mut self) -> Result<Option<Vec<Arc<SpectrumFrame>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[Arc<RawComplexFrame>]) -> Result<Option<Vec<Arc<SpectrumFrame>>>, ErrorsJSL> {
        let frame = data_in.first().map(|a| (&**a).clone()).unwrap_or_default();
        Ok(Some(vec![Arc::new(frame)]))
    }
}
