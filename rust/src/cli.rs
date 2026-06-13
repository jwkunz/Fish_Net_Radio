use crate::modem::modem_configuration::{DebugLoggingLevel, ModemConfiguration};
use crate::modem::modem_tx::ModemTX;
use crate::user_interface::{user_interface_rx, user_interface_tx};
use std::fs;

pub fn default_config_path() -> String {
    let mut default_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    default_path.push("src/default_config.yaml");
    default_path.to_string_lossy().to_string()
}

pub fn load_modem_configuration(path: Option<String>) -> ModemConfiguration {
    let config_file = path.unwrap_or_else(default_config_path);
    let contents = fs::read_to_string(&config_file)
        .unwrap_or_else(|_| panic!("The configuration file was not found: {}", config_file));
    serde_yaml::from_str(&contents).expect("The modem configuration did not parse correctly")
}

pub fn run_tx(config: ModemConfiguration) {
    if let Ok(mut modem) = ModemTX::new(config) {
        user_interface_tx::tx_loop(&mut modem);
    } else {
        panic!("Invalid modem configuration")
    }
}

pub fn run_rx(config: ModemConfiguration, debug_level: DebugLoggingLevel) {
    user_interface_rx::rx_loop(config, debug_level);
}
