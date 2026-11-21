//! Windows DPAPI wrapper (CurrentUser scope), 100% compatible with C# ProtectedData.Protect/Unprotect
//! so existing %LOCALAPPDATA%\BBRadmin\config.ini passwords continue working seamlessly.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use std::ptr;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

pub fn protect(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    let bytes = value.as_bytes();
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &input,
            ptr::null(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            0,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() {
        return String::new();
    }
    let slice = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let encoded = format!("dpapi:{}", STANDARD.encode(slice));
    unsafe {
        LocalFree(output.pbData as _);
    }
    encoded
}

pub fn unprotect(value: &str) -> String {
    let Some(encoded) = value.strip_prefix("dpapi:") else {
        return String::new();
    };
    let Ok(bytes) = STANDARD.decode(encoded.trim()) else {
        return String::new();
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            0,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() {
        return String::new();
    }
    let slice = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let result = String::from_utf8(slice.to_vec()).unwrap_or_default();
    unsafe {
        LocalFree(output.pbData as _);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_dpapi() {
        let secret = "test-secret-1234";
        let enc = protect(secret);
        assert!(enc.starts_with("dpapi:"));
        assert_eq!(unprotect(&enc), secret);
    }
}
