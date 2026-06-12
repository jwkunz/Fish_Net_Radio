use crate::modem::modem_rx::ModemRX;
use crate::modem::modem_rx_types::RxMessage;
use ctrlc;
use std::sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn rx_loop() {
    println!("Starting receive mode (stub). Press Ctrl+C to exit.");

    let running = Arc::new(AtomicBool::new(true));
    let ctrlc_running = running.clone();
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, quitting...");
        ctrlc_running.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    let (message_tx, message_rx) = mpsc::channel::<RxMessage>();

    let modem_running = running.clone();
    let modem_thread = thread::spawn(move || {
        let modem = ModemRX::new();
        modem.run(message_tx, modem_running);
    });

    while running.load(Ordering::SeqCst) {
        match message_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(message) => print_message(message),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }

    let _ = modem_thread.join();
    println!("Receive mode terminated.");
}

fn print_message(message: RxMessage) {
    let timestamp = format_system_time(message.received_at);
    println!("[{}] {} => {}", timestamp, message.source_mac, message.payload);
}

fn format_system_time(time: SystemTime) -> String {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let secs = duration.as_secs();
            let millis = duration.subsec_millis();
            format!("{}.{}", secs, millis)
        }
        Err(_) => "invalid-time".to_string(),
    }
}
