use crate::modem::modem_rx_types::RawComplexFrame;
use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use num_complex::Complex;
use std::error::Error;
use std::sync::Arc;
use zmq::{Context, SocketType::PULL};

pub struct ZmqPullStreamSource {
    endpoint: String,
    context: Context,
    pull_socket: zmq::Socket,
}

impl ZmqPullStreamSource {
    pub fn new(address: &str, port: &str) -> Result<Self, Box<dyn Error>> {
        let context = Context::new();
        let pull_socket = context.socket(PULL)?;
        let endpoint = format!("tcp://{address}:{port}");
        pull_socket.connect(&endpoint)?;

        Ok(ZmqPullStreamSource {
            endpoint,
            context,
            pull_socket,
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn recv_frame(&mut self) -> Result<Option<Arc<RawComplexFrame>>, ErrorsJSL> {
        match self.pull_socket.recv_bytes(zmq::DONTWAIT) {
            Ok(bytes) => {
                let frame = decode_complex_frame(&bytes).map_err(|err| {
                    ErrorsJSL::RuntimeError(Box::leak(err.into_boxed_str()))
                })?;
                Ok(Some(Arc::new(frame)))
            }
            Err(err) if err == zmq::Error::EAGAIN => Ok(None),
            Err(_) => Err(ErrorsJSL::RuntimeError("bad socket receive")),
        }
    }
}

impl Drop for ZmqPullStreamSource {
    fn drop(&mut self) {
        let _ = self.pull_socket.disconnect(&self.endpoint);
        let _ = &self.context;
    }
}

impl StreamOperatorManagement for ZmqPullStreamSource {
    fn reset(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), ErrorsJSL> {
        Ok(())
    }
}

impl StreamOperator<(), Arc<RawComplexFrame>> for ZmqPullStreamSource {
    fn flush(&mut self) -> Result<Option<Vec<Arc<RawComplexFrame>>>, ErrorsJSL> {
        Ok(None)
    }

    fn process(
        &mut self,
        _: &[()],
    ) -> Result<Option<Vec<Arc<RawComplexFrame>>>, ErrorsJSL> {
        self.recv_frame().map(|frame| frame.map(|sample| vec![sample]))
    }
}

pub fn decode_complex_frame(bytes: &[u8]) -> Result<RawComplexFrame, String> {
    if bytes.len() % std::mem::size_of::<Complex<f32>>() != 0 {
        return Err(format!(
            "packed complex buffer length {} is not a multiple of {}",
            bytes.len(),
            std::mem::size_of::<Complex<f32>>()
        ));
    }

    let mut frame = Vec::with_capacity(bytes.len() / std::mem::size_of::<Complex<f32>>());
    for chunk in bytes.chunks_exact(std::mem::size_of::<Complex<f32>>()) {
        let re = f32::from_le_bytes(chunk[0..4].try_into().unwrap());
        let im = f32::from_le_bytes(chunk[4..8].try_into().unwrap());
        frame.push(Complex::new(re, im));
    }

    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_complex_frame_unpacks_little_endian_samples() {
        let bytes = [
            0.0f32.to_le_bytes(),
            1.5f32.to_le_bytes(),
            (-2.0f32).to_le_bytes(),
            3.25f32.to_le_bytes(),
        ]
        .concat();

        let frame = decode_complex_frame(&bytes).unwrap();
        assert_eq!(frame.len(), 2);
        assert_eq!(frame[0], Complex::new(0.0, 1.5));
        assert_eq!(frame[1], Complex::new(-2.0, 3.25));
    }
}
