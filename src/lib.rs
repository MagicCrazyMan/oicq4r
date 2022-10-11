pub mod core;

#[cfg(test)]
fn init_logger() {
    #[cfg(test)]
    let _ = env_logger::builder()
        .filter_module("oicq4r", log::LevelFilter::Trace)
        .is_test(true)
        .try_init();
}
