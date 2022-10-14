pub mod client;
pub mod common;
pub mod core;
pub mod error;
pub mod internal;
pub mod message;

/// 生成十六进制字符串
///
/// 每个字节强制两个字符的长度，不足的用 0 补齐，
/// 每个字节之间没有连接字符
pub(crate) trait ToHexString {
    fn to_hex_string(&self) -> String;
}

impl<T> ToHexString for T
where
    T: AsRef<[u8]>,
{
    fn to_hex_string(&self) -> String {
        self.as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("")
    }
}

#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
fn tmp_dir() -> Result<PathBuf, std::io::Error> {
    use std::fs;

    let tmp_dir = std::env::current_dir()?.join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    Ok(tmp_dir)
}

#[cfg(test)]
fn init_logger() -> Result<(), std::io::Error> {
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
        .chain(fern::log_file(tmp_dir()?.join("logger.log"))?)
        .apply()
        .unwrap();

    Ok(())
}
