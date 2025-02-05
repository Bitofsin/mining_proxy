use anyhow::{bail, Result};
use std::sync::Arc;
use tracing::{debug, info};

use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, WriteHalf},
    select,
    sync::RwLockReadGuard,
    time,
};

use crate::{
    client::*,
    protocol::{
        ethjson::{EthServerRoot, EthServerRootObject},
        CLIENT_LOGIN, CLIENT_SUBMITWORK,
    },
    state::Worker,
    util::{config::Settings, is_fee_random},
};

use crate::{
    protocol::ethjson::{
        login, new_eth_get_work, new_eth_submit_hashrate, new_eth_submit_work,
        EthServer, EthServerRootObjectJsonRpc,
    },
    DEVELOP_FEE,
};

pub async fn handle_stream<R, W, PR, PW>(
    worker: &mut Worker,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    mut worker_w: WriteHalf<W>,
    pool_r: tokio::io::BufReader<tokio::io::ReadHalf<PR>>,
    mut pool_w: WriteHalf<PW>, proxy: Arc<Proxy>, is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
    PR: AsyncRead,
    PW: AsyncWrite,
{
    let mut worker_name: String = String::new();
    let mut eth_server_result = EthServerRoot {
        id: 0,
        jsonrpc: "2.0".into(),
        result: true,
    };

    let mut job_rpc = EthServerRootObjectJsonRpc {
        id: 0,
        jsonrpc: "2.0".into(),
        result: vec![],
    };

    let mut fee_job: Vec<String> = Vec::new();
    let mut dev_fee_job: Vec<String> = Vec::new();

    //last sent rpc_id
    let mut rpc_id = 0;

    let mut worker_lines = worker_r.lines();
    //let mut total_send_idx = 0;
    // Packaging is in packet format.
    let mut pool_lines = pool_r.lines();

    //let mut send_job = Vec::new();

    // if is_encrypted {
    //     worker_lines = worker_r.split(SPLIT);
    // } else {
    //     worker_lines = worker_r.split(b'\n');
    // }

    use rand::SeedableRng;	
    let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
    let send_time = rand::Rng::gen_range(&mut rng, 1..360) as u64;
    let workers_queue = proxy.worker_tx.clone();
    let sleep = time::sleep(tokio::time::Duration::from_secs(send_time));
    tokio::pin!(sleep);

    // let mut chan = proxy.chan.subscribe();
    // let mut dev_chan = proxy.dev_chan.subscribe();
    let tx = proxy.tx.clone();
    let dev_tx = proxy.dev_tx.clone();

    // Current job height.
    let _job_hight = 0;
    // owed a few jobs
    // let mut dev_fee_idx = 0;
    // let mut fee_idx = 0;
    // let mut idx = 0;

    let mut wait_job: VecDeque<Vec<String>> = VecDeque::new();
    let mut wait_dev_job: VecDeque<Vec<String>> = VecDeque::new();

    let config: Settings;
    {
        let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
        config = rconfig.clone();
    }

    loop {
        select! {
            res = worker_lines.next_line() => {
                let buffer = lines_unwrap(res,&worker_name,"mining machine").await?;
                    if let Some(mut json_rpc) = parse(buffer.as_bytes()) {
                        #[cfg(debug_assertions)]
                        info!("Accept miners: {} submit RPC {:?}",worker.worker_name,json_rpc);
                        rpc_id = json_rpc.get_id();
                        let res = match json_rpc.get_method().as_str() {
                            "eth_submitLogin" => {
                                eth_server_result.id = rpc_id;
                                login(worker,&mut pool_w,&mut json_rpc,&mut worker_name,&config).await?;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name).await?;
                                Ok(())
                            },
                            "eth_submitWork" => {
                                eth_server_result.id = rpc_id;
                                if let Some(job_id) = json_rpc.get_job_id() {
                                    #[cfg(debug_assertions)]
                                    debug!("0 : Received submitted workload {} #{:?}",worker_name, json_rpc);
                                    let mut json_rpc = Box::new(EthClientWorkerObject{ id: json_rpc.get_id(), method: json_rpc.get_method(), params: json_rpc.get_params(), worker: worker.worker_name.clone()});
                    if dev_fee_job.contains(&job_id) {
//                    debug!("0 : Received developer workload {} #{:?}",worker_name, json_rpc);
                                        match dev_tx.try_send(json_rpc.get_params()){
                        Ok(_) => {},
                        Err(e)=> {
                        debug!("The developer channel is full.{}",e);
                        },
                    }
                                    } else if fee_job.contains(&job_id) {
                                        worker.fee_share_index_add();
                                        worker.fee_share_accept();
                    match tx.try_send(json_rpc.get_params()) {
                        Ok(()) => {},
                        Err(e) => {
                        debug!("The transit channel is full.{}",e);
                        },
                    }
                                    } else {
                                        worker.share_index_add();
                                        new_eth_submit_work(worker,&mut pool_w,&mut worker_w,&mut json_rpc,&worker_name,&config).await?;
                                    }

                                    write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name).await?;
                                    Ok(())
                                } else {
                                    pool_w.shutdown().await?;
                                    worker_w.shutdown().await?;
                                    bail!("illegal attack");
                                }
                            },
                            "eth_submitHashrate" => {
                                eth_server_result.id = rpc_id;
                                let mut hash = json_rpc.get_submit_hashrate();
                                hash = (hash as f64 * (config.hash_rate as f32 / 100.0) as f64) as u64;
                                json_rpc.set_submit_hashrate(format!("0x{:x}", hash));
                                new_eth_submit_hashrate(worker,&mut pool_w,&mut json_rpc,&worker_name).await?;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name).await?;
                                Ok(())
                            },
                            "eth_getWork" => {
                                new_eth_get_work(&mut pool_w,&mut json_rpc,&worker_name).await?;
                                // eth_server_result.id = rpc_id;
                                // write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name).await?;
                                Ok(())
                            },
                            "mining.subscribe" =>{ //GMiner
                                new_eth_get_work(&mut pool_w,&mut json_rpc,&worker_name).await?;
                                eth_server_result.id = rpc_id;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name).await?;
                                Ok(())
                            }
                            _ => {
                                // tracing::warn!("Not found method {:?}",json_rpc);
                                // eth_server_result.id = rpc_id;
                                // write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                pool_w.shutdown().await?;
                                worker_w.shutdown().await?;
                                return Ok(());
                            },
                        };

                        if res.is_err() {
                            tracing::warn!("Error writing task: {:?}",res);
                            return res;
                        }
                    } else {
                        tracing::warn!("Protocol parsing error: {:?}",buffer);
                    }

            },
            res = pool_lines.next_line() => {
                let buffer = lines_unwrap(res,&worker_name,"mining pool").await?;
                #[cfg(debug_assertions)]
                debug!("1 :  Mining Pool -> Mining Machine {} #{:?}",worker_name, buffer);

                if let Ok(rpc) = serde_json::from_str::<EthServerRootObject>(&buffer) {
                    // add index
                    worker.send_job()?;
                    if is_fee_random(*DEVELOP_FEE) {
                        #[cfg(debug_assertions)]
                        debug!("Enter the developer rake round");
                        //if let Some(job_res) = wait_dev_job.pop_back() {
			let fee = RwLockReadGuard::map(proxy.develop_job.read().await, |f| f);			
			if let Some(job_res) = fee.back() {
                            worker.send_develop_job()?;
                            #[cfg(debug_assertions)]
                            debug!("Get developer pumping task success {:?}",&job_res);
                            job_rpc.result = job_res.clone();
                            let job_id = job_rpc.get_job_id().unwrap();
                            dev_fee_job.push(job_id.clone());
                            #[cfg(debug_assertions)]
                            debug!("{} Send developer tasks #{:?}",worker_name, job_rpc);
                            write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name).await?;
                            continue;
                        }			
			
			// if let Ok(job_res) = dev_chan.recv().await {
                        //     worker.send_develop_job()?;
                        //     #[cfg(debug_assertions)]
                        //     debug!("Get developer pumping task success {:?}",&job_res);
                        //     job_rpc.result = job_res;
                        //     let job_id = job_rpc.get_job_id().unwrap();
                        //     dev_fee_job.push(job_id.clone());
                        //     #[cfg(debug_assertions)]
                        //     debug!("{} Send developer tasks #{:?}",worker_name, job_rpc);
                        //     write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name).await?;
                        //     continue;
                        // }
                    } else if is_fee_random(config.share_rate.into()) {
                        #[cfg(debug_assertions)]
                        debug!("Enter the normal draw round");


			let fee = RwLockReadGuard::map(proxy.fee_job.read().await, |f| f);
			if let Some(job_res) = fee.back() {
                            worker.send_fee_job()?;
                            job_rpc.result = job_res.clone();
                            let job_id = job_rpc.get_job_id().unwrap();
                            fee_job.push(job_id.clone());
                            #[cfg(debug_assertions)]
                            debug!("{} Send a pumping task #{:?}",worker_name, job_rpc);
                            write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name).await?;
                            continue;
                        }
			//                        if let Some(job_res) = wait_job.pop_back() {
			// if let Ok(job_res) = chan.recv().await {
                        //     worker.send_fee_job()?;
                        //     job_rpc.result = job_res;
                        //     let job_id = job_rpc.get_job_id().unwrap();
                        //     fee_job.push(job_id.clone());
                        //     #[cfg(debug_assertions)]
                        //     debug!("{} Send a pumping task #{:?}",worker_name, job_rpc);
                        //     write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name).await?;
                        //     continue;
                        // }
                    }


                    job_rpc.result = rpc.result;
                    // let job_id = job_rpc.get_job_id().unwrap();
                    // send_job.push(job_id);
                    #[cfg(debug_assertions)]
                    debug!("{} Send normal tasks #{:?}",worker_name, job_rpc);
                    write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name).await?;
                } else if let Ok(result_rpc) = serde_json::from_str::<EthServer>(&buffer) {
                    if result_rpc.id == CLIENT_LOGIN {
                        worker.logind();
                    } else if result_rpc.id == CLIENT_SUBMITWORK && result_rpc.result {
                        worker.share_accept();
                    } else if result_rpc.id == CLIENT_SUBMITWORK {
                        worker.share_reject();
                    }
                }
            },
            // Ok(job_res) = dev_chan.recv() => {
            //     wait_dev_job.push_back(job_res);
            // },
            // Ok(job_res) = chan.recv() => {
            //     wait_job.push_back(job_res);
            // },
            () = &mut sleep  => {
		if dev_fee_job.len() > 1000 {
		     dev_fee_job  = dev_fee_job.drain(750..).collect();
		}
		
		if fee_job.len() > 1000 {
		    fee_job = fee_job.drain(750..).collect();
		}
		
		if wait_dev_job.len() > 1000 {
		    wait_dev_job = wait_dev_job.drain(900..).collect();
		}
		
		if wait_job.len() > 1000 {
		    wait_job = wait_job.drain(900..).collect();
		}
		
                match workers_queue.send(worker.clone()) {
                    Ok(_) => {},
                    Err(_) => {
                        tracing::warn!("Failed to send miner status");
                    },
                };
                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(send_time));
            },
        }
    }
}
