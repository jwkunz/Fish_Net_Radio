use fish_net_radio::cli::load_modem_configuration;
use fish_net_radio::modem::modem_configuration::ModemConfiguration;
use fish_net_radio::zmq_interface::zmq_pull_source::decode_complex_frame;
use num_complex::Complex;
use std::error::Error;
use std::f32::consts::TAU;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;
use zmq::{Context, SocketType::PULL, SocketType::PUSH};

fn main() {
    let args = parse_args();
    let config = load_modem_configuration(args.config_path);

    println!(
        "Loopback relay binding TX pull on {}:{} and RX push on {}:{}",
        config.gnuradio_instance_address_tx,
        config.gnuradio_instance_port_tx,
        config.gnuradio_instance_address_rx,
        config.gnuradio_instance_port_rx
    );
    println!(
        "Noise voltage: {:.6}, doppler shift: {:.3} Hz",
        args.noise_voltage, args.doppler_hz
    );
    if args.verbose {
        println!("Verbose loopback diagnostics enabled.");
    }
    println!("Press Ctrl+C to stop.");

    let running = Arc::new(AtomicBool::new(true));
    let ctrlc_running = running.clone();
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, quitting...");
        ctrlc_running.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    if let Err(err) = run_loopback(
        config,
        args.noise_voltage,
        args.doppler_hz,
        args.verbose,
        running,
    ) {
        eprintln!("Loopback relay failed: {}", err);
    }
}

