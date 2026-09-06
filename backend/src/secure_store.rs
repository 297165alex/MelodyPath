use anyhow::{Context, Result, bail};
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
}

impl SecureJsonStore {
    pub fn for_oauth_provider(provider: &str) -> Option<Self> {
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
        Some(Self {
            path: base
                .join("MelodyPath")
                .join(format!("oauth-{provider}.bin")),
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
}

impl OAuthTokenStore for SecureJsonStore {
    fn backend_name(&self) -> &'static str {
        "windows_dpapi_user_scope"
    }

    fn load_cleartext(&self) -> Result<Vec<u8>> {
        let encrypted = fs::read(&self.path).context("无法读取本地加密 OAuth 会话")?;
        unprotect(&encrypted)
    }

    fn save_cleartext(&self, cleartext: &[u8]) -> Result<()> {
        let encrypted = protect(cleartext)?;
        let parent = self.path.parent().context("OAuth 会话目录无效")?;
        fs::create_dir_all(parent).context("无法创建 OAuth 会话目录")?;
        fs::write(&self.path, encrypted).context("无法写入本地加密 OAuth 会话")?;
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
