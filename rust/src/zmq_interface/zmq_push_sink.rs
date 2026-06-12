use lib_jsl::dsp::{discrete::stream_operator::*, prelude::ErrorsJSL};
use num_complex::Complex;
use zmq::{Context, SocketType::PUSH};

pub struct ZmqPushStreamSink {
    endpoint: String,
    push_socket: zmq::Socket,
}

impl ZmqPushStreamSink {
    pub fn new(
        address: &str,
        port: &str
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // 1. Create a context
        let context = Context::new();
        // 2. Create a PUSH socket
        let push_socket = context.socket(PUSH)?;
        let endpoint: String = format!("tcp://{address}:{port}");
        // 3. Connect to a PULL receiver (e.g., GNU Radio ZMQ PULL Source)
        push_socket.connect(&endpoint)?;

        Ok(ZmqPushStreamSink {
            endpoint,
            push_socket,
        })
    }

    fn send_buffer(&mut self, buffer : &[Complex<f32>]) -> Result<(), Box<dyn std::error::Error>> {
        // SAFETY: Complex<f32> is guaranteed to be two packed f32s.
        // This works on Little Endian systems (standard for GNU Radio).
        let byte_slice = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr() as *const u8,
                buffer.len() * std::mem::size_of::<Complex<f32>>(),
            )
        };
        self.push_socket.send(byte_slice, 0)?;
        Ok(())
    }
}

impl StreamOperatorManagement for ZmqPushStreamSink {
    fn finalize(&mut self) -> Result<(), lib_jsl::prelude::ErrorsJSL> {
        if let Ok(_) = self.push_socket.disconnect(&self.endpoint) {
            Ok(())
        } else {
            Err(ErrorsJSL::RuntimeError(
                "Could not disconnect socket".into(),
            ))
        }
    }
    fn reset(&mut self) -> Result<(), lib_jsl::prelude::ErrorsJSL> {
        if let Ok(_) = self.push_socket.disconnect(&self.endpoint) {
            Ok(())
        } else {
            Err(ErrorsJSL::RuntimeError("Could not reconnect socket".into()))
        }
    }
}

impl StreamOperator<Complex<f32>, Complex<f32>> for ZmqPushStreamSink {
    fn flush(&mut self) -> Result<Option<Vec<Complex<f32>>>, lib_jsl::prelude::ErrorsJSL> {
        Ok(None)
    }

    fn process(
        &mut self,
        data_in: &[Complex<f32>],
    ) -> Result<Option<Vec<Complex<f32>>>, lib_jsl::prelude::ErrorsJSL> {
        if let Err(_) = self.send_buffer(&data_in){
            return Err(ErrorsJSL::RuntimeError("Bad socket send"));
        }
        Ok(None)
    }
}
