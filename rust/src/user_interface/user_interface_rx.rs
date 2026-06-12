use crate::modem::modem_rx::ModemRX;
use crate::modem::modem_rx_debug::RxDebugEvent;
use crate::modem::modem_configuration::{DebugLoggingLevel, ModemConfiguration};
use crate::modem::modem_rx_types::RxMessage;
use ctrlc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn rx_loop(config: ModemConfiguration, debug_level: DebugLoggingLevel) {
    println!("Starting receive mode (stub). Press Ctrl+C to exit.");

    let running = Arc::new(AtomicBool::new(true));
    let ctrlc_running = running.clone();
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, quitting...");
        ctrlc_running.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    let (message_tx, message_rx) = mpsc::channel::<RxMessage>();
    let (debug_tx, debug_rx) = mpsc::channel::<RxDebugEvent>();

    let modem_running = running.clone();
    let modem_thread = thread::spawn(move || {
        let modem = ModemRX::new(config);
        modem.run_with_debug(message_tx, modem_running, Some(debug_tx));
    });

    let debug_running = running.clone();
    let debug_thread = thread::spawn(move || {
        while debug_running.load(Ordering::SeqCst) {
            match debug_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(event) => {
                    if should_print_debug_event(debug_level, &event) {
                        print_debug_event(event);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            }
        }
        while let Ok(event) = debug_rx.try_recv() {
            if should_print_debug_event(debug_level, &event) {
                print_debug_event(event);
            }
        }
    });

    while running.load(Ordering::SeqCst) {
        match message_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(message) => print_message(message),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }

    let _ = modem_thread.join();
    let _ = debug_thread.join();
    println!("Receive mode terminated.");
}

fn print_message(message: RxMessage) {
    let timestamp = format_system_time(message.received_at);
    println!("[{}] {} => {}", timestamp, message.source_mac, message.payload);
}

fn print_debug_event(event: RxDebugEvent) {
    match event {
        RxDebugEvent::StageStart { stage, seq, detail } => {
            println!("[debug][{}][start][{}] {}", stage_name(stage), seq, detail);
        }
        RxDebugEvent::StageStop { stage, seq, detail } => {
            println!("[debug][{}][stop][{}] {}", stage_name(stage), seq, detail);
        }
        RxDebugEvent::Metric {
            stage,
            seq,
            name,
            value,
        } => {
            println!(
                "[debug][{}][metric][{}] {}={}",
                stage_name(stage),
                seq,
                name,
                value
            );
        }
        RxDebugEvent::Snapshot {
            stage,
            seq,
            label,
            rows,
            cols,
        } => {
            println!(
                "[debug][{}][snapshot][{}] {} rows={} cols={}",
                stage_name(stage),
                seq,
                label,
                rows,
                cols
            );
        }
        RxDebugEvent::Message { stage, seq, summary } => {
            println!("[debug][{}][message][{}] {}", stage_name(stage), seq, summary);
        }
        RxDebugEvent::Warning { stage, seq, detail } => {
            println!("[debug][{}][warn][{}] {}", stage_name(stage), seq, detail);
        }
        RxDebugEvent::Error { stage, seq, detail } => {
            println!("[debug][{}][error][{}] {}", stage_name(stage), seq, detail);
        }
    }
}

fn should_print_debug_event(level: DebugLoggingLevel, event: &RxDebugEvent) -> bool {
    match level {
        DebugLoggingLevel::Off => false,
        DebugLoggingLevel::Basic => matches!(
            event,
            RxDebugEvent::StageStart { .. }
                | RxDebugEvent::StageStop { .. }
                | RxDebugEvent::Message { .. }
                | RxDebugEvent::Warning { .. }
                | RxDebugEvent::Error { .. }
        ),
        DebugLoggingLevel::Verbose => true,
    }
}

fn stage_name(stage: crate::modem::modem_rx_debug::RxStageId) -> &'static str {
    use crate::modem::modem_rx_debug::RxStageId;

    match stage {
        RxStageId::Source => "source",
        RxStageId::FrontEnd => "fft",
        RxStageId::Search => "search",
        RxStageId::Acquisition => "acquisition",
        RxStageId::Cfar => "cfar",
        RxStageId::Tracker => "tracker",
        RxStageId::Demodulator => "demod",
        RxStageId::Parser => "parser",
    }
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
