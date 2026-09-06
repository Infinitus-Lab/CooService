//! 资源池凭据（S3 secret key / FTP 密码）的静态加密。
//!
//! 凭据以密文落库，主密钥由 `STORAGE_SECRET_MASTER_KEY` 环境变量注入（64 位十六进制 =
//! 32 字节 AES-256 密钥）。库表结构不变：`secret` 列存 `enc:v1:<base64(nonce|密文|tag)>`。
//!
//! 未配置主密钥时退化为明文直存（与旧版本一致），并记一次告警——上线前必须配置，
//! 否则 DB 备份 / pg_dump / 只读副本会携带全部存储凭据。
//!
//! 旧数据兼容：不以 `enc:v1:` 开头的值视为历史明文，原样返回。

use aes_gcm::{
    Aes256Gcm, Key, KeyInit, Nonce,
    aead::Aead,
};
use base64::Engine;
use rand::{TryRng, rngs::SysRng};

const HEX_TABLE: &[u8; 16] = b"0123456789abcdef";

const PREFIX: &str = "enc:v1:";
const NONCE_LEN: usize = 12;

/// 进程内只从环境变量解析一次主密钥（`OnceLock`），避免每次加解密都读 env。
static MASTER_KEY: std::sync::OnceLock<Option<Key<Aes256Gcm>>> = std::sync::OnceLock::new();

fn master_key() -> Option<&'static Key<Aes256Gcm>> {
    MASTER_KEY
        .get_or_init(|| {
            let raw = std::env::var("STORAGE_SECRET_MASTER_KEY").ok();
            let Some(raw) = raw else {
                tracing::warn!(
                    "STORAGE_SECRET_MASTER_KEY not set: pool secrets stored in plaintext"
                );
                return None;
            };
            match decode_hex(&raw) {
                Ok(key) if key.len() == 32 => Some(*Key::<Aes256Gcm>::from_slice(&key)),
                Ok(_) => {
                    tracing::error!(
                        "STORAGE_SECRET_MASTER_KEY must be 64 hex chars (32 bytes), falling back to plaintext"
                    );
                    None
                }
                Err(e) => {
                    tracing::error!(
                        "invalid STORAGE_SECRET_MASTER_KEY: {e}, falling back to plaintext"
                    );
                    None
                }
            }
        })
        .as_ref()
}

/// 加密落库前的凭据；未配置主密钥时原样返回。
pub fn encrypt_secret(plain: &str) -> String {
    let Some(key) = master_key() else {
        return plain.to_string();
    };

    let cipher = Aes256Gcm::new(key);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    SysRng
        .try_fill_bytes(&mut nonce_bytes)
        .expect("os rng unavailable");
    let nonce = Nonce::from_slice(&nonce_bytes);

    // 随机 nonce：同一明文每次密文不同。`secret` 从不参与 WHERE 查询，非确定性无副作用。
    let ciphertext = cipher
        .encrypt(nonce, plain.as_bytes())
        .expect("aes-gcm encrypt with fixed-size key cannot fail");

    let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&ciphertext);

    format!(
        "{PREFIX}{}",
        base64::engine::general_purpose::STANDARD.encode(payload)
    )
}

/// 解密库中读出的凭据；旧明文 / 未配置主密钥时原样返回。
pub fn decrypt_secret(stored: &str) -> String {
    let Some(stored) = stored.strip_prefix(PREFIX) else {
        return stored.to_string();
    };
    let Some(key) = master_key() else {
        return stored.to_string();
    };

    let payload = match base64::engine::general_purpose::STANDARD.decode(stored) {
        Ok(payload) if payload.len() > NONCE_LEN => payload,
        _ => return stored.to_string(),
    };

    let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(key);
    match cipher.decrypt(Nonce::from_slice(nonce_bytes), ciphertext) {
        Ok(plain) => String::from_utf8(plain).unwrap_or_else(|_| stored.to_string()),
        // 主密钥轮换后旧密文解不开，原样返回（建连失败会暴露，属运维事件）
        Err(_) => stored.to_string(),
    }
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err("odd hex length".into());
    }
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let hi = HEX_TABLE
                .iter()
                .position(|&b| b == pair[0])
                .ok_or("invalid hex")?;
            let lo = HEX_TABLE
                .iter()
                .position(|&b| b == pair[1])
                .ok_or("invalid hex")?;
            Ok((hi << 4 | lo) as u8)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试进程默认未配置主密钥 = 明文模式，行为与旧版本一致。
    #[test]
    fn roundtrip_plaintext_mode() {
        let secret = "s3secret-key";
        let enc = encrypt_secret(secret);
        assert_eq!(decrypt_secret(&enc), secret);
    }

    #[test]
    fn legacy_plaintext_passes_through() {
        assert_eq!(decrypt_secret("legacy-plain"), "legacy-plain");
    }
}