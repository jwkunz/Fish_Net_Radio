use std::sync::{mpsc, Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, SystemTime};

#[derive(Debug)]
pub struct RxMessage {
    pub source_mac: String,
    pub payload: String,
    pub received_at: SystemTime,
}

pub struct ModemRX;

impl ModemRX {
    pub fn new() -> Self {
        ModemRX
    }

    pub fn run(
        self,
        raw_rx: mpsc::Receiver<Vec<u8>>,
        message_tx: mpsc::Sender<RxMessage>,
        running: Arc<AtomicBool>,
    ) {
        while running.load(Ordering::SeqCst) {
            match raw_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(raw_packet) => {
                    if let Some(message) = self.process_raw_packet(raw_packet) {
                        let _ = message_tx.send(message);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            }
        }
    }

    fn process_raw_packet(&self, raw_packet: Vec<u8>) -> Option<RxMessage> {
        if raw_packet.is_empty() {
            return None;
        }

        let payload = String::from_utf8_lossy(&raw_packet).to_string();
        Some(RxMessage {
            source_mac: "00:00:00:00:01:00".to_string(),
            payload,
            received_at: SystemTime::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc, atomic::AtomicBool};
    use std::thread;

    #[test]
    fn modem_rx_processes_raw_packet() {
        let modem_rx = ModemRX::new();
        let message = modem_rx.process_raw_packet(b"hello".to_vec()).unwrap();
        assert_eq!(message.payload, "hello");
        assert_eq!(message.source_mac, "00:00:00:00:01:00");
    }

    #[test]
    fn modem_rx_thread_runs_and_receives_message() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (message_tx, message_rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let modem = ModemRX::new();
        let handle = thread::spawn(move || {
            modem.run(raw_rx, message_tx, running_clone);
        });

        raw_tx.send(b"test".to_vec()).unwrap();
        let message = message_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(message.payload, "test");
        running.store(false, Ordering::SeqCst);
        handle.join().unwrap();
    }
}
