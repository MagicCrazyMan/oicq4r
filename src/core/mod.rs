pub mod jce;
pub mod base_client;
pub mod network;
pub mod helper;
pub mod tea;
pub mod ecdh;
pub mod tlv;
pub mod device;
pub mod io;

/// Protobuf 的编解码采用各自采用两种不同的处理方式
/// encode 采用 trait，将常见的数据类型添加自定义特性，并通过借用的方式获取数据然后编码，以避免使用非必要 Clone 重复性克隆已有数据
/// decode 由于需要在解码后保存相应的数据，因此采用了完全独立的数据结构以存放相应的数据
pub mod protobuf;
