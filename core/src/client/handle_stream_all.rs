use std::io::Error;

use crate::protocol::{
    eth_stratum::EthSubscriptionNotify,
    ethjson::{
        login, new_eth_get_work, new_eth_submit_hashrate, new_eth_submit_login,
    },
    stratum::{StraumMiningSet, StraumResultBool},
};

use anyhow::{bail, Result};

use tracing::debug;

//use openssl::symm::{decrypt, Cipher};
extern crate rand;

use rand::{distributions::Alphanumeric, thread_rng, Rng};
use tokio::{
    io::{
        AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader, Lines, ReadHalf,
        WriteHalf,
    },
    net::TcpStream,
    select, time,
};

use crate::{
    client::*,
    protocol::{
        ethjson::{EthServer, EthServerRoot, EthServerRootObject},
        stratum::StraumResult,
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, CLIENT_SUBMITWORK,
        PROTOCOL, SUBSCRIBE,
    },
    state::Worker,
    util::{config::Settings, get_eth_wallet},
};

use super::write_to_socket;

async fn lines_unwrap<W>(
    _w: &mut WriteHalf<W>, res: Result<Option<String>, Error>,
    worker_name: &String, form_name: &str,
) -> Result<String>
where
    W: AsyncWrite,
{
    let buffer = match res {
        Ok(res) => match res {
            Some(buf) => Ok(buf),
            None => {
                bail!("{}：{} Active disconnect ", form_name, worker_name);
            }
        },
        Err(e) => {
            bail!("{}：{} mistake: {}", form_name, worker_name, e);
        }
    };

    buffer
}

// pub fn new_job_diff_change(
//     diff: &mut u64,
//     rpc: &EthServerRootObject,
//     a: &mut VecDeque<(String, Vec<String>)>,
//     b: &mut VecDeque<(String, Vec<String>)>,
//     c: &mut VecDeque<(String, Vec<String>)>,
// ) -> bool
// {
//     let job_diff = rpc.get_diff();
//     if job_diff == 0 {
//         return true;
//     }

//     if job_diff > *diff {
//         // write new difficulty
//         *diff = job_diff;
//         // Clear the existing task queue
//         a.clear();
//         b.clear();
//         c.clear();
//     }

//     true
// }

async fn develop_pool_login(
    hostname: String,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    let stream = match pools::get_develop_pool_stream().await {
        Ok(s) => s,
        Err(e) => {
            debug!("Unable to link to pool {}", e);
            return Err(e);
        }
    };

    let outbound = TcpStream::from_std(stream)?;

    let (develop_r, mut develop_w) = tokio::io::split(outbound);
    let develop_r = tokio::io::BufReader::new(develop_r);
    let develop_lines = develop_r.lines();

    let develop_name = hostname + "_develop";
    let login_develop = ClientWithWorkerName {
        id: CLIENT_LOGIN,
        method: "eth_submitLogin".into(),
        params: vec![get_eth_wallet(), "x".into()],
        worker: develop_name.to_string(),
    };

    write_to_socket(&mut develop_w, &login_develop, &develop_name).await?;

    Ok((develop_lines, develop_w))
}

async fn proxy_pool_login(
    config: &Settings, _hostname: String,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    //TODO Compatible with SSL mining pools here
    let (stream, _) =
        match crate::client::get_pool_stream(&config.share_address) {
            Some((stream, addr)) => (stream, addr),
            None => {
                tracing::error!("All TCP pools are unlinkable. Please modify and try again");
                bail!("All TCP pools are unlinkable. Please modify and try again");
            }
        };

    let outbound = TcpStream::from_std(stream)?;
    let (proxy_r, mut proxy_w) = tokio::io::split(outbound);
    let proxy_r = tokio::io::BufReader::new(proxy_r);
    let proxy_lines = proxy_r.lines();

    let s = config.get_share_name().unwrap();

    let login = ClientWithWorkerName {
        id: CLIENT_LOGIN,
        method: "eth_submitLogin".into(),
        params: vec![config.share_wallet.clone(), "x".into()],
        worker: s.clone(),
    };

    match write_to_socket(&mut proxy_w, &login, &s).await {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Error writing Socket {:?}", login);
            return Err(e);
        }
    }

    Ok((proxy_lines, proxy_w))
}