fn run_loopback(
    config: ModemConfiguration,
    noise_voltage: f32,
    doppler_hz: f32,
    verbose: bool,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    let context = Context::new();
    let pull_socket = context.socket(PULL)?;
    let push_socket = context.socket(PUSH)?;

    pull_socket.set_linger(0)?;
    push_socket.set_linger(0)?;

    let tx_endpoint = format!(
        "tcp://{}:{}",
        config.gnuradio_instance_address_tx, config.gnuradio_instance_port_tx
    );
    let rx_endpoint = format!(
        "tcp://{}:{}",
        config.gnuradio_instance_address_rx, config.gnuradio_instance_port_rx
    );

    pull_socket.bind(&tx_endpoint)?;
    push_socket.bind(&rx_endpoint)?;

    let mut processor = LoopbackProcessor::new(config.sample_rate_hz as f32, noise_voltage, doppler_hz);
    let mut pending: Option<Vec<u8>> = None;
    let mut receive_count = 0u64;
    let mut send_count = 0u64;

    while running.load(Ordering::SeqCst) {
        if let Some(frame) = pending.as_ref() {
            match push_socket.send(frame, zmq::DONTWAIT) {
                Ok(_) => {
                    send_count += 1;
                    if verbose {
                        println!(
                            "Loopback forwarded frame {}: samples={} bytes={}",
                            send_count,
                            frame.len() / std::mem::size_of::<Complex<f32>>(),
                            frame.len()
                        );
                    }
                    pending = None;
                }
                Err(err) if err == zmq::Error::EAGAIN => {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(err) => return Err(Box::new(err)),
            }
        }

        match pull_socket.recv_bytes(zmq::DONTWAIT) {
            Ok(bytes) => {
                receive_count += 1;
                let frame = decode_complex_frame(&bytes)
                    .map_err(|err| format!("failed to decode complex frame: {}", err))?;
                if verbose {
                    println!(
                        "Loopback received frame {}: samples={} bytes={}",
                        receive_count,
                        frame.len(),
                        bytes.len()
                    );
                }
                let processed = processor.process_frame(&frame);
                pending = Some(encode_complex_frame(&processed));
            }
            Err(err) if err == zmq::Error::EAGAIN => {
                thread::sleep(Duration::from_millis(2));
            }
            Err(err) => return Err(Box::new(err)),
        }
    }

    let _ = pull_socket.disconnect(&tx_endpoint);
    let _ = push_socket.disconnect(&rx_endpoint);
    Ok(())
}

fn encode_complex_frame(frame: &[Complex<f32>]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(frame.len() * std::mem::size_of::<Complex<f32>>());
    for sample in frame {
        bytes.extend_from_slice(&sample.re.to_le_bytes());
        bytes.extend_from_slice(&sample.im.to_le_bytes());
    }
    bytes
}

struct LoopbackProcessor {
    sample_rate_hz: f32,
    noise_voltage: f32,
    doppler_hz: f32,
    phase: f32,
    noise: XorShift64,
}

impl LoopbackProcessor {
    fn new(sample_rate_hz: f32, noise_voltage: f32, doppler_hz: f32) -> Self {
        LoopbackProcessor {
            sample_rate_hz: sample_rate_hz.max(1.0),
            noise_voltage: noise_voltage.max(0.0),
            doppler_hz,
            phase: 0.0,
            noise: XorShift64::new(0x1234_5678_9ABC_DEF0),
        }
    }

    fn process_frame(&mut self, frame: &[Complex<f32>]) -> Vec<Complex<f32>> {
        let phase_step = TAU * self.doppler_hz / self.sample_rate_hz;
        let mut phase = self.phase;
        let noise_sigma = self.noise_voltage / std::f32::consts::SQRT_2;

        let mut out = Vec::with_capacity(frame.len());
        for sample in frame {
            let shifted = if self.doppler_hz.abs() > f32::EPSILON {
                let rotation = Complex::from_polar(1.0, phase);
                phase += phase_step;
                if phase > TAU {
                    phase -= TAU;
                } else if phase < 0.0 {
                    phase += TAU;
                }
                *sample * rotation
            } else {
                *sample
            };

            if noise_sigma > 0.0 {
                let (n_re, n_im) = self.noise.gaussian_pair(noise_sigma);
                out.push(Complex::new(shifted.re + n_re, shifted.im + n_im));
            } else {
                out.push(shifted);
            }
        }

        self.phase = phase;
        out
    }
}

struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        let seed = if seed == 0 { 0xDEAD_BEEF_CAFE_BABE } else { seed };
        XorShift64 { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f32(&mut self) -> f32 {
        const SCALE: f32 = 1.0 / (u32::MAX as f32 + 1.0);
        let value = (self.next_u64() >> 32) as u32;
        (value as f32) * SCALE
    }

    fn gaussian_pair(&mut self, sigma: f32) -> (f32, f32) {
        let u1 = self.next_f32().clamp(1e-12, 1.0 - 1e-12);
        let u2 = self.next_f32().clamp(1e-12, 1.0 - 1e-12);
        let mag = (-2.0 * u1.ln()).sqrt() * sigma;
        let theta = TAU * u2;
        (mag * theta.cos(), mag * theta.sin())
    }
}

struct Args {
    config_path: Option<String>,
    noise_voltage: f32,
    doppler_hz: f32,
    verbose: bool,
}

fn parse_args() -> Args {
    let mut config_path = None;
    let mut noise_voltage = 0.0_f32;
    let mut doppler_hz = 0.0_f32;
    let mut verbose = false;

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--config" => {
                config_path = iter.next();
            }
            "--noise" => {
                noise_voltage = iter
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0.0);
            }
            "--doppler" => {
                doppler_hz = iter
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0.0);
            }
            "--verbose" => {
                verbose = true;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                eprintln!("Unrecognized argument: {}", other);
                print_help();
                std::process::exit(2);
            }
        }
    }

    Args {
        config_path,
        noise_voltage,
        doppler_hz,
        verbose,
    }
}

fn print_help() {
    println!(
        "Usage: zmq_loopback [--config PATH] [--noise VOLTS] [--doppler HZ] [--verbose]\n\
         Defaults: noise=0, doppler=0, config=rust/src/default_config.yaml"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_complex::Complex;

    #[test]
    fn zero_noise_and_zero_doppler_is_identity() {
        let mut processor = LoopbackProcessor::new(1_000_000.0, 0.0, 0.0);
        let input = vec![Complex::new(1.0, -2.0), Complex::new(0.5, 0.25)];
        let output = processor.process_frame(&input);
        assert_eq!(input, output);
    }

    #[test]
    fn doppler_rotates_samples() {
        let mut processor = LoopbackProcessor::new(1_000_000.0, 0.0, 100_000.0);
        let input = vec![Complex::new(1.0, 0.0); 4];
        let output = processor.process_frame(&input);
        assert_eq!(output.len(), 4);
        assert!((output[0].norm() - 1.0).abs() < 1e-6);
        assert!((output[1].norm() - 1.0).abs() < 1e-6);
    }
}
