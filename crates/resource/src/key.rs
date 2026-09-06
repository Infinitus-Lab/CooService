//! 对象在池里的路径规则。
//!
//! sha256 前两位做一级目录：单目录下几万文件时，S3 list、FTP LIST、ext4 都会明显变慢。
//!
//! 所有入参统一先归一为小写十六进制：存储层（S3/FTP/本地）的文件名一律由
//! `encode(sha256, 'hex')` 生成的小写 key，大小写不归一会造成"库命中但文件 404、
//! 删除留孤儿"。

/// 校验失败：不是 64 位十六进制。
#[derive(Debug, thiserror::Error)]
#[error("invalid sha256 {0:?}, expected 64 hex chars")]
pub struct InvalidSha256(pub String);

/// 归一化 + 校验：`trim` 后转小写，再校验 64 位十六进制。所有 handler 入口统一用它，
/// 返回的字符串是小写，可安全用于 `decode($n,'hex')` 与对象路径。
pub fn normalize_sha256(raw: &str) -> Result<String, InvalidSha256> {
    let sha256 = raw.trim().to_ascii_lowercase();
    object_key(&sha256).map(|_| sha256)
}

/// `sha256` 十六进制（64 字符）→ `ab/cdef...`。
pub fn object_key(sha256: &str) -> Result<String, InvalidSha256> {
    let valid = sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit());
    if !valid {
        return Err(InvalidSha256(sha256.to_string()));
    }

    Ok(format!("{}/{}", &sha256[..2], &sha256[2..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_into_two_levels() {
        let key = object_key("ab".repeat(32).as_str()).unwrap();
        assert_eq!(key, "ab/".to_string() + &"ab".repeat(31));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(object_key("abc").is_err());
        assert!(object_key("g0".repeat(32).as_str()).is_err()); // 'g' 不是 hex
        assert!(object_key("AB".repeat(32).as_str()).is_ok()); // 大写合法
    }

    #[test]
    fn normalizes_case_and_whitespace() {
        let raw = format!("  {}  ", "AbC".repeat(21) + "A"); // 64 字符混合大小写
        assert_eq!(normalize_sha256(&raw).unwrap(), "abc".repeat(21) + "a");
        assert!(normalize_sha256("XYZ").is_err());
    }
}