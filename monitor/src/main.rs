#![allow(dead_code)]
mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

use anyhow::Result;
use clap::{crate_name, crate_version, App, Arg, ArgMatches};
use std::net::ToSocketAddrs;
use tracing::info;
use tracing::Level;
use tracing_subscriber::fmt::{format::Writer, time::FormatTime};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = get_command_matches().await?;
    //mining_proxy::util::logger::init("monitor", "./logs/".into(), 0)?;
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
    info!(
        "✅ {}, Version: {} commit: {} {}",
        crate_name!(),
        crate_version!(),
        version::commit_date(),
        version::short_sha()
    );

    let port = matches.value_of("port").unwrap_or_else(|| {
        info!("Please fill in the local listening port correctly, for example: -p 8888");
        std::process::exit(1);
    });

    let server = matches.value_of("server").unwrap_or_else(|| {
        info!("Please fill in the server address correctly for example: -s 127.0.0.0:8888");
        std::process::exit(1);
    });

    let addr = match server.to_socket_addrs().unwrap().next() {
        Some(address) => address,
        None => {
            info!("Please fill in the server address correctly for example: -s 127.0.0.0:8888");
            std::process::exit(1);
        }
    };

    let port: i32 = port.parse().unwrap_or_else(|_| {
        info!("Please fill in the local listening port correctly, for example: -p 8888");
        std::process::exit(1);
    });

    let res =
        tokio::try_join!(core::client::monitor::accept_monitor_tcp(port, addr));

    if let Err(err) = res {
        tracing::warn!("Encryption service disconnected: {}", err);
    }

    Ok(())
}

pub async fn get_command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(format!(
        "{}, Version: {} commit {} {}",
        crate_name!(),
        crate_version!(),
        version::commit_date(),
        version::short_sha()
    ))
    .version(crate_version!())
    //.author(crate_authors!("\n"))
    //.about(crate_description!())
    .arg(
        Arg::with_name("port")
            .short("p")
            .long("port")
            .help("local listening port")
            .takes_value(true),
    )
    .arg(
        Arg::with_name("server")
            .short("s")
            .long("server")
            .help("Server listening port")
            .takes_value(true),
    )
    .get_matches();
    Ok(matches)
}
