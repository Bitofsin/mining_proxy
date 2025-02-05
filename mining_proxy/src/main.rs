mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

use rustls_pemfile::{certs, rsa_private_keys};
use tokio_rustls::rustls::{self, Certificate, PrivateKey};

use std::{path::Path, sync::Arc, collections::VecDeque};
use tracing::Level;

use tokio::sync::{broadcast, RwLock, Mutex};

use tracing_subscriber::{
    self,
    fmt::{format::Writer, time::FormatTime},
};

use dotenv::dotenv;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::OpenOptions, io::Read};



use actix_web::{dev::ServiceRequest, web, App, Error, HttpServer};

use core::{
    client::{
        encry::accept_en_tcp, tcp::accept_tcp, tls::accept_tcp_with_tls, SSL,
        TCP,
    },
    proxy::Job,
    state::Worker,
    util::config::Settings,
    web::{handles::auth::Claims, AppState, OnlineWorker},
};

use anyhow::{bail, Result};

use clap::{crate_version, ArgMatches};
//use crossbeam_channel::bounded;
use human_panic::setup_panic;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    select,
    sync::mpsc::{self, UnboundedReceiver},
};

fn main() -> Result<()> {
    setup_panic!();
    dotenv().ok();

    if std::fs::metadata("./logs/").is_err() {
        std::fs::create_dir("./logs/")?;
    }

    struct LocalTimer;
    impl FormatTime for LocalTimer {
        fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
            write!(w, "{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
        }
    }

    let file_appender =
        tracing_appender::rolling::daily("./logs/", "mining_proxy");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Set the format of the log output, for example, whether to include the log level, whether to include the log source location,
    // Set the time format of the log Reference: https://docs.rs/tracing-subscriber/0.3.3/tracing_subscriber/fmt/struct.SubscriberBuilder.html#method.with_timer
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(false)
        .with_line_number(true)
        .with_source_location(true)
        .with_timer(LocalTimer);

    // Initialize and set log format (customize and filter logs)
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        //.with_writer(io::stdout) // write to standard output
        .with_writer(non_blocking) // Write to file, will overwrite standard output above
        .with_ansi(false) // If the log is written to a file, the color output function of ansi should be turned off
        .event_format(format)
        .init();

    core::init();
    let matches = core::util::get_app_command_matches()?;
    if !matches.is_present("server") {
        tracing::info!(
            "version: {} commit: {} {}",
            crate_version!(),
            version::commit_date(),
            version::short_sha(),
        );

        actix_web::rt::System::with_tokio_rt(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                //.worker_threads(1)
                .thread_name("main-tokio")
                .build()
                .unwrap()
        })
        .block_on(async_main(matches))?;
    } else {
        //tokio::runtime::start
        tokio_main(&matches)?;
    }
    Ok(())
}

