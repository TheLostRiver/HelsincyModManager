#![cfg(windows)]

use windows::core::{HRESULT, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, LocalFree, ERROR_INSUFFICIENT_BUFFER, HANDLE, HLOCAL,
};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TokenUser, PSID, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub(crate) fn current_process_user_sid() -> windows::core::Result<String> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)? };
    let token = OwnedHandle(token);

    let mut required_bytes = 0_u32;
    let first = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut required_bytes) };
    let first_error = match first {
        Err(error) => error,
        Ok(()) => return Err(windows::core::Error::from_win32()),
    };
    if first_error.code() != HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0)
        || required_bytes < std::mem::size_of::<TOKEN_USER>() as u32
    {
        return Err(first_error);
    }

    let word_bytes = std::mem::size_of::<usize>();
    let word_count = (required_bytes as usize).div_ceil(word_bytes);
    let mut token_user = vec![0_usize; word_count];
    let buffer_bytes = u32::try_from(token_user.len() * word_bytes)
        .map_err(|_| windows::core::Error::from_win32())?;
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(token_user.as_mut_ptr().cast()),
            buffer_bytes,
            &mut required_bytes,
        )?;
    }
    let token_user = unsafe { &*token_user.as_ptr().cast::<TOKEN_USER>() };
    sid_to_string(token_user.User.Sid)
}

pub(crate) fn sid_to_string(sid: PSID) -> windows::core::Result<String> {
    if sid.is_invalid() {
        return Err(windows::core::Error::from_win32());
    }
    let mut sid_text = PWSTR::null();
    unsafe { ConvertSidToStringSidW(sid, &mut sid_text)? };
    let sid_text = LocalWideString(sid_text);
    unsafe { sid_text.0.to_string() }.map_err(|_| windows::core::Error::from_win32())
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

struct LocalWideString(PWSTR);

impl Drop for LocalWideString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = LocalFree(Some(HLOCAL(self.0.as_ptr().cast())));
            }
        }
    }
}
