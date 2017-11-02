# FFI Helpers

A crate exposing common functions helpers for doing proper error handling in a 
FFI context. 


## Error Handling Theory

This employs a thread-local variable which holds the most recent error as
well as some convenience functions for getting/clearing this variable. 

The theory is if a function fails then it should return an "obviously
invalid" value, this is commonly `-1` when returning an integer or `null`
when returning a pointer. The user can then check for this and consult the
most recent error for more information... Of course that means all fallible
operations must update the most recent error if they fail.

> **Note:** This error handling strategy is strongly influenced by libgit2]'s 
> error handling docs, ported to Rust. As such, it is **strongly recommended** 
> to skim the [error handling docs][docs] themselves.

[docs]: https://github.com/libgit2/libgit2/blob/master/docs/error-handling.md)