async fn async_main(_matches: ArgMatches<'_>) -> Result<()> {
    let data: AppState = Arc::new(std::sync::Mutex::new(HashMap::new()));

    match OpenOptions::new()
        .write(true)
        .read(true)
        .open("configs.yaml")
    {
        Ok(mut f) => {
            let mut configs = String::new();
            if let Ok(len) = f.read_to_string(&mut configs) {
                if len > 0 {
                    let configs: Vec<Settings> =
                        match serde_yaml::from_str(&configs) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!("{}", e);
                                vec![]
                            }
                        };
                    for config in configs {
                        match core::util::run_server(&config) {
                            Ok(child) => {
                                let online = OnlineWorker {
                                    child,
                                    config: config.clone(),
                                    workers: vec![],
                                    online: 0,
                                };

                                data.lock()
                                    .unwrap()
                                    .insert(config.name, online);
                            }
                            Err(e) => {
                                tracing::error!("{}", e);
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {}
    };

    let tcp_data = data.clone();

    tokio::spawn(async move { recv_from_child(tcp_data).await });

    let port: i32 = match std::env::var("MINING_PROXY_WEB_PORT") {
        Ok(p) => p.parse().unwrap(),
        Err(_) => 8888,
    };

    let http_data = data.clone();
    let web_sever = if let Ok(http) = HttpServer::new(move || {
        let generated = generate();

        use actix_web_grants::GrantsMiddleware;
        let auth = GrantsMiddleware::with_extractor(extract);

        App::new()
            .wrap(auth)
            .app_data(web::Data::new(http_data.clone()))
            .service(
                web::scope("/api")
                    .service(core::web::handles::user::login)
                    .service(core::web::handles::user::info)
                    .service(core::web::handles::user::logout)
                    .service(core::web::handles::server::crate_app)
                    .service(core::web::handles::server::server_list)
                    .service(core::web::handles::server::server)
                    .service(core::web::handles::server::dashboard),
            )
            .service(actix_web_static_files::ResourceFiles::new("/", generated))
    })
    .bind(format!("0.0.0.0:{}", port))
    {
        http.run()
    } else {
        let mut proxy_server = data.lock().unwrap();
        for (_, other_server) in &mut *proxy_server {
            other_server.child.kill().await?;
        }
        bail!("web port {} is occupied", port);
    };

    tracing::info!("The interface starts successfully at the address: {}", format!("0.0.0.0:{}", port));
    web_sever.await?;
    Ok(())
}

fn tokio_main(matches: &ArgMatches<'_>) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { tokio_run(matches).await })?;
    Ok(())
}

async fn tokio_run(matches: &ArgMatches<'_>) -> Result<()> {
    let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
    let config = Settings::new(config_file_name, true)?;

    match config.check().await {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("config configuration error {}", err);
            std::process::exit(1);
        }
    };

    match config.check_net_work().await {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("Network Error {}", err);
        }
    };

    // let cert = match std::fs::File::open(config.pem_path.clone()).await {
    //     Ok(f) => {
    //         tracing::info!("Read custom certificate: {}", config.pem_path);
    //         // let cert = certs(&mut std::io::BufReader::new(f))
    //         // .map_err(|_| {
    //         //     // std::io::Error::new(
    //         //     //     std::io::ErrorKind::InvalidInput,
    //         //     //     "{} pem certificate failed, please use correct pem certificate!!!",
    //         //     // )
    //         //     bail!("{} pem certificate failed, please use correct pem certificate!!!")
    //         // })
    //         // .map(|mut certs| certs.drain(..).map(Certificate).collect();
    //         load_certs()

    //         Some(f)
    //     }
    //     Err(_) => {
    //         tracing::info!("Certificate not read: {}, use default certificate", config.pem_path);
    //         None
    //     }
    // };

    // let key = match File::open(config.key_path.clone()).await {
    //     Ok(f) => {
    //         tracing::info!("Read custom certificate: {}", config.key_path);

    //         Some(f)
    //     }
    //     Err(_) => {
    //         tracing::info!("Certificate not read: {}, use default certificate", config.key_path);
    //         None
    //     }
    // };

    let mode = if config.share == 0 {
        "Pure proxy mode"
    } else if config.share == 1 {
        "Pumping mode"
    } else {
        "Unified wallet mode"
    };

    tracing::info!("name {} The current startup mode is: {}", config.name, mode);

    let worker_name = config.share_name.clone();

    let (stream_type, _) = match core::client::get_pool_ip_and_type_from_vec(
        &config.share_address,
    ) {
        Ok((stream, addr)) => (stream, addr),
        Err(e) => {
            tracing::error!("Share_address mining pool parameter formatting failed. Unable to start {}", e);
            return Ok(());
        }
    };

    let certs = match load_certs(Path::new(&config.pem_path)) {
        Ok(cert) => {
            // tracing::info!(
            //     "Custom SSL certificate {} read successfully. use this certificate.",
            //     config.pem_path
            // );
            cert
        }
        Err(_) => {
            // tracing::error!(
            //     "Custom SSL certificate {} not found or malformed. Default certificate will be used",
            //     config.pem_path
            // );
            //panic!("Default certificate pem not found");

            tracing::info!(
                "Custom SSL certificate {} read failed. Please set the certificate. The program will exit if not set. .",
                config.pem_path
            );
            std::process::exit(1);
        }
    };

    let mut keys = match load_keys(Path::new(&config.key_path)) {
        Ok(key) => {
            // tracing::info!(
            //     "The custom key key {} was read successfully. Use this key.",
            //     config.key_path
            // );
            key
        }
        Err(_) => {
            // tracing::error!(
            //     "Custom key key {} not found or malformed. Default certificate will be used",
            //     config.key_path
            // );
            tracing::info!(
                "Failed to read custom key key {}. Please set the certificate. The program will exit if not set. .",
                config.key_path
            );
            std::process::exit(1);
            //panic!("Default Key not found");
            //let key_pem = include_bytes!("key.pem");
        }
    };

    let cert_config = match rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, keys.remove(0))
        .map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, err)
        }) {
        Ok(conf) => {
            // tracing::info!(
            //     "Custom SSL certificate {} is well-formed. Use this certificate as SSL certificate",
            //     config.pem_path
            // );
            conf
        }
        Err(e) => {
            tracing::info!("Certificate formatting failed. Please modify the certificate: {}", e);
            std::process::exit(1);
        }
    };

    //    if config.coin == "ETH" {
    // let (chan_tx, _chan_rx) = broadcast::channel::<Vec<String>>(1);
    // let (dev_chan_tx, _dev_chan_rx) = broadcast::channel::<Vec<String>>(1);
    let fee_job:Job = Arc::new(RwLock::new(VecDeque::new()));
    let develop_job:Job = Arc::new(RwLock::new(VecDeque::new()));
    
    let (tx, rx) = mpsc::channel::<Vec<String>>(15);
    let (dev_tx, dev_rx) = mpsc::channel::<Vec<String>>(15);
    // let (tx, rx) =
    //     bounded::<Vec<String>>(15);
    // let (dev_tx, dev_rx) =
    //     bounded::<Vec<String>>(15);
    tracing::debug!("Create a miner queue");
    // Absenteeism status sending queue
    let (worker_tx, worker_rx) = mpsc::unbounded_channel::<Worker>();

    let mconfig = config.clone();
    let proxy = Arc::new(core::proxy::Proxy {
        config: Arc::new(RwLock::new(config)),
        worker_tx,
//        chan: chan_tx.clone(),
        tx,
        dev_tx,
	fee_job:fee_job.clone(),
	develop_job:develop_job.clone(),
//        dev_chan: dev_chan_tx.clone(),
    });

    let (dev_lines, dev_w) =
        core::client::dev_pool_ssl_login(core::DEVELOP_WORKER_NAME.to_string())
            .await?;

    if stream_type == TCP {
        let (proxy_lines, proxy_w) =
            core::client::proxy_pool_login(&mconfig, worker_name.clone())
                .await?;
        let res = tokio::try_join!(
            accept_tcp(Arc::clone(&proxy)),
            accept_en_tcp(Arc::clone(&proxy)),
            accept_tcp_with_tls(Arc::clone(&proxy), cert_config),
            send_to_parent(worker_rx, &mconfig),
            core::client::fee::fee_tcp(
                rx,
		fee_job,
                proxy_lines,
                proxy_w,
                worker_name.clone(),
		proxy.clone(),
            ),
            core::client::fee::develop_fee_ssl(
                dev_rx,
		develop_job,
                dev_lines,
                dev_w,
                core::DEVELOP_WORKER_NAME.to_string(),
		proxy,
            ),
        );

        if let Err(err) = res {
            tracing::error!("fatal error: {}", err);
        }
    } else if stream_type == SSL {
        let (proxy_lines, proxy_w) = core::client::proxy_pool_login_with_ssl(
            &mconfig,
            worker_name.clone(),
        )
        .await?;

        let res = tokio::try_join!(
            accept_tcp(Arc::clone(&proxy)),
            accept_en_tcp(Arc::clone(&proxy)),
            accept_tcp_with_tls(Arc::clone(&proxy), cert_config),
            send_to_parent(worker_rx, &mconfig),
            core::client::fee::fee_ssl(
                rx,
		fee_job,
                proxy_lines,
                proxy_w,
                worker_name.clone(),
		proxy.clone(),
            ),
            core::client::fee::develop_fee_ssl(
                dev_rx,
		develop_job,
                dev_lines,
                dev_w,
                core::DEVELOP_WORKER_NAME.to_string(),
		proxy,
            ),
        );

        if let Err(err) = res {
            tracing::error!("fatal error: {}", err);
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendToParentStruct {
    name: String,
    worker: Worker,
}

async fn send_to_parent(
    mut worker_rx: UnboundedReceiver<Worker>, config: &Settings,
) -> Result<()> {
    loop {
        if let Ok(mut stream) =
            tokio::net::TcpStream::connect("127.0.0.1:65501").await
        {
            //let name = config.name.clone();
            loop {
                select! {
                    Some(w) = worker_rx.recv() => {
                        let send = SendToParentStruct{
                            name:config.name.clone(),
                            worker:w,
                        };
                        let mut rpc = serde_json::to_vec(&send)?;
                        rpc.push(b'\n');
                        stream.write(&rpc).await.unwrap();
                    },
                }
            }
        } else {
            tracing::error!("Unable to link to the master web");
            tokio::time::sleep(tokio::time::Duration::from_secs(60 * 2)).await;
        }
    }
}

async fn recv_from_child(app: AppState) -> Result<()> {
    let address = "127.0.0.1:65501";
    let listener = match tokio::net::TcpListener::bind(address.clone()).await {
        Ok(listener) => listener,
        Err(_) => {
            tracing::info!("Local port is occupied {}", address);
            std::process::exit(1);
        }
    };

    tracing::info!("Local TCP port {} started successfully!!!", &address);
    loop {
        let (mut stream, _) = listener.accept().await?;
        let inner_app = app.clone();

        tokio::spawn(async move {
            let (r, _) = stream.split();
            let r_buf = BufReader::new(r);
            let mut r_lines = r_buf.lines();

            loop {
                if let Ok(Some(buf_str)) = r_lines.next_line().await {
                    if let Ok(online_work) =
                        serde_json::from_str::<SendToParentStruct>(&buf_str)
                    {
                        #[cfg(debug_assertions)]
                        dbg!("{}", &online_work);

                        if let Some(temp_app) =
                            inner_app.lock().unwrap().get_mut(&online_work.name)
                        {
                            let mut is_update = false;
                            for worker in &mut temp_app.workers {
                                if worker.worker == online_work.worker.worker {
                                    //dbg!(&worker);
                                    *worker = online_work.worker.clone();
                                    is_update = true;
                                }
                            }
                            if !is_update {
                                temp_app.workers.push(online_work.worker);
                            }
                        } else {
                            tracing::error!("This port was not found");
                        }
                    }
                };
            }
        });
    }
}

use core::JWT_SECRET;

const ROLE_ADMIN: &str = "ROLE_ADMIN";
// You can use both &ServiceRequest and &mut ServiceRequest
async fn extract(req: &mut ServiceRequest) -> Result<Vec<String>, Error> {
    // Here is a place for your code to get user permissions/grants/permissions
    // from a request For example from a token or database
    // tracing::info!("check the Role");
    // println!("{:?}", req.headers().get("token"));

    if req.path() != "/api/user/login" {
        // Judgment authority
        if let Some(token) = req.headers().get("token") {
            let token_data = decode::<Claims>(
                token.to_str().unwrap(),
                &DecodingKey::from_secret(JWT_SECRET.as_bytes()),
                &Validation::default(),
            );
            if let Ok(_) = token_data {
                Ok(vec![ROLE_ADMIN.to_string()])
            } else {
                Ok(vec![])
            }
        } else {
            Ok(vec![])
        }
    } else {
        Ok(vec![])
    }
}

fn load_certs(path: &Path) -> std::io::Result<Vec<Certificate>> {
    certs(&mut std::io::BufReader::new(std::fs::File::open(path)?))
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid cert",
            )
        })
        .map(|mut certs| certs.drain(..).map(Certificate).collect())
}

fn load_keys(path: &Path) -> std::io::Result<Vec<PrivateKey>> {
    rsa_private_keys(&mut std::io::BufReader::new(std::fs::File::open(path)?))
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid key")
        })
        .map(|mut keys| keys.drain(..).map(PrivateKey).collect())
}

// async fn flux_transfer(mut inbound: TcpStream, proxy_addr: String) ->
// Result<()> {     let mut outbound =
// tokio::net::TcpStream::connect(proxy_addr).await?;

//     let (mut ri, mut wi) = inbound.split();
//     let (mut ro, mut wo) = outbound.split();

//     let client_to_server = async {
//         io::copy(&mut ri, &mut wo).await?;
//         wo.shutdown().await
//     };

//     let server_to_client = async {
//         io::copy(&mut ro, &mut wi).await?;
//         wi.shutdown().await
//     };
//     tokio::try_join!(client_to_server, server_to_client)?;

//     return Ok(());
// }
