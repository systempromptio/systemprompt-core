//! DPAPI in the current-user scope: sealing and opening with an entropy blob,
//! never prompting, and wiping DPAPI's output buffer before it is freed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    unsafe_code,
    reason = "CryptProtectData and CryptUnprotectData are raw Win32 FFI"
)]

use std::io;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{ERROR_INVALID_DATA, NTE_BAD_DATA, NTE_BAD_KEY_STATE};
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use zeroize::{Zeroize, Zeroizing};

use super::ffi::{LocalAllocation, checked};

// Why: DPAPI answers with these codes for a blob that is corrupt, sealed with
// other entropy, or sealed for another user or a master key this user no longer
// has; every other failure is treated as transient.
pub(crate) fn is_foreign_ciphertext(error: &io::Error) -> bool {
    error.raw_os_error().is_some_and(|code| {
        code == ERROR_INVALID_DATA.cast_signed()
            || code == NTE_BAD_DATA
            || code == NTE_BAD_KEY_STATE
    })
}

fn blob(bytes: &[u8]) -> io::Result<CRYPT_INTEGER_BLOB> {
    let length = u32::try_from(bytes.len())
        .map_err(|_too_long| io::Error::new(io::ErrorKind::InvalidInput, "blob exceeds 4 GiB"))?;
    Ok(CRYPT_INTEGER_BLOB {
        cbData: length,
        pbData: bytes.as_ptr().cast_mut(),
    })
}

// SAFETY: callers pass the `pDataOut` of a successful `CryptProtectData` /
// `CryptUnprotectData` call that nothing else frees; the buffer is copied out,
// wiped and freed here.
unsafe fn take_output(output: &CRYPT_INTEGER_BLOB) -> Zeroizing<Vec<u8>> {
    let allocation = LocalAllocation(output.pbData.cast());
    if output.pbData.is_null() {
        return Zeroizing::new(Vec::new());
    }
    // SAFETY: per this function's contract, `pbData` points at `cbData` bytes DPAPI
    // allocated for this call; they are copied, wiped in place, then freed by
    // `allocation`.
    let copy = unsafe {
        let bytes = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let copy = Zeroizing::new(bytes.to_vec());
        bytes.zeroize();
        copy
    };
    drop(allocation);
    copy
}

pub(crate) fn dpapi_protect(plaintext: &[u8], entropy: &[u8]) -> io::Result<Vec<u8>> {
    let input = blob(plaintext)?;
    let entropy_blob = blob(entropy)?;
    let entropy_ptr: *const CRYPT_INTEGER_BLOB = if entropy.is_empty() {
        null()
    } else {
        &raw const entropy_blob
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    // SAFETY: the input and entropy blobs point at slices that outlive the call and
    // DPAPI only reads them; the output slot is a live local, taken over by
    // `take_output` on success.
    unsafe {
        checked(CryptProtectData(
            &raw const input,
            null(),
            entropy_ptr,
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        ))?;
        Ok(take_output(&output).to_vec())
    }
}


pub(crate) fn dpapi_unprotect(ciphertext: &[u8], entropy: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
    let input = blob(ciphertext)?;
    let entropy_blob = blob(entropy)?;
    let entropy_ptr: *const CRYPT_INTEGER_BLOB = if entropy.is_empty() {
        null()
    } else {
        &raw const entropy_blob
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    // SAFETY: as in `dpapi_protect`; no description is requested, so DPAPI
    // allocates nothing besides the output, which `take_output` wipes and
    // frees.
    unsafe {
        checked(CryptUnprotectData(
            &raw const input,
            null_mut(),
            entropy_ptr,
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        ))?;
        Ok(take_output(&output))
    }
}
