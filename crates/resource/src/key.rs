//! 对象在池里的路径规则。
//!
//! sha256 前两位做一级目录：单目录下几万文件时，S3 list、FTP LIST、ext4 都会明显变慢。

/// 校验失败：不是 64 位十六进制。
#[derive(Debug, thiserror::Error)]
#[error("invalid sha256 {0:?}, expected 64 hex chars")]
pub struct InvalidSha256(pub String);

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
}
