use std::sync::Arc;

use anyhow::Result;
use log::info;

use tokio::io::split;
use tokio::net::{TcpListener, TcpStream};

use futures::FutureExt;
use tokio::sync::broadcast;

use tokio::sync::{mpsc::UnboundedSender, RwLock};

use crate::client::{client_to_server, server_to_client};
use crate::protocol::rpc::eth::ServerId1;
use crate::state::State;
use crate::util::config::Settings;

pub async fn accept_tcp(
    state: Arc<RwLock<State>>,
    config: Settings,
    job_send: broadcast::Sender<String>,
    proxy_fee_sender: UnboundedSender<String>,
    develop_fee_sender: UnboundedSender<String>,
    state_send: UnboundedSender<String>,
    dev_state_send: UnboundedSender<String>,
) -> Result<()> {
    if config.pool_tcp_address.is_empty() {
        return Ok(());
    }
    if config.pool_tcp_address[0] == "" {
        return Ok(());
    }
    let address = format!("0.0.0.0:{}", config.tcp_port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("😄 Accepting Tcp On: {}", &address);

    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
        info!("😄 accept connection from {}", addr);
        let c = config.clone();
        let proxy_fee_sender = proxy_fee_sender.clone();
        let d = develop_fee_sender.clone();
        let state = state.clone();

        let jobs_recv = job_send.subscribe();
        let state_send = state_send.clone();
        let dev_state_send = dev_state_send.clone();
        tokio::spawn(async move {
            let transfer = transfer(
                state,
                jobs_recv,
                stream,
                c,
                proxy_fee_sender,
                d,
                state_send,
                dev_state_send,
            )
            .map(|r| {
                if let Err(e) = r {
                    info!("❎ 线程退出 : {}", e);
                }
            });

            info!("初始化完成");
            tokio::spawn(transfer);
        });
    }
}

async fn transfer(
    state: Arc<RwLock<State>>,
    jobs_recv: broadcast::Receiver<String>,
    inbound: TcpStream,
    config: Settings,
    proxy_fee_send: UnboundedSender<String>,
    fee: UnboundedSender<String>,
    state_send: UnboundedSender<String>,
    dev_state_send: UnboundedSender<String>,
) -> Result<()> {
    // let mut inbound = tokio_io_timeout::TimeoutStream::new(inbound);
    // inbound.set_read_timeout(Some(std::time::Duration::new(10,0)));
    // inbound.set_write_timeout(Some(std::time::Duration::new(10,0)));
    // tokio::pin!(inbound);

    let (stream, _) = match crate::util::get_pool_stream(&config.pool_tcp_address) {
        Some((stream, addr)) => (stream, addr),
        None => {
            info!("所有TCP矿池均不可链接。请修改后重试");
            return Ok(());
        }
    };
    let outbound = TcpStream::from_std(stream)?;

    let (r_client, w_client) = split(inbound);
    let (r_server, w_server) = split(outbound);

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<ServerId1>();
    let worker = Arc::new(RwLock::new(String::new()));
    let client_rpc_id = Arc::new(RwLock::new(0u64));
    info!("start client and server");
    let res = tokio::try_join!(
        client_to_server(
            state.clone(),
            worker.clone(),
            client_rpc_id.clone(),
            config.clone(),
            r_client,
            w_server,
            proxy_fee_send.clone(),
            //state_send.clone(),
            fee.clone(),
            tx.clone()
        ),
        server_to_client(
            state.clone(),
            worker,
            client_rpc_id,
            config.clone(),
            jobs_recv,
            r_server,
            w_client,
            proxy_fee_send.clone(),
            state_send.clone(),
            dev_state_send.clone(),
            rx
        )
    );

    if let Err(err) = res {
        info!("矿机错误或者代理池错误: {}", err);
    }
    
    Ok(())
}
