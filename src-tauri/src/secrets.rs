//! API Key 的本地保护：Windows DPAPI（CryptProtectData / CryptUnprotectData）。
//!
//! 密文以 hex 存在 settings.json 的 `api_key_enc`；明文只在进程内使用，不落盘、不进日志。

use std::ffi::c_void;
use std::ptr::null_mut;

#[repr(C)]
struct DataBlob {
    cb_data: u32,
    pb_data: *mut u8,
}

#[cfg(windows)]
#[link(name = "crypt32")]
extern "system" {
    fn CryptProtectData(
        p_data_in: *const DataBlob,
        sz_data_descr: *const u16,
        p_optional_entropy: *const DataBlob,
        pv_reserved: *mut c_void,
        p_prompt_struct: *mut c_void,
        dw_flags: u32,
        p_data_out: *mut DataBlob,
    ) -> i32;
    fn CryptUnprotectData(
        p_data_in: *const DataBlob,
        ppsz_data_descr: *mut *mut u16,
        p_optional_entropy: *const DataBlob,
        pv_reserved: *mut c_void,
        p_prompt_struct: *mut c_void,
        dw_flags: u32,
        p_data_out: *mut DataBlob,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn LocalFree(h_mem: *mut c_void) -> *mut c_void;
}

const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
    #[cfg(not(windows))]
    {
        let _ = data;
        return None;
    }
    #[cfg(windows)]
    unsafe {
        let mut input = DataBlob {
            cb_data: data.len() as u32,
            pb_data: data.as_ptr() as *mut u8,
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: null_mut(),
        };
        let ok = CryptProtectData(
            &mut input,
            std::ptr::null(),
            std::ptr::null(),
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 {
            return None;
        }
        let bytes = std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec();
        LocalFree(output.pb_data as *mut c_void);
        Some(bytes)
    }
}

pub fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    #[cfg(not(windows))]
    {
        let _ = data;
        return None;
    }
    #[cfg(windows)]
    unsafe {
        let mut input = DataBlob {
            cb_data: data.len() as u32,
            pb_data: data.as_ptr() as *mut u8,
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: null_mut(),
        };
        let ok = CryptUnprotectData(
            &mut input,
            null_mut(),
            std::ptr::null(),
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 {
            return None;
        }
        let bytes = std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec();
        LocalFree(output.pb_data as *mut c_void);
        Some(bytes)
    }
}

pub fn to_hex(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 2);
    for b in data {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

/// 把 API Key 加密成可落盘的 hex；空字符串返回空。
pub fn encrypt_key(key: &str) -> Option<String> {
    if key.trim().is_empty() {
        return Some(String::new());
    }
    let protected = protect(key.as_bytes())?;
    Some(to_hex(&protected))
}

/// 从 hex 解出明文 Key；未设置/解密失败返回 None。
pub fn decrypt_key(hex: &str) -> Option<String> {
    if hex.trim().is_empty() {
        return None;
    }
    let bytes = from_hex(hex)?;
    let plain = unprotect(&bytes)?;
    String::from_utf8(plain).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let raw = vec![0u8, 1, 2, 250, 255];
        assert_eq!(from_hex(&to_hex(&raw)).unwrap(), raw);
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_roundtrip() {
        let encrypted = encrypt_key("sk-test-123456").unwrap();
        assert!(!encrypted.is_empty());
        assert_eq!(decrypt_key(&encrypted).unwrap(), "sk-test-123456");
    }
}
