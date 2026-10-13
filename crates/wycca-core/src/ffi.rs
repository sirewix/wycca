//! C ABI entry points. The browser build calls these directly (no wasm-bindgen,
//! no wasm-pack); they compile everywhere, so the same surface is unit tested here.
//!
//! JS side, before calling:
//! ```js
//! globalThis.Date_now = () => Date.now();
//! ```
//!
//! Contract: `wycca_render` then `wycca_len` then `wycca_free`, in that order.

use std::{
    alloc::{Layout, alloc, dealloc},
    sync::atomic::{AtomicUsize, Ordering},
};

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn Date_now() -> f64;
}

/// The one system-clock read, and wasm32-unknown-unknown cannot do it for us.
#[cfg(target_arch = "wasm32")]
pub fn now_ms() -> u64 {
    unsafe { Date_now() as u64 }
}

static LEN: AtomicUsize = AtomicUsize::new(0);

/// # Safety
/// `json` must point to `len` readable bytes of valid UTF-8. The returned pointer
/// is owned by the caller until `wycca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wycca_render(json: *const u8, len: usize) -> *mut u8 {
    let json = unsafe { std::str::from_utf8(std::slice::from_raw_parts(json, len)) };
    let cfg = json
        .map_err(|e| e.to_string())
        .and_then(|j| crate::parse(j).map_err(|e| e.to_string()));
    let svg = match cfg {
        Ok(cfg) => crate::render(&cfg, crate::now_ms()),
        Err(e) => format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><desc>{e}</desc></svg>"),
    };
    let (ptr, layout) = leak(&svg);
    LEN.store(layout.size(), Ordering::Relaxed);
    ptr
}

fn leak(s: &str) -> (*mut u8, Layout) {
    let bytes = s.as_bytes();
    let layout = Layout::from_size_align(bytes.len(), 1).unwrap();
    let ptr = unsafe { alloc(layout) };
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
    (ptr, layout)
}

/// # Safety
/// Must be the pointer from the immediately preceding `wycca_render`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wycca_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let layout = unsafe { Layout::from_size_align_unchecked(LEN.load(Ordering::Relaxed), 1) };
    unsafe { dealloc(ptr, layout) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_round_trips_through_the_c_abi() {
        let json = br#"{"size":256,"rings":[{"type":"week"}]}"#;
        let ptr = unsafe { wycca_render(json.as_ptr(), json.len()) };
        let len = LEN.load(Ordering::Relaxed);
        let svg = unsafe { std::str::from_utf8(std::slice::from_raw_parts(ptr, len)) }.unwrap();
        assert!(svg.contains("<path") && svg.contains("width=\"256\""));
        unsafe { wycca_free(ptr) };
    }
}
