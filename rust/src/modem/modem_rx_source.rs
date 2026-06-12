use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use num_complex::Complex;
use std::sync::Arc;
use crate::modem::modem_rx_types::RawComplexFrame;

pub struct RxSource {
    counter: usize,
}

impl RxSource {
    pub fn new() -> Self {
        RxSource { counter: 0 }
    }
}

impl StreamOperatorManagement for RxSource {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<(), Arc<RawComplexFrame>> for RxSource {
    fn flush(&mut self) -> Result<Option<Vec<Arc<RawComplexFrame>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, _: &[()]) -> Result<Option<Vec<Arc<RawComplexFrame>>>, ErrorsJSL> {
        self.counter += 1;
        let frame_len = 512;
        let mut frame = Vec::with_capacity(frame_len);
        for i in 0..frame_len {
            let phase = (i as f32) * 2.0 * std::f32::consts::PI / frame_len as f32;
            frame.push(Complex::new((phase).cos(), (phase).sin()));
        }
        Ok(Some(vec![Arc::new(frame)]))
    }
}
