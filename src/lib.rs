//! A crate exposing common functions for doing proper error handling in a FFI
//! context.
//!
//! ## Theory
//!
//! This employs a thread-local variable which holds the most recent error as
//! well as some convenience functions for getting/clearing this variable.
//!
//! The theory is if a function fails then it should return an "obviously
//! invalid" value, this is commonly `-1` when returning an integer or `null`
//! when returning a pointer. The user can then check for this and consult the
//! most recent error for more information... Of course that means all fallible
//! operations must update the most recent error if they fail.
//!
//! > **Note:** This error handling strategy is strongly influenced by libgit2]'s
//! > error handling docs, ported to Rust. As such, it is **strongly recommended**
//! > to skim the [error handling docs][docs] themselves.
//!
//! [docs]: https://github.com/libgit2/libgit2/blob/master/docs/error-handling.md)

extern crate libc;

use std::error::Error;
use std::cell::RefCell;
use std::ptr;
use std::slice;
use libc::{c_char, c_int};


thread_local!(
    static LAST_ERROR: RefCell<Option<Box<Error>>> = RefCell::new(None);
);

/// Set the thread-local `LAST_ERROR` variable.
pub fn update_last_error<E: Error + 'static>(e: E) {
    let boxed = Box::new(e);

    LAST_ERROR.with(|last| {
        *last.borrow_mut() = Some(boxed);
    });
}

/// Get the last error, clearing the variable in the process.
pub fn get_last_error() -> Option<Box<Error>> {
    LAST_ERROR.with(|last| last.borrow_mut().take())
}


#[no_mangle]
pub unsafe extern "C" fn error_message(buffer: *mut c_char, length: c_int) -> c_int {
    let buffer = slice::from_raw_parts_mut(buffer as *mut u8, length as usize);

    // Take the last error, if there isn't one then there's no error message to
    // display.
    let err = match get_last_error() {
        Some(e) => e,
        None => return 0,
    };

    let error_message = format!("{}", err);
    let bytes_required = error_message.len() + 1;

    if buffer.len() < bytes_required {
        return -1;
    }

    let data = error_message.as_bytes();
    ptr::copy_nonoverlapping(data.as_ptr(), buffer.as_mut_ptr(), data.len());

    // zero out the rest of the buffer just in case
    let rest = &mut buffer[data.len()..];
    ptr::write_bytes(rest.as_mut_ptr(), 0, rest.len());

    data.len() as c_int
}
