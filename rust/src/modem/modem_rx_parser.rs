use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use crate::modem::modem_rx_types::{RxMessage, SymbolStream};
use std::time::SystemTime;

pub struct FrameParser;

impl FrameParser {
    pub fn new() -> Self {
        FrameParser
    }
}

impl StreamOperatorManagement for FrameParser {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<SymbolStream, RxMessage> for FrameParser {
    fn flush(&mut self) -> Result<Option<Vec<RxMessage>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(&mut self, data_in: &[SymbolStream]) -> Result<Option<Vec<RxMessage>>, ErrorsJSL> {
        let payload = data_in
            .first()
            .map(|symbols| String::from_utf8_lossy(symbols).to_string())
            .unwrap_or_else(|| "<empty>".to_string());

        Ok(Some(vec![RxMessage {
            source_mac: "00:00:00:00:01:00".to_string(),
            payload,
            received_at: SystemTime::now(),
        }]))
    }
}