pub async fn pool_with_tcp_reconnect(
    config: &Settings,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    let (_stream_type, pools) =
        match crate::client::get_pool_ip_and_type(config) {
            Ok(pool) => pool,
            Err(_) => {
                bail!("Not matched to a mining pool or neither can be linked. Please modify and try again");
            }
        };
    // if stream_type == crate::client::TCP {
    let (outbound, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            bail!("All TCP pools are unlinkable. Please modify and try again");
        }
    };

    let stream = TcpStream::from_std(outbound)?;

    let (pool_r, pool_w) = tokio::io::split(stream);
    let pool_r = tokio::io::BufReader::new(pool_r);
    let pool_lines = pool_r.lines();
    Ok((pool_lines, pool_w))
    // } else if stream_type == crate::client::SSL {
    // let (stream, _) =
    //     match crate::client::get_pool_stream_with_tls(&pools,
    // "proxy".into()).await {         Some((stream, addr)) => (stream,
    // addr),         None => {
    //             bail!("All SSL pools are not linkable. Please modify and try again");
    //         }
    //     };

    // let (pool_r, pool_w) = tokio::io::split(stream);
    // let pool_r = tokio::io::BufReader::new(pool_r);

    // Ok((pool_r, pool_w))
    // } else {
    //     tracing::error!("Fatal error: No supported mining pool BUG found please report");
    //     bail!("Fatal error: No supported mining pool BUG found please report");
    // }
}

pub async fn pool_with_ssl_reconnect(
    config: &Settings,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    let (_stream_type, pools) =
        match crate::client::get_pool_ip_and_type(config) {
            Ok(pool) => pool,
            Err(_) => {
                bail!("Not matched to a mining pool or neither can be linked. Please modify and try again");
            }
        };
    let (outbound, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            bail!("All TCP pools are unlinkable. Please modify and try again");
        }
    };

    let stream = TcpStream::from_std(outbound)?;

    let (pool_r, pool_w) = tokio::io::split(stream);
    let pool_r = tokio::io::BufReader::new(pool_r);
    let pool_lines = pool_r.lines();
    Ok((pool_lines, pool_w))
}

