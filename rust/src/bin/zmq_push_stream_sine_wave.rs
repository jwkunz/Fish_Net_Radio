use lib_jsl::dsp::discrete::stream_operator::StreamOperator;
use fish_net_radio::zmq_interface::zmq_push_sink::ZmqPushStreamSink;
use num_complex::Complex;
use std::f32::consts::PI;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;

fn main() {
    let mut args = std::env::args().skip(1);
    let address = args.next().unwrap_or_else(|| "127.0.0.1".to_string());
    let port = args.next().unwrap_or_else(|| "20002".to_string());
    let sample_rate_hz = 1_000_000.0_f32;
    let pdu_size = 1_000usize;

    println!(
        "Streaming a sine wave to tcp://{}:{} . Press Ctrl+C to stop.",
        address, port
    );

    let running = Arc::new(AtomicBool::new(true));
    let ctrlc_running = running.clone();
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, quitting...");
        ctrlc_running.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    let mut sink = ZmqPushStreamSink::new(&address, &port)
        .expect("Failed to connect the ZMQ PUSH sink");

    let mut phase = 0.0_f32;
    let advance = 2.0 * PI * 100_000.0 / sample_rate_hz;

    while running.load(Ordering::SeqCst) {
        let mut samples = Vec::<Complex<f32>>::with_capacity(pdu_size);
        for _ in 0..pdu_size {
            samples.push(Complex::from_polar(1.0, phase));
            phase += advance;
            if phase > 2.0 * PI {
                phase -= 2.0 * PI;
            }
        }

        if sink.process(&samples).is_err() {
            eprintln!("ZMQ send failed; is GNU Radio listening on tcp://{}:{}?", address, port);
            thread::sleep(std::time::Duration::from_millis(250));
        }
    }
}
