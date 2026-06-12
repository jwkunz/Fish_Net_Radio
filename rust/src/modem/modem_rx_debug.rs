use std::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxStageId {
    Source,
    FrontEnd,
    Search,
    Acquisition,
    Cfar,
    Tracker,
    Demodulator,
    Parser,
}

#[derive(Debug, Clone)]
pub enum RxDebugEvent {
    StageStart {
        stage: RxStageId,
        seq: u64,
        detail: String,
    },
    StageStop {
        stage: RxStageId,
        seq: u64,
        detail: String,
    },
    Metric {
        stage: RxStageId,
        seq: u64,
        name: &'static str,
        value: f64,
    },
    Snapshot {
        stage: RxStageId,
        seq: u64,
        label: &'static str,
        rows: usize,
        cols: usize,
    },
    Message {
        stage: RxStageId,
        seq: u64,
        summary: String,
    },
    Warning {
        stage: RxStageId,
        seq: u64,
        detail: String,
    },
    Error {
        stage: RxStageId,
        seq: u64,
        detail: String,
    },
}

pub type RxDebugTx = mpsc::Sender<RxDebugEvent>;

pub fn emit_debug(debug_tx: &Option<RxDebugTx>, event: RxDebugEvent) {
    if let Some(tx) = debug_tx {
        let _ = tx.send(event);
    }
}
