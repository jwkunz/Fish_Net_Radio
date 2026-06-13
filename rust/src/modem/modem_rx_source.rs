use crate::modem::modem_rx_types::RawComplexFrame;
use crate::zmq_interface::zmq_pull_source::ZmqPullStreamSource;
use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use std::sync::Arc;

pub struct RxSource {
    inner: ZmqPullStreamSource,
}

impl RxSource {
    pub fn new(address: &str, port: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(RxSource {
            inner: ZmqPullStreamSource::new(address, port)?,
        })
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
        self.inner.process(&[])
    }
}
