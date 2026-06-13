use fish_net_radio::cli::{load_modem_configuration, run_tx};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let config_path = args
        .iter()
        .position(|arg| arg == "--config")
        .and_then(|index| args.get(index + 1).cloned());

    let modem_configuration = load_modem_configuration(config_path);
    run_tx(modem_configuration);
}
