use anyhow::{bail, Result};
use std::net::TcpStream;

// const POOLS:Vec<String> =  vec![
//     "127.0.0.1:4444".to_string(),
//     "127.0.0.1:4444".to_string(),
// ];

// const POOLS:Vec<String> = vec![
//     "127.0.0.1:4444".to_string(),
//     "127.0.0.1:4444".to_string(),
//     "127.0.0.1:4444".to_string(),
//     "127.0.0.1:4444".to_string(),
// ];

pub async fn get_develop_pool_stream() -> Result<TcpStream> {
    cfg_if::cfg_if! {
        if #[cfg(debug_assertions)] {
            let pools = vec![
                "127.0.0.1:8888".to_string(),
                "127.0.0.1:8888".to_string(),
            ];
        } else {
            let pools = vec![
                "127.0.0.1:4444".to_string(),
                "127.0.0.1:4444".to_string(),
                "127.0.0.1:4444".to_string(),
                // "127.0.0.1:4444".to_string(),
                "127.0.0.1:4444".to_string(),
                "127.0.0.1:4444".to_string(),
            ];
        }
    }

    let (stream, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            bail!("所有TCP矿池均不可链接。请修改后重试");
        }
    };

    Ok(stream)
}

// pub async fn get_proxy_pool_stream(_config: &crate::util::config::Settings)
// -> Result<TcpStream> {     cfg_if::cfg_if! {
//         if #[cfg(debug_assertions)] {
//             let pools = vec![
//                 "127.0.0.1:4444".to_string(),
//                 "127.0.0.1:4444".to_string(),
//             ];
//         }  else {
//             let pools = vec![
//                 "127.0.0.1:4444".to_string(),
//                 "127.0.0.1:4444".to_string(),
//                 "127.0.0.1:4444".to_string(),
//                 "127.0.0.1:4444".to_string(),
//             ];
//         }
//     }

//     let (stream, _) = match crate::client::get_pool_stream(&pools) {
//         Some((stream, addr)) => (stream, addr),
//         None => {
//             bail!("所有TCP矿池均不可链接。请修改后重试");
//         }
//     };

//     Ok(stream)
// }
