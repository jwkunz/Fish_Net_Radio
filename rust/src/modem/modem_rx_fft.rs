use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::{RawComplexFrame, SpectrumFrame};

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

impl StreamOperator<RawComplexFrame, SpectrumFrame> for FftFrontEnd {
    fn flush(&mut self) -> Result<Option<Vec<SpectrumFrame>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[RawComplexFrame]) -> Result<Option<Vec<SpectrumFrame>>, ErrorsJSL> {
        let frame = data_in.first().cloned().unwrap_or_default();
        Ok(Some(vec![frame]))
    }
}
