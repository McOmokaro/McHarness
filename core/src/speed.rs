//! Safe FFI boundary to the C++ speed organs.
//!
//! All unsafe lives here and only here. The rest of the core
//! consumes these as safe functions.

use std::ffi::{c_char, c_double, c_int, CStr, CString};
use std::os::raw::c_float;

#[link(name = "mcharness_speed")]
extern "C" {
    fn mh_stream_push(token: *const c_char, out: *mut c_char, out_cap: u64) -> u64;
    fn mh_memsearch_score(query: *const c_char, entries: *const *const c_char, count: u64) -> i64;
    fn mh_embed_cosine(a: *const c_float, b: *const c_float, dim: u64) -> c_double;
}

/// Push a token through the streaming organ.
pub fn stream_push(token: &str, out: &mut [u8]) -> usize {
    let token_c = CString::new(token).expect("no interior nul");
    let mut probe = out.as_mut_ptr() as *mut c_char;
    let cap = out.len() as u64;
    probe = probe; // pointer stays stable; call the organ
    unsafe { mh_stream_push(token_c.as_ptr(), out.as_mut_ptr() as *mut c_char, cap) as usize }
}

/// Fast memory search: best-matching index for a query, if any.
pub fn memsearch(query: &str, entries: &[&str]) -> Option<usize> {
    let query_c = CString::new(query).expect("no interior nul");
    let entry_cs: Vec<CString> = entries.iter().map(|e| CString::new(*e).expect("no interior nul")).collect();
    let ptrs: Vec<*const c_char> = entry_cs.iter().map(|e| e.as_ptr()).collect();
    let idx = unsafe { mh_memsearch_score(query_c.as_ptr(), ptrs.as_ptr(), ptrs.len() as u64) };
    if idx < 0 { None } else { Some(idx as usize) }
}

/// Cosine similarity via the embedding organ.
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    assert_eq!(a.len(), b.len(), "embedding dims must match");
    unsafe { mh_embed_cosine(a.as_ptr(), b.as_ptr(), a.len() as u64) }
}
