//! C ABI entry points. The browser build calls these directly (no wasm-bindgen,
//! no wasm-pack); they compile everywhere, so the same surface is unit tested here.
//!
//! The caller passes the timestamp. wasm32-unknown-unknown has no clock we could
//! import without also linking an undefined symbol, and `render` already takes
//! `t_ms`, so the browser just hands it `Date.now() + utc_offset_ms`.
//!
//! Every function that returns a pointer owns it until `wycca_free`, whose length
//! comes from `wycca_len`. The one exception is `wycca_scratch`.

use crate::Layer;
use std::{
    alloc::{Layout, alloc, dealloc},
    sync::atomic::{AtomicUsize, Ordering},
};

static LEN: AtomicUsize = AtomicUsize::new(0);

/// The built-in config, so the page has a starting point without keeping a copy
/// of it that can drift from this file.
#[unsafe(no_mangle)]
pub extern "C" fn wycca_default() -> *mut u8 {
    owned(&serde_json::to_string(&crate::Config::default()).unwrap())
}

/// The configuration form, described as data. See [`crate::ui`].
#[unsafe(no_mangle)]
pub extern "C" fn wycca_form() -> *mut u8 {
    owned(&serde_json::to_string(&crate::ui::form()).unwrap())
}

/// One ring at `scale` as JSON, with every layer filled in from
/// [`crate::Ring::new`]. The form describes a ring's controls but not their
/// starting values, so adding a ring goes through here rather than through a
/// half-written object in the UI.
///
/// # Safety
/// `scale` must point to `len` readable bytes of UTF-8 naming a scale.
#[unsafe(no_mangle)]
pub extern "C" fn wycca_ring(scale: *const u8, len: usize) -> *mut u8 {
    let scale = unsafe { std::str::from_utf8(std::slice::from_raw_parts(scale, len)) };
    let json = match scale {
        Ok("hour24") => serde_json::to_string(&crate::Ring::new(crate::Scale::Hour24)).unwrap(),
        Ok("week") => serde_json::to_string(&crate::Ring::new(crate::Scale::Week)).unwrap(),
        Ok("month") => serde_json::to_string(&crate::Ring::new(crate::Scale::Month)).unwrap(),
        Ok("year") => serde_json::to_string(&crate::Ring::new(crate::Scale::Year)).unwrap(),
        _ => "<ring>unknown scale</ring>".into(),
    };
    owned(&json)
}

/// One layer of `kind` as JSON, with every field filled in from
/// [`crate::Layer::new`]. Same reasoning as `wycca_ring`, one level down: the
/// form says which controls a layer has, not what they start at.
///
/// # Safety
/// `kind` must point to `len` readable bytes of UTF-8 naming a layer kind.
#[unsafe(no_mangle)]
pub extern "C" fn wycca_layer(kind: *const u8, len: usize) -> *mut u8 {
    let kind = unsafe { std::str::from_utf8(std::slice::from_raw_parts(kind, len)) };
    let json = match kind.ok().and_then(Layer::new) {
        Some(layer) => serde_json::to_string(&layer).unwrap(),
        None => "<layer>unknown kind</layer>".into(),
    };
    owned(&json)
}

/// What to call a layer in the UI: its source, spelled the way the config spells
/// it. The core owns the words so a card's title and its fields cannot disagree.
///
/// # Safety
/// `json` must point to `len` readable bytes of a serialised layer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wycca_title(json: *const u8, len: usize) -> *mut u8 {
    let json = unsafe { std::str::from_utf8(std::slice::from_raw_parts(json, len)) };
    owned(&match json
        .ok()
        .and_then(|j| serde_json::from_str::<Layer>(j).ok())
    {
        Some(layer) => layer.title(),
        None => String::new(),
    })
}

/// Every whole-day event in an .ics feed, as a JSON array. The page puts it in
/// `Config::feeds` exactly as the CLI's `--events` does, under the same name.
///
/// # Safety
/// `ics` must point to `len` readable bytes of UTF-8, and the buffer must be at
/// least `len` bytes of the scratch from `wycca_scratch`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wycca_events(ics: *const u8, len: usize) -> *mut u8 {
    let ics = unsafe { std::str::from_utf8(std::slice::from_raw_parts(ics, len)) };
    owned(&match ics {
        Ok(ics) => serde_json::to_string(&crate::events::parse(ics)).unwrap(),
        Err(_) => "[]".into(),
    })
}

