//! 资源池：把 S3 / FTP 等远端服务抽象成哈希池，统一为 [`RemotePool`] trait。

pub mod error;
pub mod ftp;
pub mod health;
pub mod key;
pub mod local;
pub mod pool;
pub mod s3;
