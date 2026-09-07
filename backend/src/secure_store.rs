use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use anyhow::{Context, Result, bail};
use base64::Engine;
use rand::RngCore;
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, path::PathBuf};

/// OAuth token persistence boundary. Platform writers only need clear serialized bytes; the
/// implementation owns encryption, key custody and deletion. Local Windows uses DPAPI. A hosted
/// deployment must inject an implementation backed by a managed secret/KMS service rather than
/// writing plaintext refresh tokens to the application database.
pub trait OAuthTokenStore: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn load_cleartext(&self) -> Result<Vec<u8>>;
    fn save_cleartext(&self, cleartext: &[u8]) -> Result<()>;
    fn remove(&self) -> Result<()>;
}

#[derive(Clone)]
pub struct SecureJsonStore {
    path: PathBuf,
    server_key: Option<[u8; 32]>,
}

pub fn server_key() -> Result<[u8; 32]> {
    let encoded = std::env::var("OAUTH_TOKEN_ENCRYPTION_KEY")
        .context("CONFIG_REQUIRED: OAUTH_TOKEN_ENCRYPTION_KEY is required")?;
    decode_key(&encoded)
}

fn decode_key(encoded: &str) -> Result<[u8; 32]> {
    base64::engine::general_purpose::STANDARD.decode(encoded.trim()).ok()
        .and_then(|bytes| bytes.try_into().ok())
        .context("CONFIG_REQUIRED: OAUTH_TOKEN_ENCRYPTION_KEY must encode exactly 32 random bytes as base64")
}

impl SecureJsonStore {
    pub fn for_oauth_provider(provider: &str) -> Option<Self> {
        if std::env::var("OAUTH_TOKEN_STORE").is_ok_and(|v| v == "server_encrypted") {
            return Some(Self {
                path: PathBuf::from(std::env::var("MELODYPATH_DATA_DIR").ok()?)
                    .join(format!("oauth-{provider}.bin")),
                server_key: Some(server_key().ok()?),
            });
        }
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
        Some(Self {
            path: base
                .join("MelodyPath")
                .join(format!("oauth-{provider}.bin")),
            server_key: None,
        })
    }

    pub fn load<T: DeserializeOwned>(&self) -> Result<T> {
        let clear = self.load_cleartext()?;
        serde_json::from_slice(&clear).context("本地加密 OAuth 会话格式无效")
    }

    pub fn save<T: Serialize>(&self, value: &T) -> Result<()> {
        let clear = serde_json::to_vec(value).context("无法序列化 OAuth 会话")?;
        self.save_cleartext(&clear)
    }

    pub fn remove(&self) -> Result<()> {
        OAuthTokenStore::remove(self)
    }

    /// Fail closed before serving requests if an existing encrypted store cannot be opened.
    pub fn verify_existing(&self) -> Result<()> {
        if self.path.exists() {
            let _: serde_json::Value = self.load().context("CONFIG_REQUIRED: encrypted OAuth store could not be opened; restore the correct key before starting")?;
        }
        Ok(())
    }

    fn seal(&self, cleartext: &[u8]) -> Result<Vec<u8>> {
        let Some(key) = self.server_key.as_ref() else {
            return protect(cleartext);
        };
        let mut nonce = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce);
        let aad = self
            .path
            .file_name()
            .and_then(|v| v.to_str())
            .context("invalid OAuth store path")?;
        let encrypted = Aes256Gcm::new(key.into())
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: cleartext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("OAuth encryption failed"))?;
        Ok([b"MPG1".as_slice(), &nonce, &encrypted].concat())
    }

    fn open(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        let Some(key) = self.server_key.as_ref() else {
            return unprotect(encrypted);
        };
        if encrypted.len() < 32 || &encrypted[..4] != b"MPG1" {
            bail!("Invalid encrypted OAuth envelope");
        }
        let aad = self
            .path
            .file_name()
            .and_then(|v| v.to_str())
            .context("invalid OAuth store path")?;
        Aes256Gcm::new(key.into())
            .decrypt(
                Nonce::from_slice(&encrypted[4..16]),
                Payload {
                    msg: &encrypted[16..],
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("OAuth store authentication failed"))
    }
}

impl OAuthTokenStore for SecureJsonStore {
    fn backend_name(&self) -> &'static str {
        if self.server_key.is_some() {
            "server_aes256_gcm"
        } else {
            "windows_dpapi_user_scope"
        }
    }

    fn load_cleartext(&self) -> Result<Vec<u8>> {
        let encrypted = fs::read(&self.path).context("无法读取本地加密 OAuth 会话")?;
        self.open(&encrypted)
    }

    fn save_cleartext(&self, cleartext: &[u8]) -> Result<()> {
        let encrypted = self.seal(cleartext)?;
        let parent = self.path.parent().context("OAuth 会话目录无效")?;
        fs::create_dir_all(parent).context("无法创建 OAuth 会话目录")?;
        if self.server_key.is_some() {
            use std::io::Write;
            let temporary = self
                .path
                .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temporary)
                .context("Cannot create encrypted OAuth store")?;
            file.write_all(&encrypted)?;
            file.sync_all()?;
            drop(file);
            // Atomic replacement on Linux; never remove the old store before replacement.
            fs::rename(&temporary, &self.path)
                .context("Cannot atomically replace encrypted OAuth store")?;
        } else {
            fs::write(&self.path, encrypted).context("无法写入本地加密 OAuth 会话")?;
        }
        Ok(())
    }

    fn remove(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("无法删除本地加密 OAuth 会话"),
        }
    }
}