/// Somewhere to put the JSON a caller passes to `wycca_render`, since a caller
/// cannot write into wasm memory itself. Not tracked by `wycca_free`: allocate
/// one per page and keep it.
///
/// # Safety
/// The caller keeps the returned buffer and must not pass it to `wycca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wycca_scratch(len: usize) -> *mut u8 {
    let layout = Layout::from_size_align(len.max(1), 1).unwrap();
    unsafe { alloc(layout) }
}

/// Render `json` at `t_ms`, a wall clock timestamp. Same input, same bytes.
///
/// # Safety
/// `json` must point to `len` readable bytes of valid UTF-8. The returned pointer
/// is owned by the caller until `wycca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wycca_render(json: *const u8, len: usize, t_ms: f64) -> *mut u8 {
    let json = unsafe { std::str::from_utf8(std::slice::from_raw_parts(json, len)) };
    let cfg = json
        .map_err(|e| e.to_string())
        .and_then(|j| crate::parse(j).map_err(|e| e.to_string()));
    owned(&match cfg {
        Ok(cfg) => crate::render(&cfg, t_ms.max(0.0) as u64),
        Err(e) => format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><desc>{e}</desc></svg>"),
    })
}

fn leak(s: &str) -> (*mut u8, Layout) {
    let bytes = s.as_bytes();
    let layout = Layout::from_size_align(bytes.len(), 1).unwrap();
    let ptr = unsafe { alloc(layout) };
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
    (ptr, layout)
}

fn owned(s: &str) -> *mut u8 {
    let (ptr, layout) = leak(s);
    LEN.store(layout.size(), Ordering::Relaxed);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn wycca_len() -> usize {
    LEN.load(Ordering::Relaxed)
}

/// # Safety
/// Must be a pointer from the immediately preceding call that returned one, and
/// `wycca_len` must not have been called since.
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

    /// Read the buffer the last call left behind, the way the browser does.
    fn take(ptr: *mut u8) -> String {
        let len = wycca_len();
        let s = unsafe { std::str::from_utf8(std::slice::from_raw_parts(ptr, len)) }.unwrap();
        let s = s.to_owned();
        unsafe { wycca_free(ptr) };
        s
    }

    #[test]
    fn render_round_trips_through_the_c_abi() {
        let json = br#"{"size":256,"rings":[{"scale":"week","layers":[
            {"kind":"periodic","unit":"day","amount":1,"shift":0,"disp":{"background":"muted"}},
            {"kind":"periodic","unit":"day","amount":2,"every":7,"shift":5,"disp":{"background":"weekend"}}
        ]}]}"#;
        let ptr = unsafe { wycca_render(json.as_ptr(), json.len(), 1_800_000_000_000.0) };
        let svg = take(ptr);
        assert!(svg.contains("<path") && svg.contains("width=\"256\""));
    }

    #[test]
    fn the_page_can_ask_for_a_default_config_a_form_a_ring_and_a_layer() {
        let config: serde_json::Value = serde_json::from_str(&take(wycca_default())).unwrap();
        assert_eq!(config["rings"][0]["scale"], "hour24");

        let form: serde_json::Value = serde_json::from_str(&take(wycca_form())).unwrap();
        assert!(form["sections"].as_array().unwrap().len() >= 4);
        assert!(form["layer_fields"]["periodic"].as_array().unwrap().len() > 4);

        let ring: serde_json::Value =
            serde_json::from_str(&take(wycca_ring(b"week".as_ptr(), 4))).unwrap();
        assert_eq!(ring["scale"], "week");
        assert_eq!(ring["thickness"], 0.04, "a new ring starts at its Default");

        let layer = take(wycca_layer(b"solar".as_ptr(), 5));
        let title = take(unsafe { wycca_title(layer.as_ptr(), layer.len()) });
        assert_eq!(title, "daylight", "the core names its own layers");
    }

    #[test]
    fn a_bad_config_says_so_in_the_svg() {
        let json = br#"{"rings":[{"scale":"nope"}]}"#;
        let ptr = unsafe { wycca_render(json.as_ptr(), json.len(), 0.0) };
        assert!(take(ptr).contains("<desc>"));
    }
}
