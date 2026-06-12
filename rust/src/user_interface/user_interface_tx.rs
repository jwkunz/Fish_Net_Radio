use lib_jsl::dsp::discrete::stream_operator::StreamOperator;
use num_complex::Complex;

use crate::modem::modem_tx::ModemTX;
use crate::zmq_interface::zmq_push_sink::ZmqPushStreamSink;
use std::io::{self, BufRead};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::Duration;

const POLL_INTERVAL_SECONDS: u64 = 1;

pub fn tx_loop(modem: &mut ModemTX) {
    println!("Connecting ZMQ Socket to GNUradio front-end");
    let mut push_socket = ZmqPushStreamSink::new(
        &modem.config.gnuradio_instance_address_tx,
        &modem.config.gnuradio_instance_port_tx,
    )
    .expect("The ZMQ socket could not connect properly");

    println!(
        "Use this terminal to type UTF-8 text messages that you would like to send.\nTransmission will begin once you type ENTER.  Use ctrl+c to exit."
    );

    let running = Arc::new(AtomicBool::new(true));
    let ctrlc_running = running.clone();
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, quitting...");
        ctrlc_running.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    let (tx, rx) = mpsc::channel::<String>();
    let thread_running = running.clone();

    // Input thread (blocking, but isolated)
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut handle = stdin.lock();

        loop {
            if !thread_running.load(Ordering::SeqCst) {
                break;
            }

            let mut input = String::new();
            if handle.read_line(&mut input).is_ok() {
                let input = input.trim().to_string();
                if !input.is_empty() {
                    if tx.send(input).is_err() {
                        break;
                    }
                }
            } else if !thread_running.load(Ordering::SeqCst) {
                break;
            } else {
                break;
            }
        }
    });

    while running.load(Ordering::SeqCst) {
        // Wait up to 1 second for user input
        match rx.recv_timeout(Duration::from_secs(POLL_INTERVAL_SECONDS)) {
            Ok(input) => {
                let bytes: Vec<u8> = input.bytes().collect();
                let waveform = modem.create_packet(&bytes);
                push_socket
                    .process(&waveform)
                    .expect("An error occured during the push socket transmission");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                let n_fill = modem.config.sample_rate_hz.ceil() as usize;
                let pad = vec![Complex::<f32>::from(0.01); n_fill];
                push_socket
                    .process(&pad)
                    .expect("An error occured during the push socket transmission");
            }
            Err(_) => break, // channel disconnected
        }
    }
}