#[cfg(windows)]
#[repr(C)]
struct DataBlob {
    size: u32,
    data: *mut u8,
}

#[cfg(windows)]
#[link(name = "Crypt32")]
unsafe extern "system" {
    fn CryptProtectData(
        input: *const DataBlob,
        description: *const u16,
        entropy: *const DataBlob,
        reserved: *mut core::ffi::c_void,
        prompt: *mut core::ffi::c_void,
        flags: u32,
        output: *mut DataBlob,
    ) -> i32;
    fn CryptUnprotectData(
        input: *const DataBlob,
        description: *mut *mut u16,
        entropy: *const DataBlob,
        reserved: *mut core::ffi::c_void,
        prompt: *mut core::ffi::c_void,
        flags: u32,
        output: *mut DataBlob,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "Kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
}

#[cfg(windows)]
fn protect(clear: &[u8]) -> Result<Vec<u8>> {
    crypt(clear, true)
}

#[cfg(windows)]
fn unprotect(encrypted: &[u8]) -> Result<Vec<u8>> {
    crypt(encrypted, false)
}

#[cfg(windows)]
fn crypt(input: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
    let input_size = u32::try_from(input.len()).context("OAuth 会话过大")?;
    let input_blob = DataBlob {
        size: input_size,
        data: input.as_ptr().cast_mut(),
    };
    let mut output_blob = DataBlob {
        size: 0,
        data: std::ptr::null_mut(),
    };
    let success = unsafe {
        if encrypt {
            CryptProtectData(
                &input_blob,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output_blob,
            )
        } else {
            CryptUnprotectData(
                &input_blob,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output_blob,
            )
        }
    };
    if success == 0 || output_blob.data.is_null() {
        bail!(
            "Windows 无法保护本地 OAuth 会话：{}",
            std::io::Error::last_os_error()
        );
    }
    let output =
        unsafe { std::slice::from_raw_parts(output_blob.data, output_blob.size as usize).to_vec() };
    unsafe {
        LocalFree(output_blob.data.cast());
    }
    Ok(output)
}

#[cfg(not(windows))]
fn protect(_clear: &[u8]) -> Result<Vec<u8>> {
    bail!("此平台没有可用的本地用户凭据保护实现")
}

#[cfg(not(windows))]
fn unprotect(_encrypted: &[u8]) -> Result<Vec<u8>> {
    bail!("此平台没有可用的本地用户凭据保护实现")
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_user_scope_protection_round_trips_without_plaintext_storage() {
        let clear = b"local oauth session fixture";
        let Ok(encrypted) = protect(clear) else {
            // Some isolated CI/service profiles do not expose a DPAPI master key.
            // Runtime authorization remains in memory and reports persistence failure.
            return;
        };
        assert_ne!(encrypted, clear);
        assert_eq!(unprotect(&encrypted).unwrap(), clear);
    }

    #[test]
    fn local_store_declares_its_key_custody_backend() {
        let Some(store) = SecureJsonStore::for_oauth_provider("test-interface") else {
            return;
        };
        assert_eq!(store.backend_name(), "windows_dpapi_user_scope");
    }
}

#[cfg(test)]
mod encrypted_tests {
    use super::*;
    #[test]
    fn authenticated_encryption_rejects_tampering_wrong_key_and_provider() {
        let store = SecureJsonStore {
            path: PathBuf::from("oauth-test.bin"),
            server_key: Some([7; 32]),
        };
        let clear = br#"{"synthetic-session":"synthetic-token"}"#;
        let mut encrypted = store.seal(clear).unwrap();
        assert_ne!(encrypted, store.seal(clear).unwrap());
        assert!(!encrypted.windows(15).any(|w| w == b"synthetic-token"));
        assert_eq!(store.open(&encrypted).unwrap(), clear);
        let wrong_key = SecureJsonStore {
            server_key: Some([8; 32]),
            ..store.clone()
        };
        let wrong_provider = SecureJsonStore {
            path: PathBuf::from("oauth-other.bin"),
            ..store.clone()
        };
        assert!(wrong_key.open(&encrypted).is_err());
        assert!(wrong_provider.open(&encrypted).is_err());
        encrypted[17] ^= 1;
        assert!(store.open(&encrypted).is_err());
        assert!(store.open(b"truncated").is_err());
        assert!(decode_key("invalid").is_err());
    }
    #[test]
    fn encrypted_store_survives_reopen_without_plaintext() {
        let dir = std::env::temp_dir().join(format!("melody-store-test-{}", uuid::Uuid::new_v4()));
        let store = SecureJsonStore {
            path: dir.join("oauth-test.bin"),
            server_key: Some([9; 32]),
        };
        store.save(&vec!["synthetic"]).unwrap();
        assert_eq!(store.load::<Vec<String>>().unwrap(), vec!["synthetic"]);
        store.verify_existing().unwrap();
        store.remove().unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
