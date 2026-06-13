use crate::modem::modem_tx::ModemTX;
use crate::zmq_interface::zmq_push_sink::ZmqPushStreamSink;
use lib_jsl::dsp::discrete::stream_operator::StreamOperator;
use num_complex::Complex;
use std::io::{self, Read};
use std::os::unix::io::AsRawFd;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

const IDLE_FILL_INTERVAL: Duration = Duration::from_secs(1);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

pub fn tx_loop(modem: &mut ModemTX) {
    println!("Connecting ZMQ Socket to GNUradio front-end");
    let mut push_socket = ZmqPushStreamSink::new(
        &modem.config.gnuradio_instance_address_tx,
        &modem.config.gnuradio_instance_port_tx,
    )
    .expect("The ZMQ socket could not connect properly");

    println!("Type UTF-8 text and press ENTER to transmit. Press Ctrl+C to exit.");

    let running = Arc::new(AtomicBool::new(true));
    let ctrlc_running = running.clone();
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, quitting...");
        ctrlc_running.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    let _stdin_guard = NonBlockingStdin::new().expect("Failed to set stdin to nonblocking mode");
    let stdin = io::stdin();
    let mut stdin_handle = stdin.lock();
    let mut pending = String::new();
    let mut last_fill = Instant::now();
    let mut stdin_closed = false;

    while running.load(Ordering::SeqCst) {
        if !stdin_closed && pump_stdin(&mut stdin_handle, &mut pending) {
            stdin_closed = true;
        }

        while let Some(line) = next_line(&mut pending) {
            if !line.is_empty() {
                let bytes: Vec<u8> = line.bytes().collect();
                let waveform = modem.create_packet(&bytes);
                if let Err(err) = push_socket.process(&waveform) {
                    eprintln!("TX send failed: {:?}", err);
                } else {
                    println!("TX sent {} bytes", bytes.len());
                }
                last_fill = Instant::now();
            }
        }

        if stdin_closed && pending.is_empty() {
            running.store(false, Ordering::SeqCst);
            break;
        }

        if last_fill.elapsed() >= IDLE_FILL_INTERVAL {
            let n_fill = modem.config.transmitter.idle_fill_samples.max(1);
            let pad = vec![Complex::<f32>::from(0.0); n_fill];
            if let Err(err) = push_socket.process(&pad) {
                eprintln!("Idle fill send failed: {:?}", err);
            }
            last_fill = Instant::now();
        }

        thread::sleep(POLL_INTERVAL);
    }
}

fn pump_stdin(stdin: &mut dyn Read, pending: &mut String) -> bool {
    let mut buffer = [0u8; 1024];
    loop {
        match stdin.read(&mut buffer) {
            Ok(0) => return true,
            Ok(n) => pending.push_str(&String::from_utf8_lossy(&buffer[..n])),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => return false,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => {
                eprintln!("stdin read failed: {}", err);
                return true;
            }
        }
    }
}

fn next_line(pending: &mut String) -> Option<String> {
    if let Some(pos) = pending.find('\n') {
        let line = pending[..pos].trim_end_matches('\r').to_string();
        pending.drain(..=pos);
        Some(line)
    } else {
        None
    }
}

struct NonBlockingStdin {
    old_flags: i32,
}

impl NonBlockingStdin {
    fn new() -> io::Result<Self> {
        let fd = io::stdin().as_raw_fd();
        let old_flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if old_flags < 0 {
            return Err(io::Error::last_os_error());
        }

        let new_flags = old_flags | libc::O_NONBLOCK;
        if unsafe { libc::fcntl(fd, libc::F_SETFL, new_flags) } < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(NonBlockingStdin { old_flags })
    }
}

impl Drop for NonBlockingStdin {
    fn drop(&mut self) {
        let fd = io::stdin().as_raw_fd();
        let _ = unsafe { libc::fcntl(fd, libc::F_SETFL, self.old_flags) };
    }
}
