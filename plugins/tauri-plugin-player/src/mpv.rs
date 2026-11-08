//! The slice of libmpv's client and render API the desktop player uses.

use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr,
};

#[repr(C)]
pub struct RenderParam {
    pub kind: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
struct RawEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct RawEndFile {
    reason: c_int,
    error: c_int,
}

const EVENT_SHUTDOWN: c_int = 1;
const EVENT_END_FILE: c_int = 7;
const EVENT_FILE_LOADED: c_int = 8;
const END_FILE_REASON_ERROR: c_int = 4;
const LC_NUMERIC: c_int = 1;

#[link(name = "mpv")]
unsafe extern "C" {
    fn mpv_create() -> *mut c_void;
    fn mpv_initialize(h: *mut c_void) -> c_int;
    fn mpv_set_option_string(h: *mut c_void, name: *const c_char, value: *const c_char) -> c_int;
    fn mpv_set_property_string(h: *mut c_void, name: *const c_char, value: *const c_char) -> c_int;
    fn mpv_get_property_string(h: *mut c_void, name: *const c_char) -> *mut c_char;
    fn mpv_command(h: *mut c_void, args: *mut *const c_char) -> c_int;
    fn mpv_free(data: *mut c_void);
    fn mpv_error_string(error: c_int) -> *const c_char;
    fn mpv_wait_event(h: *mut c_void, timeout: f64) -> *mut RawEvent;
    pub fn mpv_render_context_create(
        out: *mut *mut c_void,
        h: *mut c_void,
        params: *mut RenderParam,
    ) -> c_int;
    pub fn mpv_render_context_set_update_callback(
        ctx: *mut c_void,
        cb: Option<unsafe extern "C" fn(*mut c_void)>,
        cb_ctx: *mut c_void,
    );
    pub fn mpv_render_context_render(ctx: *mut c_void, params: *mut RenderParam) -> c_int;
    pub fn mpv_render_context_free(ctx: *mut c_void);
}

unsafe extern "C" {
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
}

pub enum Event {
    Nothing,
    FileLoaded,
    /// A file stopped for good; `Some` when it failed.
    EndFile(Option<String>),
    Shutdown,
}

pub struct Mpv(*mut c_void);

// libmpv's client API is thread-safe: any thread may call it on the same handle.
unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

fn error_string(code: c_int) -> String {
    unsafe { CStr::from_ptr(mpv_error_string(code)) }
        .to_string_lossy()
        .into_owned()
}

fn check(code: c_int) -> Result<(), String> {
    if code < 0 {
        Err(error_string(code))
    } else {
        Ok(())
    }
}

impl Mpv {
    pub fn new(options: &[(&str, &str)]) -> Result<Self, String> {
        // libmpv refuses to start under a non-C LC_NUMERIC, and gtk_init sets the user's locale.
        unsafe { setlocale(LC_NUMERIC, c"C".as_ptr()) };
        let h = unsafe { mpv_create() };
        if h.is_null() {
            return Err("mpv_create failed".into());
        }
        let mpv = Mpv(h);
        for (k, v) in options {
            let (k, v) = (CString::new(*k).unwrap(), CString::new(*v).unwrap());
            check(unsafe { mpv_set_option_string(h, k.as_ptr(), v.as_ptr()) })
                .map_err(|e| format!("{}: {e}", k.to_string_lossy()))?;
        }
        check(unsafe { mpv_initialize(h) })?;
        Ok(mpv)
    }

    pub fn raw(&self) -> *mut c_void {
        self.0
    }

    pub fn get(&self, name: &str) -> Option<String> {
        let name = CString::new(name).ok()?;
        let raw = unsafe { mpv_get_property_string(self.0, name.as_ptr()) };
        if raw.is_null() {
            return None;
        }
        let value = unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        unsafe { mpv_free(raw.cast()) };
        Some(value)
    }

    pub fn number(&self, name: &str) -> Option<f64> {
        self.get(name)?.parse().ok()
    }

    pub fn flag(&self, name: &str) -> bool {
        self.get(name).as_deref() == Some("yes")
    }

    pub fn set(&self, name: &str, value: &str) -> Result<(), String> {
        let (n, v) = (
            CString::new(name).unwrap(),
            CString::new(value).map_err(|e| e.to_string())?,
        );
        check(unsafe { mpv_set_property_string(self.0, n.as_ptr(), v.as_ptr()) })
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let owned: Vec<CString> = args
            .iter()
            .map(|a| CString::new(*a))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|a| a.as_ptr()).collect();
        ptrs.push(ptr::null());
        check(unsafe { mpv_command(self.0, ptrs.as_mut_ptr()) })
    }

    /// Only one thread may wait for events.
    pub fn wait_event(&self, timeout: f64) -> Event {
        let e = unsafe { &*mpv_wait_event(self.0, timeout) };
        match e.event_id {
            EVENT_FILE_LOADED => Event::FileLoaded,
            EVENT_END_FILE => {
                let end = unsafe { &*(e.data as *const RawEndFile) };
                Event::EndFile(
                    (end.reason == END_FILE_REASON_ERROR).then(|| error_string(end.error)),
                )
            }
            EVENT_SHUTDOWN => Event::Shutdown,
            _ => Event::Nothing,
        }
    }
}
