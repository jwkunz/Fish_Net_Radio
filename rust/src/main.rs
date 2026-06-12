use std::fs;

use fish_net_radio::{
    modem::{
        modem_configuration::ModemConfiguration,
        modem_tx::ModemTX,
    },
    user_interface::*,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let config_file: String = if let Some(position) = args.iter().position(|x| x==&String::from("--config")){
        args[position+1].clone()
    } else {
        let mut default_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        default_path.push("src/default_config.yaml");
        default_path.to_string_lossy().to_string()
    };

    let modem_configuration : ModemConfiguration = serde_yaml::from_str(&fs::read_to_string(config_file).expect("The configuration file was not found")).expect("The modem configuration did not parse correctly");

    // DEBUG
    //args.push("--tx".into());

    if args.contains(&String::from("--tx")) {
        if let Ok(mut modem) = ModemTX::new(modem_configuration){
            user_interface_tx::tx_loop(&mut modem);
        }else{
            panic!("Invalid modem configuration")
        }
    }else if args.contains(&String::from("--rx")) {
        let debug_level = modem_configuration.receiver.debug_logging_level;
        user_interface_rx::rx_loop(modem_configuration, debug_level);
    }else{
        panic!("Must include either --tx or --rx");
    }
}
