use clap::crate_version;
use tokio::time::Instant;
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
const SPLIT: u8 = b'\n';

lazy_static! {
    pub static ref JWT_SECRET: String = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| {
            "Generate : 0x60cc493FD745E268622274D877f1A50eD8368251".into()
        });
}

lazy_static! {
    pub static ref DEVELOP_WORKER_NAME: String = {
        let name = match hostname::get() {
            Ok(name) => {
                "develop_".to_string()
                    + name.to_str().expect("Unable to convert machine name to string")
            }
            Err(_) => crate_version!().to_string().replace(".", ""),
        };
        name
    };
}

lazy_static! {
    pub static ref DEVELOP_FEE: f64 = match std::env::var("DEVELOP_FEE") {
        Ok(fee) => {
            fee.parse().unwrap()
        }
        Err(_) => 0.02,
    };
}

lazy_static! {
    pub static ref RUNTIME: tokio::time::Instant = Instant::now();
}

pub fn init() {
    let a = RUNTIME.elapsed().as_secs();
    a.to_string();
    let name = &DEVELOP_WORKER_NAME;
    name.to_string();
    let jwt_secret = &JWT_SECRET;
    jwt_secret.to_string();
    let dev_fee = &DEVELOP_FEE;
    dev_fee.to_string();
}

pub mod client;
pub mod protocol;
pub mod proxy;
pub mod state;
pub mod util;
pub mod web;
