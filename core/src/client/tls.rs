use anyhow::Result;

use tokio_rustls::rustls::ServerConfig;
use tracing::info;

use tokio::{
    io::{split, BufReader},
    net::{TcpListener, TcpStream},
    sync::RwLockReadGuard,
};
//extern crate native_tls;
// use native_tls::Identity;
// use tokio::sync::mpsc::UnboundedSender;
use tokio_rustls::TlsAcceptor;

use super::*;
use crate::{proxy::Proxy, state::Worker, util::config::Settings};

pub async fn accept_tcp_with_tls(
    proxy: Arc<Proxy>, cert: ServerConfig,
) -> Result<()> {
    let config: Settings;
    {
        let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
        config = rconfig.clone();
    }

    if config.ssl_port == 0 {
        return Ok(());
    }

    let address = format!("0.0.0.0:{}", config.ssl_port);
    let listener = match TcpListener::bind(address.clone()).await {
        Ok(listener) => listener,
        Err(_) => {
            tracing::info!("Local port is occupied {}", address);
            std::process::exit(1);
        }
    };

    tracing::info!("Local SSL port {} started successfully!!!", &address);

    // let tls_acceptor = tokio_native_tls::TlsAcceptor::from(
    //     native_tls::TlsAcceptor::builder(cert).build()?,
    // );
    let tls_acceptor = TlsAcceptor::from(Arc::new(cert));

    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
        stream.set_nodelay(true)?;
        let acceptor = tls_acceptor.clone();

        let p = Arc::clone(&proxy);

        tokio::spawn(async move {
            // Miner Status Management
            let mut worker: Worker = Worker::default();
            let worker_tx = p.worker_tx.clone();
            match transfer_ssl(p, &mut worker, stream, acceptor).await {
                Ok(_) => {
                    if worker.is_online() {
                        worker.offline();
                        info!("IP: {} safe offline", addr);
                        worker_tx.send(worker).unwrap();
                    } else {
                        info!("IP: {} offline", addr);
                    }
                }
                Err(e) => {
                    if worker.is_online() {
                        worker.offline();
                        worker_tx.send(worker).unwrap();
                        info!("IP: {} Downtime Reason {}", addr, e);
                    } else {
                        debug!("IP: {} Malicious link broken: {}", addr, e);
                    }
                }
            }
        });
    }
}

async fn transfer_ssl(
    proxy: Arc<Proxy>, worker: &mut Worker, tcp_stream: TcpStream,
    tls_acceptor: TlsAcceptor,
) -> Result<()> {
    let client_stream = tls_acceptor.accept(tcp_stream).await?;
    let (worker_r, worker_w) = split(client_stream);
    let worker_r = BufReader::new(worker_r);
    let pool_address: Vec<String>;
    {
        let config = RwLockReadGuard::map(proxy.config.read().await, |s| s);
        pool_address = config.pool_address.to_vec();
    }

    let (stream_type, pools) =
        match crate::client::get_pool_ip_and_type_from_vec(&pool_address) {
            Ok(pool) => pool,
            Err(_) => {
                bail!("Not matched to a mining pool or neither can be linked. Please modify and try again");
            }
        };

    // if config.share == 0 {
    //     handle_tcp_pool(
    //         worker,
    //         worker_queue,
    //         worker_r,
    //         worker_w,
    //         &pools,
    //         &config,
    //         false,
    //     )
    //     .await
    // } else if config.share == 1 {
    //if config.share_alg == 99 {
    // handle_tcp_pool(
    //     worker,
    //     worker_queue,
    //     worker_r,
    //     worker_w,
    //     &pools,
    //     &config,
    //     false,
    // )
    // .await
    handle_tcp_random(
        worker,
        worker_r,
        worker_w,
        &pools,
        proxy,
        stream_type,
        false,
    )
    .await
    // } else {
    //     handle_tcp_pool_timer(
    //         worker,
    //         worker_queue,
    //         worker_r,
    //         worker_w,
    //         &pools,
    //         &config,
    //         false,
    //     )
    //     .await
    // }
    // } else {
    //     handle_tcp_pool_all(
    //         worker,
    //         worker_queue,
    //         worker_r,
    //         worker_w,
    //         &config,
    //         false,
    //     )
    //     .await
    // }
}
