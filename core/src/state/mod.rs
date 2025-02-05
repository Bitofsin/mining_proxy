use std::u128;

extern crate serde_millis;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, info};

use crate::protocol::PROTOCOL;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Worker {
    pub worker: String,
    pub online: bool,
    pub worker_name: String,
    pub worker_wallet: String,
    pub protocol: PROTOCOL,
    #[serde(with = "serde_millis")]
    pub login_time: Instant,
    #[serde(with = "serde_millis")]
    pub last_subwork_time: Instant,
    pub rpc_id: u64,
    pub hash: u64,
    pub total_send_idx: u128,
    pub total_dev_idx: u128,
    pub total_fee_idx: u128,

    pub share_index: u64,
    pub accept_index: u64,
    pub invalid_index: u64,
    pub fee_share_index: u64,
    pub fee_accept_index: u64,
    pub fee_invalid_index: u64,
}

impl Worker {
    pub fn new(
        worker: String, worker_name: String, worker_wallet: String,
        online: bool,
    ) -> Self {
        Self {
            worker,
            online,
            worker_wallet,
            worker_name,
            login_time: Instant::now(),
            last_subwork_time: Instant::now(),
            protocol: PROTOCOL::KNOWN,
            hash: 0,
            total_send_idx: 0,
            total_fee_idx: 0,
            total_dev_idx: 0,
            share_index: 0,
            accept_index: 0,
            invalid_index: 0,
            fee_share_index: 0,
            fee_accept_index: 0,
            fee_invalid_index: 0,
            rpc_id: 0,
        }
    }

    pub fn default() -> Self {
        Self {
            worker: "".into(),
            online: false,
            worker_name: "".into(),
            worker_wallet: "".into(),
            protocol: PROTOCOL::KNOWN,
            login_time: Instant::now(),
            last_subwork_time: Instant::now(),
            hash: 0,
            share_index: 0,
            accept_index: 0,
            total_send_idx: 0,
            total_fee_idx: 0,
            total_dev_idx: 0,
            invalid_index: 0,
            fee_share_index: 0,
            fee_accept_index: 0,
            fee_invalid_index: 0,
            rpc_id: 0,
        }
    }

    pub fn login(
        &mut self, worker: String, worker_name: String, worker_wallet: String,
    ) {
        info!("miner: {} request login", worker);
        self.worker = worker;
        self.worker_name = worker_name;
        self.worker_wallet = worker_wallet;
    }

    pub fn logind(&mut self) {
        info!("Miner: {} Login successful", self.worker);
        self.online = true;
        self.clear_state();
    }

    pub fn send_job(&mut self) -> Result<()> {
        self.total_send_idx += 1;
        Ok(())
    }

    pub fn send_develop_job(&mut self) -> Result<()> {
        self.total_dev_idx += 1;
        Ok(())
    }

    pub fn send_fee_job(&mut self) -> Result<()> {
        self.total_fee_idx += 1;
        Ok(())
    }

    // offline
    pub fn offline(&mut self) -> bool {
        if self.is_online() {
            self.online = false;
            // info!(
            //     "Miner: {} offline online duration {}",
            //     self.worker,
            //     crate::util::time_to_string(self.login_time.elapsed().
            // as_secs()) );
        } else {
            info!("Malicious attack The protocol is incorrect. Agreement was not submitted correctly. Forced to close.");
        }
        true
    }

    // Set the current link protocol
    pub fn set_protocol(&mut self, p: PROTOCOL) { self.protocol = p; }

    // Determine if online
    pub fn is_online(&self) -> bool { self.online }

    // Call method to empty shares every ten minutes
    pub fn clear_state(&mut self) {
        // info!(
        //     "✅ worker {} clears all data. When emptying, there are the following data {} {} {}",
        //     self.worker, self.share_index, self.accept_index,
        // self.invalid_index );
        self.share_index = 0;
        self.accept_index = 0;
        self.invalid_index = 0;
        //self.login_time = Instant::now();
    }

    // total share increase
    pub fn share_index_add(&mut self) {
        self.last_subwork_time = Instant::now();

        self.share_index += 1;
        debug!("Miners: {} Share #{}", self.worker, self.share_index);
    }

    // take share
    pub fn share_accept(&mut self) {
        self.accept_index += 1;
        debug!("Miner: {} Share Accept #{}", self.worker, self.share_index);
    }

    // rejected share
    pub fn share_reject(&mut self) {
        self.invalid_index += 1;
        debug!("Miner: {} Share Reject #{}", self.worker, self.share_index);
    }

    // total share increase
    pub fn fee_share_index_add(&mut self) {
        //self.last_subwork_time = Instant::now();

        self.fee_share_index += 1;
        //debug!("Miners: {} Share #{}", self.worker, self.share_index);
    }

    // take share
    pub fn fee_share_accept(&mut self) {
        self.fee_accept_index += 1;
        //debug!("Miner: {} Share Accept #{}", self.worker, self.share_index);
    }

    // rejected share
    pub fn fee_share_reject(&mut self) {
        self.fee_invalid_index += 1;
        //debug!("Miner: {} Share Reject #{}", self.worker, self.share_index);
    }

    pub fn submit_hashrate<T>(&mut self, rpc: &T) -> bool
    where T: crate::protocol::rpc::eth::ClientRpc {
        self.hash = rpc.get_submit_hashrate();
        true
    }

    pub fn new_submit_hashrate(
        &mut self,
        rpc: &mut Box<
            dyn crate::protocol::ethjson::EthClientObject + Send + Sync,
        >,
    ) -> bool {
        self.hash = rpc.get_submit_hashrate();
        true
    }
}

#[test]
fn test_new_work() {
    let w = Worker::default();
    assert_eq!(w.share_index, 0);
    assert_eq!(w.accept_index, 0);
    assert_eq!(w.invalid_index, 0);
}

#[test]
fn test_share_index_add() {
    let mut w = Worker::default();
    w.share_index_add();
    assert_eq!(w.share_index, 1);
    assert_eq!(w.accept_index, 0);
    assert_eq!(w.invalid_index, 0);
}

#[test]
fn test_share_accept() {
    let mut w = Worker::default();
    w.share_accept();
    assert_eq!(w.share_index, 0);
    assert_eq!(w.accept_index, 1);
    assert_eq!(w.invalid_index, 0);
}

#[test]
fn test_share_reject() {
    let mut w = Worker::default();
    w.share_reject();
    assert_eq!(w.share_index, 0);
    assert_eq!(w.accept_index, 0);
    assert_eq!(w.invalid_index, 1);
}
