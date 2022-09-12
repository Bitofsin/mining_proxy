// pub fn init(
//     app_name: &str, path: String, log_level: u32,
// ) -> anyhow::Result<()> {
//     let lavel = match log_level {
//         4 => log::LevelFilter::Off,
//         3 => log::LevelFilter::Error,
//         2 => log::LevelFilter::Warn,
//         1 => log::LevelFilter::Info,
//         0 => log::LevelFilter::Debug,
//         _ => log::LevelFilter::Info,
//     };
//     cfg_if::cfg_if! {
//         if #[cfg(debug_assertions)] {
//             if path != "" {
//                 let log = fern::DateBased::new(path,
// format!("{}.log.%Y-%m-%d.%H", app_name))                     .utc_time()
//                     .local_time();
//                 let (lavel, logger) = fern::Dispatch::new()
//                     .format(move |out, message, record| {
//                         out.finish(format_args!(
//                             "[{}] [{}] [{}:{}] {}",
//                             chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//                             record.level(),
//                             record.file().expect("Failed to get file name"),
//                             record.line().expect("Failed to get file line number"),
//                             message
//                         ))
//                     })
//                     .level(lavel)
//                     .level_for("reqwest", log::LevelFilter::Off)
//                     .chain(std::io::stdout())
//                     .chain(log)
//                     .into_log();

//                 // let logger =
// sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
//                 //     log::Level::Error => sentry_log::LogFilter::Event,
//                 //     log::Level::Warn => sentry_log::LogFilter::Event,
//                 //     _ => sentry_log::LogFilter::Ignore,
//                 // });

//                 log::set_boxed_logger(Box::new(logger)).unwrap();
//                 log::set_max_level(lavel);
//             } else {
//                 let (lavel, logger) = fern::Dispatch::new()
//                     .format(move |out, message, record| {
//                         out.finish(format_args!(
//                             "[{}] [{}] [{}:{}] {}",
//                             record.level(),
//                             chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//                             record.file().expect("Failed to get file name"),
//                             record.line().expect("Failed to get file line number"),
//                             message
//                         ))
//                     })
//                     .level(lavel)
//                     .level_for("reqwest", log::LevelFilter::Off)
//                     .chain(std::io::stdout())
//                     .into_log();

//                 // let logger =
// sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
//                 //     log::Level::Error => sentry_log::LogFilter::Event,
//                 //     log::Level::Warn => sentry_log::LogFilter::Event,
//                 //     _ => sentry_log::LogFilter::Ignore,
//                 // });

//                 log::set_boxed_logger(Box::new(logger)).unwrap();
//                 log::set_max_level(lavel);
//             }
//         }  else {

//             if path != "" {
//                 let log = fern::DateBased::new(path,
// format!("{}.log.%Y-%m-%d.%H", app_name))                     .utc_time()
//                     .local_time();
//                 let (lavel, logger) = fern::Dispatch::new()
//                     .format(move |out, message, record| {
//                         out.finish(format_args!(
//                             "[{}] [{}] {}",
//                             chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//                             record.level(),
//                             message
//                         ))
//                     })
//                     .level(lavel)
//                     .level_for("reqwest", log::LevelFilter::Off)
//                     .chain(std::io::stdout())
//                     .chain(log)
//                     .into_log();

//                 // let logger =
// sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
//                 //     log::Level::Error => sentry_log::LogFilter::Event,
//                 //     log::Level::Warn => sentry_log::LogFilter::Event,
//                 //     _ => sentry_log::LogFilter::Ignore,
//                 // });

//                 log::set_boxed_logger(Box::new(logger)).unwrap();
//                 log::set_max_level(lavel);
//             } else {
//                 let (lavel, logger) = fern::Dispatch::new()
//                     .format(move |out, message, record| {
//                         out.finish(format_args!(
//                             "[{}] [{}] {}",
//                             chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//                             record.level(),
//                             message
//                         ))
//                     })
//                     .level(lavel)
//                     .level_for("reqwest", log::LevelFilter::Off)
//                     .chain(std::io::stdout())
//                     .into_log();

//                 // let logger =
// sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
//                 //     log::Level::Error => sentry_log::LogFilter::Event,
//                 //     log::Level::Warn => sentry_log::LogFilter::Event,
//                 //     _ => sentry_log::LogFilter::Ignore,
//                 // });

//                 log::set_boxed_logger(Box::new(logger)).unwrap();
//                 log::set_max_level(lavel);
//             }
//         }
//     }

//     Ok(())
// }

// pub fn init_client(log_level: u32) -> anyhow::Result<()> {
//     let lavel = match log_level {
//         4 => log::LevelFilter::Off,
//         3 => log::LevelFilter::Error,
//         2 => log::LevelFilter::Warn,
//         1 => log::LevelFilter::Info,
//         0 => log::LevelFilter::Debug,
//         _ => log::LevelFilter::Info,
//     };

//     let (lavel, logger) = fern::Dispatch::new()
//         .format(move |out, message, record| {
//             out.finish(format_args!(
//                 "[{}] [{}:{}] [{}] {}",
//                 chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//                 record.file().unwrap(),
//                 record.line().unwrap(),
//                 record.level(),
//                 message
//             ))
//         })
//         .level(lavel)
//         .level_for("reqwest", log::LevelFilter::Off)
//         .chain(std::io::stdout())
//         .into_log();

//     // let logger = sentry_log::SentryLogger::with_dest(logger).filter(|md|
//     // match md.level() {     log::Level::Error =>
//     // sentry_log::LogFilter::Event,     log::Level::Warn =>
//     // sentry_log::LogFilter::Event,     _ => sentry_log::LogFilter::Ignore,
//     // });

//     log::set_boxed_logger(Box::new(logger)).unwrap();
//     log::set_max_level(lavel);

//     Ok(())
// }

use std::io;

use tracing::*;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::{self, fmt::time::FormatTime};

#[inline(always)]
pub fn init() {
    struct LocalTimer;
    impl FormatTime for LocalTimer {
        fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
            write!(w, "{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
        }
    }

    let file_appender =
        tracing_appender::rolling::daily("./logs/", "mining_proxy");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Set the format of the log output, for example, whether to include the log level, whether to include the log source location、
    // Set the time format of the log Reference: https://docs.rs/tracing-subscriber/0.3.3/tracing_subscriber/fmt/struct.SubscriberBuilder.html#method.with_timer
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(false)
        .with_line_number(true)
        .with_source_location(true)
        .with_timer(LocalTimer);

    // Initialize and set log format (customize and filter logs)
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_writer(io::stdout) // write to standard output
        .with_writer(non_blocking) // Write to file, will overwrite standard output above
        .with_ansi(false) // If the log is written to a file, the color output function of ansi should be turned off
        .event_format(format)
        .init();

    // tracing::subscriber::set_global_default(subscriber)
    //     .expect("setting default subscriber failed");
}
