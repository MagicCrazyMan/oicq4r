pub mod core;

#[cfg(test)]
fn init_logger() {
    use log4rs::{
        append::{console::ConsoleAppender, file::FileAppender},
        config::{Appender, Logger, Root},
        Config,
    };

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(ConsoleAppender::builder().build())))
        .appender(Appender::builder().build(
            "file",
            Box::new(FileAppender::builder().build("./logger.log").unwrap()),
        ))
        .logger(Logger::builder().appender("stdout").build("console_logger", log::LevelFilter::Trace))
        .logger(
            Logger::builder()
                .appender("file")
                .additive(false)
                .build("file_logger", log::LevelFilter::Trace),
        )
        .build(
            Root::builder()
                .appender("stdout")
                .appender("file")
                .build(log::LevelFilter::Trace),
        )
        .unwrap();

    log4rs::init_config(config).unwrap();
}
