use serde::Deserialize;

#[derive(Debug,Deserialize)]
pub struct ModemConfiguration{
    pub sample_rate : f64,
    pub gnuradio_instance_address_tx : String,
    pub gnuradio_instance_port_tx : String,
    pub modem_version : usize,
    pub samples_per_symbol : usize,
}