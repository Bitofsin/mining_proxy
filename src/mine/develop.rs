use std::sync::Arc;

use crate::{
    protocol::rpc::eth::{Client, ClientGetWork, Server, ServerId1},
    state::State,
    util::{calc_hash_rate, config::Settings},
    FEE,
};
use anyhow::Result;

use bytes::{BufMut, BytesMut};

use log::{debug, info};

use tokio::{
    io::{split, AsyncRead, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    sync::{
        broadcast,
        mpsc::{UnboundedReceiver, UnboundedSender},
        RwLock, RwLockReadGuard, RwLockWriteGuard,
    },
    time::sleep,
};

#[derive(Debug)]
pub struct Mine {
    config: Settings,
    hostname: String,
    wallet: String,
}

impl Mine {
    pub async fn new(config: Settings, wallet: String) -> Result<Self> {
        let name = hostname::get()?;
        let mut hostname = String::new();
        if name.is_empty() {
            hostname = "proxy_wallet_mine".into();
        } else {
            hostname = hostname + name.to_str().unwrap();
        }

        Ok(Self {
            config,
            hostname: hostname + "_dev",
            wallet: wallet,
        })
    }

    // pub async fn accept(&self, send: Sender<String>, mut recv: Receiver<String>) {
    //     if self.config.share == 1 {
    //         info!("✅✅ 开启TCP矿池抽水{}",self.config.share_tcp_address);
    //         self.accept_tcp(send, recv)
    //             .await
    //             .expect("❎❎ TCP 抽水线程启动失败");
    //     } else if self.config.share == 2 {
    //         info!("✅✅ 开启TLS矿池抽水{}",self.config.share_ssl_address);
    //         self.accept_tcp_with_tls(send, recv)
    //             .await
    //             .expect("❎❎ TLS 抽水线程启动失败");
    //     } else {
    //         info!("✅✅ 未开启抽水");
    //     }
    // }

    // async fn accept_tcp(&self, send: Sender<String>, mut recv: Receiver<String>) -> Result<()> {
    //     let mut outbound = TcpStream::connect(&self.config.share_tcp_address.to_string()).await?;
    //     let (mut r_server, mut w_server) = split(outbound);

    //     // { id: 40, method: "eth_submitWork", params: ["0x5fcef524222c218e", "0x5dc7070a672a9b432ec76075c1e06cccca9359d81dc42a02c7d80f90b7e7c20c", "0xde91884821ac90d583725a85d94c68468c0473f49a0907f45853578b9c617e0e"], worker: "P0001" }
    //     // { id: 6, method: "eth_submitHashrate", params: ["0x1dab657b", "a5f9ff21c5d98fbe3d08bf733e2ac47c0650d198bd812743684476d4d98cdf32"], worker: "P0001" }

    //     tokio::try_join!(
    //         self.login_and_getwork(send),
    //         self.client_to_server(w_server, recv),
    //         self.server_to_client(r_server)
    //     )?;
    //     Ok(())
    // }
    pub async fn accept(
        &self,
        state: Arc<RwLock<State>>,
        jobs_send: broadcast::Sender<String>,
        send: UnboundedSender<String>,
        recv: UnboundedReceiver<String>,
    ) -> Result<()> {
        if self.config.share != 0 {
            //info!("✅✅ 开启TCP矿池抽水");
            self.accept_tcp_with_tls(state, jobs_send.clone(), send, recv)
                .await
        } else {
            //info!("✅✅ 未开启抽水");
            Ok(())
        }
    }
    pub async fn accept_tcp_with_tls(
        &self,
        state: Arc<RwLock<State>>,
        jobs_send: broadcast::Sender<String>,
        send: UnboundedSender<String>,
        recv: UnboundedReceiver<String>,
    ) -> Result<()> {
        let pools = vec![
            "asia2.ethermine.org:5555".to_string(),
            "asia1.ethermine.org:5555".to_string(),
            "eu1.ethermine.org:5555".to_string(),
            "47.242.58.242:8089".to_string(),
        ];
        let (server_stream, _) =
            match crate::util::get_pool_stream_with_tls(&pools, "Develop".into()).await {
                Some((stream, addr)) => (stream, addr),
                None => {
                    #[cfg(debug_assertions)]
                    info!("所有SSL矿池均不可链接。请修改后重试");
                    std::process::exit(100);
                }
            };

        let (r_server, w_server) = split(server_stream);

        tokio::try_join!(
            self.login_and_getwork(state.clone(), jobs_send.clone(), send.clone()),
            self.client_to_server(
                state.clone(),
                jobs_send.clone(),
                send.clone(),
                w_server,
                recv
            ),
            self.server_to_client(state.clone(), jobs_send.clone(), send, r_server)
        )?;
        Ok(())
    }

    async fn server_to_client<R>(
        &self,
        state: Arc<RwLock<State>>,
        _: broadcast::Sender<String>,
        _: UnboundedSender<String>,
        mut r: ReadHalf<R>,
    ) -> Result<(), std::io::Error>
    where
        R: AsyncRead,
    {
        let mut is_login = false;
        let mut diff = "".to_string();

        loop {
            let mut buf = vec![0; 1024];
            let len = r.read(&mut buf).await.expect("从服务器读取失败.");
            if len == 0 {
                info!("❗❎ 服务端断开连接");
                #[cfg(debug_assertions)]
                debug!("❗❎ 服务端断开连接",);
                return Ok(());
                //return w_server.shutdown().await;
            }

            #[cfg(debug_assertions)]
            debug!(
                "-------- 矿池 to 开发者矿机 RPC #{:?}",
                String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
            );

            if !is_login {
                if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf[0..len]) {
                    if server_json_rpc.result == false {
                        info!("❗❎ 矿池登录失败，请尝试重启程序");
                        std::process::exit(18);
                    }

                    //info!("✅✅ 登录成功");
                    is_login = true;
                } else {
                    info!("❗❎ 矿池登录失败，请尝试重启程序");
                    #[cfg(debug_assertions)]
                    debug!(
                        "❗❎ 登录失败{:?}",
                        String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                    );
                    std::process::exit(19);
                }
            } else {
                if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf[0..len]) {
                    #[cfg(debug_assertions)]
                    debug!("收到抽水矿机返回 {:?}", server_json_rpc);
                    // if server_json_rpc.id == 6 {
                    //     //info!("🚜🚜 算力提交成功");
                    // } else if server_json_rpc.result {
                    //     info!("👍👍 Share Accept");
                    // } else {
                    //     info!("❗❗ Share Reject",);
                    // }
                } else if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
                    if let Some(job_diff) = server_json_rpc.result.get(3) {
                        #[cfg(debug_assertions)]
                        debug!("当前难度:{}", diff);
                        if diff != *job_diff {
                            //新的难度发现。

                            diff = job_diff.clone();
                            {
                                //清理队列。
                                #[cfg(debug_assertions)]
                                debug!("清理队列。:{}", diff);
                                let mut jobs = RwLockWriteGuard::map(state.write().await, |s| {
                                    &mut s.develop_jobs_queue
                                });
                                jobs.clear();
                            }
                        }
                    }

                    #[cfg(debug_assertions)]
                    debug!("Got jobs {:?}", server_json_rpc);
                    //新增一个share
                    if let Some(_) = server_json_rpc.result.get(0) {
                        //0 工作任务HASH
                        //1 DAG
                        //2 diff

                        // 判断是丢弃任务还是通知任务。

                        // 测试阶段全部通知

                        // 等矿机可以上线 由算力提交之后再处理这里。先启动一个Channel全部提交给矿机。
                        //debug!("发送到等待队列进行工作: {}", job_id);
                        // 判断以submitwork时jobs_id 是不是等于我们保存的任务。如果等于就发送回来给抽水矿机。让抽水矿机提交。
                        let job = serde_json::to_string(&server_json_rpc)?;
                        {
                            //
                            #[cfg(debug_assertions)]
                            debug!("将任务加入队列。{:?}", job);
                            let mut jobs = RwLockWriteGuard::map(state.write().await, |s| {
                                &mut s.develop_jobs_queue
                            });
                            jobs.push_back(job);
                        }

                        // let job = serde_json::to_string(&server_json_rpc)?;
                        // jobs_send.send(job);
                    }

                    // if let Some(diff) = server_json_rpc.result.get(3) {
                    //     //debug!("✅ Got Job Diff {}", diff);
                    // }
                } else {
                    #[cfg(debug_assertions)]
                    debug!(
                        "❗ ------未捕获封包:{:?}",
                        String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                    );
                }
            }
        }
    }

    async fn client_to_server<W>(
        &self,
        _: Arc<RwLock<State>>,
        _: broadcast::Sender<String>,
        _: UnboundedSender<String>,
        mut w: WriteHalf<W>,
        mut recv: UnboundedReceiver<String>,
    ) -> Result<(), std::io::Error>
    where
        W: AsyncWriteExt,
    {
        loop {
            let client_msg = recv.recv().await.expect("Channel Close");
            #[cfg(debug_assertions)]
            debug!("-------- 开发者矿机 to 矿池 RPC #{:?}", client_msg);
            if let Ok(mut client_json_rpc) = serde_json::from_slice::<Client>(client_msg.as_bytes())
            {
                if client_json_rpc.method == "eth_submitWork" {
                    //client_json_rpc.id = 40;
                    client_json_rpc.id = 599;
                    client_json_rpc.worker = self.hostname.clone();
                    // debug!(
                    //     "🚜🚜 抽水矿机 :{} Share #{:?}",
                    //     client_json_rpc.worker, client_json_rpc
                    // );
                    // info!(
                    //     "✅✅ 矿机 :{} Share #{:?}",
                    //     client_json_rpc.worker, client_json_rpc.id
                    // );
                } else if client_json_rpc.method == "eth_submitHashrate" {
                    // if let Some(hashrate) = client_json_rpc.params.get(0) {
                    //     debug!(
                    //         "✅✅ 矿机 :{} 提交本地算力 {}",
                    //         client_json_rpc.worker, hashrate
                    //     );
                    // }
                } else if client_json_rpc.method == "eth_submitLogin" {
                    //debug!("✅✅ 矿机 :{} 请求登录", client_json_rpc.worker);
                } else {
                    //debug!("矿机传递未知RPC :{:?}", client_json_rpc);
                }

                let rpc = serde_json::to_vec(&client_json_rpc)?;
                let mut byte = BytesMut::new();
                byte.put_slice(&rpc[0..rpc.len()]);
                byte.put_u8(b'\n');
                let w_len = w.write_buf(&mut byte).await?;
                if w_len == 0 {
                    return w.shutdown().await;
                }
            } else if let Ok(client_json_rpc) =
                serde_json::from_slice::<ClientGetWork>(client_msg.as_bytes())
            {
                let rpc = serde_json::to_vec(&client_json_rpc)?;
                let mut byte = BytesMut::new();
                byte.put_slice(&rpc[0..rpc.len()]);
                byte.put_u8(b'\n');
                let w_len = w.write_buf(&mut byte).await?;
                if w_len == 0 {
                    return w.shutdown().await;
                }
            }
        }
    }

    async fn login_and_getwork(
        &self,
        state: Arc<RwLock<State>>,
        _: broadcast::Sender<String>,
        send: UnboundedSender<String>,
    ) -> Result<(), std::io::Error> {
        let login = Client {
            id: 1,
            method: "eth_submitLogin".into(),
            params: vec![self.wallet.clone(), "x".into()],
            worker: self.hostname.clone(),
        };
        let login_msg = serde_json::to_string(&login)?;
        send.send(login_msg).unwrap();

        sleep(std::time::Duration::new(1, 0)).await;
        //RECV the work.Login package success or fail
        let eth_get_work = ClientGetWork {
            id: 5,
            method: "eth_getWork".into(),
            params: vec![],
        };

        let eth_get_work_msg = serde_json::to_string(&eth_get_work)?;
        send.send(eth_get_work_msg).unwrap();

        loop {
            let mut my_hash_rate: u64 = 0;
            {
                let workers = RwLockReadGuard::map(state.read().await, |s| &s.workers);
                for w in &*workers {
                    my_hash_rate = my_hash_rate + w.hash;
                }
            }

            //计算速率
            let submit_hashrate = Client {
                id: 6,
                method: "eth_submitHashrate".into(),
                params: [
                    format!("0x{:x}", calc_hash_rate(my_hash_rate, FEE),),
                    hex::encode(self.hostname.clone()),
                ]
                .to_vec(),
                worker: self.hostname.clone(),
            };

            let submit_hashrate_msg = serde_json::to_string(&submit_hashrate)?;
            #[cfg(debug_assertions)]
            debug!("开发者 提交本地算力{:?}", &submit_hashrate_msg);
            send.send(submit_hashrate_msg).unwrap();

            sleep(std::time::Duration::new(2, 0)).await;
            let eth_get_work_msg = serde_json::to_string(&eth_get_work)?;
            #[cfg(debug_assertions)]
            debug!("开发者 发送获取工作任务{:?}", &eth_get_work_msg);
            send.send(eth_get_work_msg).unwrap();

            if my_hash_rate <= 1000 {
                sleep(std::time::Duration::new(20, 0)).await;
            } else {
                sleep(std::time::Duration::new(10, 0)).await;
            }
        }
    }
}
