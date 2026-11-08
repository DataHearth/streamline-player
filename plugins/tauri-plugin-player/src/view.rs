//! Video under the page on Linux: the webview moves into a GtkOverlay whose bottom layer is a
//! GtkGLArea that libmpv draws into through its OpenGL render API. One window, one compositor
//! surface, so the page's controls and panels blend over the picture.

use crate::mpv::{self, RenderParam};
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    ffi::{c_char, c_int, c_void},
    ptr,
    sync::Arc,
};
use webkit2gtk::WebViewExt;

const PARAM_END: c_int = 0;
const PARAM_API_TYPE: c_int = 1;
const PARAM_GL_INIT: c_int = 2;
const PARAM_GL_FBO: c_int = 3;
const PARAM_FLIP_Y: c_int = 4;
const PARAM_WL_DISPLAY: c_int = 9;
const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;

#[repr(C)]
struct GlInitParams {
    get_proc_address: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    ctx: *mut c_void,
}

#[repr(C)]
struct GlFbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}

// ponytail: EGL only. GTK draws through EGL on Wayland; an X11 session (GLX) gets no video
// until a glXGetProcAddress path is added here.
#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetProcAddress(name: *const c_char) -> *mut c_void;
}

unsafe extern "C" {
    fn gdk_wayland_display_get_wl_display(display: *mut c_void) -> *mut c_void;
}

thread_local! {
    static AREA: RefCell<Option<gtk::GLArea>> = const { RefCell::new(None) };
    static RENDER: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
}

unsafe extern "C" fn proc_address(_: *mut c_void, name: *const c_char) -> *mut c_void {
    unsafe { eglGetProcAddress(name) }
}

// mpv calls this from its own threads; the GLArea belongs to the GTK one.
unsafe extern "C" fn on_update(_: *mut c_void) {
    glib::idle_add_once(|| {
        AREA.with(|a| {
            if let Some(a) = a.borrow().as_ref() {
                a.queue_render();
            }
        })
    });
}

fn wayland_display() -> *mut c_void {
    let Some(display) = gdk::Display::default() else {
        return ptr::null_mut();
    };
    if display.type_().name() != "GdkWaylandDisplay" {
        return ptr::null_mut();
    }
    use glib::translate::ToGlibPtr;
    let d: *mut gdk::ffi::GdkDisplay = display.to_glib_none().0;
    unsafe { gdk_wayland_display_get_wl_display(d.cast()) }
}

fn create_render_context(area: &gtk::GLArea, player: &mpv::Mpv) {
    area.make_current();
    if let Some(e) = area.error() {
        eprintln!("no GL context for the video: {e}");
        return;
    }
    let mut gl = GlInitParams {
        get_proc_address: proc_address,
        ctx: ptr::null_mut(),
    };
    // With the Wayland display, VA-API frames reach GL as dmabufs instead of being copied back.
    let wl = wayland_display();
    let mut params = vec![
        RenderParam {
            kind: PARAM_API_TYPE,
            data: c"opengl".as_ptr() as *mut c_void,
        },
        RenderParam {
            kind: PARAM_GL_INIT,
            data: (&raw mut gl).cast(),
        },
    ];
    if !wl.is_null() {
        params.push(RenderParam {
            kind: PARAM_WL_DISPLAY,
            data: wl,
        });
    }
    params.push(RenderParam {
        kind: PARAM_END,
        data: ptr::null_mut(),
    });
    let mut ctx = ptr::null_mut();
    let err =
        unsafe { mpv::mpv_render_context_create(&mut ctx, player.raw(), params.as_mut_ptr()) };
    if err < 0 {
        eprintln!("mpv render context failed: {err}");
        return;
    }
    unsafe { mpv::mpv_render_context_set_update_callback(ctx, Some(on_update), ptr::null_mut()) };
    RENDER.with(|r| r.set(ctx));
}

// The render context holds GL objects, so it goes before GTK drops the GL context.
fn free_render_context(area: &gtk::GLArea) {
    let ctx = RENDER.with(|r| r.replace(ptr::null_mut()));
    if !ctx.is_null() {
        area.make_current();
        unsafe { mpv::mpv_render_context_free(ctx) };
    }
}

fn render(area: &gtk::GLArea) -> glib::Propagation {
    let ctx = RENDER.with(Cell::get);
    if ctx.is_null() {
        return glib::Propagation::Stop;
    }
    let get_integer: unsafe extern "C" fn(u32, *mut c_int) =
        unsafe { std::mem::transmute(eglGetProcAddress(c"glGetIntegerv".as_ptr())) };
    let mut fbo = 0;
    unsafe { get_integer(GL_FRAMEBUFFER_BINDING, &mut fbo) };
    let scale = area.scale_factor();
    let mut target = GlFbo {
        fbo,
        w: area.allocated_width() * scale,
        h: area.allocated_height() * scale,
        internal_format: 0,
    };
    let mut flip: c_int = 1;
    let mut params = [
        RenderParam {
            kind: PARAM_GL_FBO,
            data: (&raw mut target).cast(),
        },
        RenderParam {
            kind: PARAM_FLIP_Y,
            data: (&raw mut flip).cast(),
        },
        RenderParam {
            kind: PARAM_END,
            data: ptr::null_mut(),
        },
    ];
    unsafe { mpv::mpv_render_context_render(ctx, params.as_mut_ptr()) };
    glib::Propagation::Stop
}

/// Puts the GL area under `webview`. Runs on the GTK thread.
pub fn attach(webview: webkit2gtk::WebView, player: Arc<mpv::Mpv>) {
    let Some(parent) = webview.parent().and_downcast::<gtk::Container>() else {
        eprintln!("the webview has no container; no video layer");
        return;
    };
    parent.remove(&webview);
    let overlay = gtk::Overlay::new();
    let area = gtk::GLArea::new();
    area.connect_realize(move |a| {
        create_render_context(a, &player);
        if let Some(hz) = refresh_rate(a) {
            let _ = player.set("display-fps-override", &hz.to_string());
        }
    });
    area.connect_unrealize(free_render_context);
    area.connect_render(|a, _| render(a));
    overlay.add(&area);
    overlay.add_overlay(&webview);
    match parent.downcast_ref::<gtk::Box>() {
        Some(b) => b.pack_start(&overlay, true, true, 0),
        None => parent.add(&overlay),
    }
    webview.set_background_color(&gdk::RGBA::new(0.0, 0.0, 0.0, 0.0));
    #[cfg(debug_assertions)]
    if let Some(settings) = WebViewExt::settings(&webview) {
        use webkit2gtk::SettingsExt;
        settings.set_enable_write_console_messages_to_stdout(true);
    }
    overlay.show_all();
    AREA.with(|a| *a.borrow_mut() = Some(area));
}

pub fn refresh_rate(widget: &impl IsA<gtk::Widget>) -> Option<f64> {
    let window = widget
        .as_ref()
        .window()
        .or_else(|| widget.as_ref().toplevel()?.window())?;
    let monitor = window.display().monitor_at_window(&window)?;
    let mhz = monitor.refresh_rate();
    (mhz > 0).then(|| f64::from(mhz) / 1000.0)
}
