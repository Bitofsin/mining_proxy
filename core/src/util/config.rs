use anyhow::{bail, Result};
use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::{env, net::TcpListener};

use crate::client::{SSL, TCP};

use super::get_develop_fee;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub coin: String,
    pub name: String,
    pub log_level: String,
    pub ssl_port: u32,
    pub tcp_port: u32,
    pub encrypt_port: u32,
    pub pool_address: Vec<String>,
    pub share_address: Vec<String>,
    pub share_wallet: String,
    pub share_name: String,
    pub share_rate: f32,
    pub hash_rate: u32,
    pub share: u32,
    pub share_alg: u32,
    pub pem_path: String,
    pub key_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            log_level: "DEBUG".into(),
            coin: "ETH".into(),
            share_wallet: "".into(),
            share_rate: 0.0,
            ssl_port: 8443,
            tcp_port: 14444,
            encrypt_port: 14444,
            pem_path: "./cert.pem".into(),
            key_path: "./key.pem".into(),
            share: 0,
            share_name: "".into(),
            name: "proxy".into(),
            share_alg: 0,
            hash_rate: 100,
            pool_address: Vec::new(),
            share_address: Vec::new(),
        }
    }
}

impl Settings {
    pub fn new(file_path: &str, with_file: bool) -> Result<Self, ConfigError> {
        let mut s = Config::default();

        if with_file {
            s.merge(File::with_name(file_path).required(false))?;
        }
        // Add in the current environment file
        // Default to 'development' env
        // Note that this file is _optional_
        // let env = env::var("RUN_MODE").unwrap_or_else(|_| "dev".into());
        // s.merge(File::with_name(&format!("config/{}",
        // env)).required(false))?;

        // Add in a local configuration file
        // This file shouldn't be checked in to git
        //s.merge(File::with_name("config/local").required(false))?;

        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        s.merge(Environment::with_prefix("PROXY"))?;
        if let Ok(address) = env::var("PROXY_POOL_ADDRESS") {
            let arr: Vec<&str> = address.split(',').collect();
            s.set("pool_address", arr)?;
        }

        if let Ok(address) = env::var("PROXY_SHARE_ADDRESS") {
            let arr: Vec<&str> = address.split(',').collect();
            s.set("share_address", arr)?;
        }

        // match env::var("PROXY_POOL_TCP_ADDRESS") {
        //     Ok(tcp_address) => {
        //         let arr: Vec<&str> = tcp_address.split(',').collect();
        //         s.set("pool_tcp_address", arr)?;
        //     }
        //     Err(_) => {}
        // }

        // match env::var("PROXY_POOL_SSL_ADDRESS") {
        //     Ok(tcp_address) => {
        //         let arr: Vec<&str> = tcp_address.split(',').collect();
        //         s.set("pool_ssl_address", arr)?;
        //     }
        //     Err(_) => {}
        // }

        // match env::var("PROXY_SHARE_TCP_ADDRESS") {
        //     Ok(tcp_address) => {
        //         let arr: Vec<&str> = tcp_address.split(',').collect();
        //         s.set("share_tcp_address", arr)?;
        //     }
        //     Err(_) => {}
        // }

        // match env::var("PROXY_SHARE_SSL_ADDRESS") {
        //     Ok(tcp_address) => {
        //         let arr: Vec<&str> = tcp_address.split(',').collect();
        //         s.set("share_ssl_address", arr)?;
        //     }
        //     Err(_) => {}
        // }
        s.try_into()
    }

    pub fn get_fee(&self) -> f64 {
        let develop_fee = get_develop_fee(self.share_rate.into(), true);

        let share_fee = self.share_rate;

        develop_fee + share_fee as f64
    }

    pub fn get_share_name(&self) -> Result<String> {
        let mut hostname = self.share_name.clone();
        if hostname.is_empty() {
            let name = hostname::get()?;
            if name.is_empty() {
                hostname = "proxy_wallet_mine".into();
            } else {
                hostname = hostname + name.to_str().unwrap();
            }
        }
        Ok(hostname)
    }

    pub async fn check(&self) -> Result<()> {
        if self.share_rate > 1.0 && self.share_rate < 0.001 {
            bail!("Incorrect pumping rate cannot be greater than 1. or less than 0.001")
        };

        if self.share_name.is_empty() {
            bail!("Pumping absentee name is not set")
        };

        if self.pool_address.is_empty() {
            bail!("The proxy pool address is empty")
        };

        if self.share_address.is_empty() {
            bail!("The proxy pool address of the pumping pool is empty")
        };

        match self.coin.as_str() {
            "ETH" => {}
            "ETC" => {}
            "CFX" => {}
            _ => {
                bail!("Unsupported proxy currency {}", self.coin)
            }
        }

        if self.tcp_port == 0 && self.ssl_port == 0 && self.encrypt_port == 0 {
            bail!("The local listening port must start one. All are currently 0")
        };

        if self.share != 0 && self.share_wallet.is_empty() {
            bail!("In pumping mode or unified wallet function, the receiving wallet cannot be empty.")
        }

        Ok(())
    }

    pub async fn check_net_work(&self) -> Result<()> {
        let (stream_type, pools) =
            match crate::client::get_pool_ip_and_type_from_vec(
                &self.share_address,
            ) {
                Ok(s) => s,
                Err(e) => {
                    bail!("{}", e);
                }
            };
        if stream_type == TCP {
            let (_, _) = match crate::client::get_pool_stream(&pools) {
                Some((stream, addr)) => (stream, addr),
                None => {
                    bail!("Unable to link to TCP proxy pool");
                }
            };
        } else if stream_type == SSL {
            let (_, _) =
                match crate::client::get_pool_stream_with_tls(&pools).await {
                    Some((stream, addr)) => (stream, addr),
                    None => {
                        bail!("Unable to link to SSL proxy pool");
                    }
                };
        }

        if self.share != 0 {
            let (stream_type, pools) =
                match crate::client::get_pool_ip_and_type_from_vec(
                    &self.share_address,
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        bail!("{}", e);
                    }
                };

            if stream_type == TCP {
                let (_, _) = match crate::client::get_pool_stream(&pools) {
                    Some((stream, addr)) => (stream, addr),
                    None => {
                        bail!("Unable to link to TCP pumping pool");
                    }
                };
            } else if stream_type == SSL {
                let (_, _) =
                    match crate::client::get_pool_stream_with_tls(&pools).await
                    {
                        Some((stream, addr)) => (stream, addr),
                        None => {
                            bail!("Unable to link to SSL pumping pool");
                        }
                    };
            }
        }

        //try listening on local port
        if self.tcp_port != 0 {
            let address = format!("0.0.0.0:{}", self.tcp_port);
            let _listener = match TcpListener::bind(address.clone()) {
                Ok(listener) => listener,
                Err(_) => {
                    bail!("TCP port occupied {}", self.tcp_port);
                }
            };
        }

        if self.ssl_port != 0 {
            let address = format!("0.0.0.0:{}", self.ssl_port);
            let _listener = match TcpListener::bind(address.clone()) {
                Ok(listener) => listener,
                Err(_) => {
                    bail!("SSL port occupied {}", self.ssl_port);
                }
            };
        }

        if self.encrypt_port != 0 {
            let address = format!("0.0.0.0:{}", self.encrypt_port);
            let _listener = match TcpListener::bind(address.clone()) {
                Ok(listener) => listener,
                Err(_) => {
                    bail!("Encrypted port is occupied {}", self.encrypt_port);
                }
            };

            Ok(())
        } else {
            Ok(())
        }
    }
}