pub async fn handle_stream<R, W>(
    worker: &mut Worker, workers_queue: UnboundedSender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    mut worker_w: WriteHalf<W>,
    pool_r: tokio::io::BufReader<tokio::io::ReadHalf<TcpStream>>,
    mut pool_w: WriteHalf<TcpStream>, config: &Settings, is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let _proxy_wallet_and_worker_name =
        config.share_wallet.clone() + "." + &config.share_name;
    let mut all_walllet_name = String::new();
    let mut protocol = PROTOCOL::KNOWN;
    let mut first = true;

    let mut worker_name: String = String::new();
    let mut eth_server_result = EthServerRoot {
        id: 0,
        jsonrpc: "2.0".into(),
        result: true,
    };

    let _stratum_result = StraumResult {
        id: 0,
        jsonrpc: "2.0".into(),
        result: vec![true],
    };

    let s = config.get_share_name().unwrap();
    let develop_name = s.clone() + "_develop";
    let rand_string = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .collect::<Vec<u8>>();

    let _proxy_eth_submit_hash = EthClientWorkerObject {
        id: CLIENT_SUBHASHRATE,
        method: "eth_submitHashrate".to_string(),
        params: vec!["0x0".into(), hexutil::to_hex(&rand_string)],
        worker: s.clone(),
    };

    let rand_string = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .collect::<Vec<u8>>();

    let _develop_eth_submit_hash = EthClientWorkerObject {
        id: CLIENT_SUBHASHRATE,
        method: "eth_submitHashrate".to_string(),
        params: vec!["0x0".into(), hexutil::to_hex(&rand_string)],
        worker: develop_name.to_string(),
    };

    // Pool The total number of packets sent to the miner.
    let _pool_job_idx: u64 = 0;

    let mut rpc_id = 0;
    //let mut pool_lines: MyStream;
    // Packaging is in packet format.
    // let mut worker_lines = worker_r.lines();
    let mut pool_lines = pool_r.lines();
    let mut worker_lines;

    if is_encrypted {
        worker_lines = worker_r.split(SPLIT);
    } else {
        worker_lines = worker_r.split(b'\n');
    }

    let _is_submithashrate = false;

    let sleep = time::sleep(tokio::time::Duration::from_secs(30));
    tokio::pin!(sleep);

    loop {
        select! {
            res = worker_lines.next_segment() => {
                //let start = std::time::Instant::now();
                let buf_bytes = seagment_unwrap(&mut pool_w,res,&worker_name).await?;

                // if is_encrypted {
                //     let key = Vec::from_hex(config.key.clone()).unwrap();
                //     let iv = Vec::from_hex(config.iv.clone()).unwrap();
                //     let cipher = Cipher::aes_256_cbc();

                //     buf_bytes = match base64::decode(&buf_bytes[..]) {
                //         Ok(buffer) => buffer,
                //         Err(e) => {
                //             tracing::error!("{}",e);
                //             match pool_w.shutdown().await  {
                //                 Ok(_) => {},
                //                 Err(_) => {
                //                     tracing::error!("Error Shutdown Socket {:?}",e);
                //                 },
                //             };
                //             bail!("Decryption miner request failed{}",e);
                //         },
                //     };

                //     buf_bytes = match decrypt(
                //         cipher,
                //         &key,
                //         Some(&iv),
                //         &buf_bytes[..]) {
                //             Ok(s) => s,
                //             Err(e) => {
                //                 tracing::warn!("Encrypted packet decryption failed");
                //                 match pool_w.shutdown().await  {
                //                     Ok(_) => {},
                //                     Err(e) => {
                //                         tracing::error!("Error Shutdown Socket {:?}",e);
                //                     },
                //                 };
                //                 bail!("Decryption miner request failed{}",e);
                //         },
                //     };
                // }


                let buf_bytes = buf_bytes.split(|c| *c == b'\n');
                for buffer in buf_bytes {
                    if buffer.is_empty() {
                        continue;
                    }

                    #[cfg(debug_assertions)]
                    debug!(">-------------------->  mining machine {} #{:?}",worker_name, String::from_utf8(buffer.to_vec())?);

                    if let Some(mut json_rpc) = parse(&buffer) {
                        if first {
                            first = false;
                            let _res = match json_rpc.get_method().as_str() {
                                "eth_submitLogin" => {
                                    protocol = PROTOCOL::ETH;
                                },
                                "mining.subscribe" => {
                                    if json_rpc.is_protocol_eth_statum() {
                                        protocol = PROTOCOL::NICEHASHSTRATUM;
                                    } else {
                                        protocol = PROTOCOL::STRATUM;
                                    }
                                },
                                _ => {
                                    tracing::warn!("Not found method {:?}",json_rpc);
                                    // eth_server_result.id = rpc_id;
                                    // write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                    // Ok(())
                                },
                            };
                        }


                        rpc_id = json_rpc.get_id();
                        if protocol == PROTOCOL::ETH {
                            let res = match json_rpc.get_method().as_str() {
                                "eth_submitLogin" => {
                                    eth_server_result.id = rpc_id;
                                    new_eth_submit_login(worker,&mut pool_w,&mut json_rpc,&mut worker_name,&config).await?;
                                    write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name).await?;
                                    Ok(())
                                },
                                "eth_submitWork" => {
                                    eth_server_result.id = rpc_id;
                                    worker.share_index_add();
                                    //new_eth_submit_work(worker,&mut pool_w,&mut worker_w,&mut json_rpc,&mut worker_name,&config,&mut state).await?;
                                    write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name).await?;
                                    Ok(())
                                },
                                "eth_submitHashrate" => {
                                    eth_server_result.id = rpc_id;
                                    new_eth_submit_hashrate(worker,&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                    write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name).await?;

                                    Ok(())
                                },
                                "eth_getWork" => {

                                    new_eth_get_work(&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                    //write_rpc(is_encrypted,&mut worker_w,eth_server_result,&worker_name).await?;
                                    Ok(())
                                },
                                _ => {
                                    tracing::warn!("Not found ETH method {:?}",json_rpc);
                                    eth_server_result.id = rpc_id;
                                    write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                    Ok(())
                                },
                            };

                            if res.is_err() {
                                tracing::warn!("Write task error: {:?}",res);
                                return res;
                            }
                        } else if protocol == PROTOCOL::STRATUM {

                            let res = match json_rpc.get_method().as_str() {
                                "mining.subscribe" => {
                                    all_walllet_name = login(worker,&mut pool_w,&mut json_rpc,&mut worker_name,&config).await?;
                                    Ok(())
                                },
                                "mining.submit" => {
                                    worker.share_index_add();
                                    json_rpc.set_wallet(&all_walllet_name);
                                    write_to_socket_byte(&mut pool_w, json_rpc.to_vec()?, &worker_name).await?;
                                    Ok(())
                                },
                                _ => {
                                    tracing::warn!("Not found ETH method {:?}",json_rpc);
                                    write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                    Ok(())
                                },
                            };

                            if res.is_err() {
                                tracing::warn!("write task error: {:?}",res);
                                return res;
                            }
                        } else if protocol ==  PROTOCOL::NICEHASHSTRATUM {
                            let res = match json_rpc.get_method().as_str() {
                                "mining.subscribe" => {
                                    //login(worker,&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                    write_to_socket_byte(&mut pool_w, buffer.to_vec(), &worker_name).await?;
                                    Ok(())
                                },
                                "mining.authorize" => {
                                    all_walllet_name = login(worker,&mut pool_w,&mut json_rpc,&mut worker_name,&config).await?;
                                    Ok(())
                                },
                                "mining.submit" => {
                                    json_rpc.set_id(CLIENT_SUBMITWORK);
                                    json_rpc.set_wallet(&all_walllet_name);
                                    worker.share_index_add();
                                    write_to_socket_byte(&mut pool_w, json_rpc.to_vec()?, &worker_name).await?;
                                    Ok(())
                                },
                                _ => {
                                    tracing::warn!("Not found ETH method {:?}",json_rpc);
                                    write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                    Ok(())
                                },
                            };

                            if res.is_err() {
                                tracing::warn!("write task error: {:?}",res);
                                return res;
                            }
                        }

                    } else {
                        tracing::warn!("Protocol parsing error: {:?}",buffer);
                        bail!("unknown protocol {}",buf_parse_to_string(&mut worker_w,&buffer).await?);
                    }
                }
            },
            res = pool_lines.next_line() => {
                let buffer = lines_unwrap(&mut worker_w,res,&worker_name,"mining pool").await?;

                #[cfg(debug_assertions)]
                debug!("<--------------------<  mining pool {} #{:?}",worker_name, buffer);

                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    if protocol == PROTOCOL::ETH {
                        if let Ok(mut job_rpc) = serde_json::from_str::<EthServerRootObject>(&buf) {
                            if job_rpc.id == CLIENT_GETWORK{
                                job_rpc.id = rpc_id;
                            } else {
                                job_rpc.id = 0;
                            }

                            write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name).await?;
                        } else if let Ok(result_rpc) = serde_json::from_str::<EthServer>(&buf) {
                            if result_rpc.id == CLIENT_LOGIN {

                                worker.logind();

                            } else if result_rpc.id == CLIENT_SUBHASHRATE {
                                //info!("{} Hashrate submitted successfully",worker_name);
                            } else if result_rpc.id == CLIENT_GETWORK {
                                //info!("{} Get task success",worker_name);
                            } else if result_rpc.id == SUBSCRIBE{
                            } else if result_rpc.id == CLIENT_SUBMITWORK && result_rpc.result {

                                worker.share_accept();


                            } else if result_rpc.id == CLIENT_SUBMITWORK {

                                worker.share_reject();

                            }
                        }
                    } else if protocol == PROTOCOL::STRATUM {

                        //write_rpc(is_encrypted,&mut worker_w,&)


                        if let Ok(_job_rpc) = serde_json::from_str::<EthSubscriptionNotify>(&buf) {

                        } else if let Ok(result_rpc) = serde_json::from_str::<StraumResult>(&buf) {

                            if let Some(res) = result_rpc.result.get(0) {
                                if *res == true {

                                    worker.share_accept();
                                } else {

                                    worker.share_reject();

                                }
                            }

                        } else if let Ok(_result_rpc) = serde_json::from_str::<StraumResultBool>(&buf) {

                            worker.logind();

                            //write_string(is_encrypted,&mut worker_w,&buf,&worker_name).await?;
                        } else {
                            tracing::error!("Fatal error. Agreement not found{:?}",buf);
                        }

                        write_string(is_encrypted,&mut worker_w,&buf,&worker_name).await?;
                    } else if protocol ==  PROTOCOL::NICEHASHSTRATUM {
                        if let Ok(_result_rpc) = serde_json::from_str::<StraumResult>(&buf) {

                        } else if let Ok(mut result_rpc) = serde_json::from_str::<StraumResultBool>(&buf) {

                            if result_rpc.id == CLIENT_SUBMITWORK {
                                result_rpc.id = rpc_id;
                                if result_rpc.result == true {

                                        worker.share_accept();

                                } else {

                                        worker.share_reject();

                                }

                                write_rpc(is_encrypted,&mut worker_w,&result_rpc,&worker_name).await?;
                            } else if result_rpc.id == CLIENT_LOGIN {
                                continue;
                            } else {

                                    worker.logind();

                            }

                            continue;
                        } else if let Ok(_set_rpc) = serde_json::from_str::<StraumMiningSet>(&buf) {
                        } else if let Ok(_set_rpc) = serde_json::from_str::<EthSubscriptionNotify>(&buf) {

                            write_string(is_encrypted,&mut worker_w,&buf,&worker_name).await?;

                            continue;
                        }
                        write_string(is_encrypted,&mut worker_w,&buf,&worker_name).await?;
                    }
                }
            },
            () = &mut sleep  => {
                // Send local miner status to remote.
                //info!("Send local miner status to remote. {:?}",worker);
                match workers_queue.send(worker.clone()){
                    Ok(_) => {},
                    Err(_) => {
                        tracing::warn!("Failed to send miner status");
                    },
                };
                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(30));
            },
        }
    }
}
