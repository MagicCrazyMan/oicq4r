use std::time::SystemTime;

pub static BUF_0: [u8; 0] = [];
pub static BUF_1: [u8; 1] = [0];
pub static BUF_2: [u8; 2] = [0; 2];
pub static BUF_4: [u8; 4] = [0; 4];
pub static BUF_7: [u8; 7] = [0; 7];
pub static BUF_16: [u8; 16] = [0; 16];

// pub fn rand

pub fn current_unix_timestamp_as_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn current_unix_timestamp_as_millis() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
