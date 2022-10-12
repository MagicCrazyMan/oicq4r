pub mod core;

#[cfg(test)]
fn init_logger() {
    fern::Dispatch::default()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                chrono::Utc::now().format("[%Y-%m-%d][%H:%M:%S+%Z]"),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Trace)
        .chain(std::io::stdout())
        .chain(fern::log_file("./logger.log").unwrap())
        .apply()
        .unwrap();
}
