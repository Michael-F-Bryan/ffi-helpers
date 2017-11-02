//! A crate exposing common functions helpers for doing proper error handling in a
//! FFI context.
//!
//!
//! ## Error Handling Theory
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
use std::panic::{self, UnwindSafe};
use std::any::Any;
use libc::{c_char, c_int};


thread_local!(
    static LAST_ERROR: RefCell<Option<Box<Error>>> = RefCell::new(None);
);

/// Set the thread-local `LAST_ERROR` variable.
pub fn update_last_error<E: Into<Box<Error>> + 'static>(e: E) {
    let boxed = e.into();

    LAST_ERROR.with(|last| {
        *last.borrow_mut() = Some(boxed);
    });
}

/// Get the last error, clearing the variable in the process.
pub fn get_last_error() -> Option<Box<Error>> {
    LAST_ERROR.with(|last| last.borrow_mut().take())
}


/// Write the latest error message to a buffer.
///
/// # Returns
///
/// This returns the number of bytes written to the buffer. If no bytes were
/// written (i.e. there is no last error) then it returns `0`. If the buffer
/// isn't big enough or a `null` pointer was passed in, you'll get a `-1`.
#[no_mangle]
pub unsafe extern "C" fn error_message(buffer: *mut c_char, length: c_int) -> c_int {
    if buffer.is_null() {
        return -1;
    }

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
        // We don't have enough room. Make sure to return the error so it
        // isn't accidentally consumed
        update_last_error(err);
        return -1;
    }

    let data = error_message.as_bytes();
    ptr::copy_nonoverlapping(data.as_ptr(), buffer.as_mut_ptr(), data.len());

    // zero out the rest of the buffer just in case
    let rest = &mut buffer[data.len()..];
    ptr::write_bytes(rest.as_mut_ptr(), 0, rest.len());

    data.len() as c_int
}

/// Execute some closure, catching any panics and converting them into errors
/// so you don't accidentally unwind across the FFI boundary.
///
/// # Note
///
/// It will need to be possible to convert an opaque `Box<Any + Send + 'static>`
/// received from [`std::panic::catch_unwind()`][cu] back into your error type.
///
/// If you are using [error-chain] then you can leverage the `error_chain!()`
/// macro to generate some of this for you.
///
/// ```ignore
/// error_chain!{
///     ...
///     errors {
///         Panic(inner: Box<::std::any::Any + Send + 'static>) {
///             description("Thread Panicked")
///                 display("{}",
///                         if let Some(s) = inner.downcast_ref::<String>() {
///                             s.clone()
///                         } else if let Some(s) = inner.downcast_ref::<&str>() {
///                             s.to_string()
///                         } else {
///                             String::from("Thread Panicked")
///                         })
///         }
///     }
/// }
/// }
/// ```
///
/// When converting from a `Box<Any + Send + 'static>`, the best way to try and
/// recover the panic message is to use `Any::downcast_ref()` to try various
/// "common" panic message types. Falling back to some sane default if we can't
/// figure it out. Luckily almost all panic messages are either `&str` or
/// `String`.
///
///
/// # Examples
///
/// This is a basic example of how you may use `catch_panic()`. It looks a
/// little long because you need to define a way to convert a panic message into
/// your error type, but that's a one-time cost and the `catch_panic()` call
/// itself is trivial.
///
/// ```
/// use std::any::Any;
/// extern crate ffi_helpers;
///
/// fn main() {
///   let got: Result<u32, Error> = ffi_helpers::catch_panic(|| {
///       let something  = None;
///       something.unwrap()
///   });
///
///   let message = format!("{:?}", got);
///   assert_eq!(message, r#"Err(Message("called `Option::unwrap()` on a `None` value"))"#);
/// }
///
/// #[derive(Debug)]
/// enum Error {
///   Message(String),
///   Unknown,
/// }
///
/// impl From<Box<Any + Send + 'static>> for Error {
///   fn from(other: Box<Any + Send + 'static>) -> Error {
///     if let Some(owned) = other.downcast_ref::<String>() {
///       Error::Message(owned.clone())
///     } else if let Some(owned) = other.downcast_ref::<String>() {
///       Error::Message(owned.to_string())
///     } else {
///       Error::Unknown
///     }
///   }
/// }
/// ```
///
/// [cu]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
/// [error-chain]: https://crates.io/crates/error-chain
pub fn catch_panic<T, E, F>(func: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E> + UnwindSafe,
    E: From<Box<Any + Send + 'static>>,
{
    panic::catch_unwind(func)
        .map_err(Into::into)
        .and_then(|t| t)
}
