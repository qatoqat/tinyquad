#![allow(dead_code, non_snake_case, clippy::upper_case_acronyms)]

use super::{X11Error, libx11::*};

use crate::native::module;

pub type GLXContext = *mut ();
pub type GLXFBConfig = *mut ();
pub type GLXWindow = XID;
pub type GLXDrawable = XID;

pub const GLX_VENDOR: libc::c_int = 1 as libc::c_int;
pub const GLX_RENDER_TYPE: libc::c_int = 0x8011 as libc::c_int;
pub const GLX_RGBA_BIT: libc::c_int = 0x1 as libc::c_int;
pub const GLX_DRAWABLE_TYPE: libc::c_int = 0x8010 as libc::c_int;
pub const GLX_WINDOW_BIT: libc::c_int = 0x1 as libc::c_int;
pub const GLX_RED_SIZE: libc::c_int = 8 as libc::c_int;
pub const GLX_GREEN_SIZE: libc::c_int = 9 as libc::c_int;
pub const GLX_BLUE_SIZE: libc::c_int = 10 as libc::c_int;
pub const GLX_ALPHA_SIZE: libc::c_int = 11 as libc::c_int;
pub const GLX_DEPTH_SIZE: libc::c_int = 12 as libc::c_int;
pub const GLX_STENCIL_SIZE: libc::c_int = 13 as libc::c_int;
pub const GLX_DOUBLEBUFFER: libc::c_int = 5 as libc::c_int;
pub const GLX_SAMPLES: libc::c_int = 0x186a1 as libc::c_int;

pub const GLX_CONTEXT_MAJOR_VERSION_ARB: libc::c_int = 0x2091 as libc::c_int;
pub const GLX_CONTEXT_MINOR_VERSION_ARB: libc::c_int = 0x2092 as libc::c_int;
pub const GLX_CONTEXT_PROFILE_MASK_ARB: libc::c_int = 0x9126 as libc::c_int;
pub const GLX_CONTEXT_CORE_PROFILE_BIT_ARB: libc::c_int = 0x1 as libc::c_int;
pub const GLX_CONTEXT_FLAGS_ARB: libc::c_int = 0x2094 as libc::c_int;
pub const GLX_CONTEXT_FORWARD_COMPATIBLE_BIT_ARB: libc::c_int = 0x2 as libc::c_int;

pub type GLenum = ::core::ffi::c_uint;
pub type GLboolean = ::core::ffi::c_uchar;
pub type GLbitfield = ::core::ffi::c_uint;
pub type GLvoid = ::core::ffi::c_void;
pub type GLbyte = ::core::ffi::c_schar;
pub type GLshort = ::core::ffi::c_short;
pub type GLint = ::core::ffi::c_int;
pub type GLubyte = ::core::ffi::c_uchar;
pub type GLushort = ::core::ffi::c_ushort;
pub type GLuint = ::core::ffi::c_uint;
pub type GLuint64 = ::core::ffi::c_ulonglong;
pub type GLsizei = ::core::ffi::c_int;
pub type GLchar = ::core::ffi::c_char;

pub type PFNGLXDESTROYCONTEXTPROC =
    Option<unsafe extern "C" fn(_: *mut Display, _: GLXContext) -> ()>;
pub type PFNGLXDESTROYWINDOWPROC =
    Option<unsafe extern "C" fn(_: *mut Display, _: GLXWindow) -> ()>;
pub type PFNGLXSWAPBUFFERSPROC =
    Option<unsafe extern "C" fn(_: *mut Display, _: GLXDrawable) -> ()>;

pub type PFNGLXGETFBCONFIGATTRIBPROC = Option<
    unsafe extern "C" fn(
        _: *mut Display,
        _: GLXFBConfig,
        _: libc::c_int,
        _: *mut libc::c_int,
    ) -> libc::c_int,
>;
pub type PFNGLXGETFBCONFIGSPROC = Option<
    unsafe extern "C" fn(_: *mut Display, _: libc::c_int, _: *mut libc::c_int) -> *mut GLXFBConfig,
>;
pub type PFNGLXMAKECURRENTPROC =
    Option<unsafe extern "C" fn(_: *mut Display, _: GLXDrawable, _: GLXContext) -> libc::c_int>;
pub type PFNGLXSWAPINTERVALMESAPROC = Option<unsafe extern "C" fn(_: libc::c_int) -> libc::c_int>;
pub type PFNGLXSWAPINTERVALEXTPROC =
    Option<unsafe extern "C" fn(_: *mut Display, _: GLXDrawable, _: libc::c_int) -> ()>;
pub type PFNGLXGETCLIENTSTRINGPROC =
    Option<unsafe extern "C" fn(_: *mut Display, _: libc::c_int) -> *const libc::c_char>;
pub type PFNGLXCREATEWINDOWPROC = Option<
    unsafe extern "C" fn(
        _: *mut Display,
        _: GLXFBConfig,
        _: Window,
        _: *const libc::c_int,
    ) -> GLXWindow,
>;
pub type PFNGLXCREATECONTEXTATTRIBSARBPROC = Option<
    unsafe extern "C" fn(
        _: *mut Display,
        _: GLXFBConfig,
        _: GLXContext,
        _: libc::c_int,
        _: *const libc::c_int,
    ) -> GLXContext,
>;
pub type PFNGLXGETVISUALFROMFBCONFIGPROC =
    Option<unsafe extern "C" fn(_: *mut Display, _: GLXFBConfig) -> *mut XVisualInfo>;
pub type PFNGLXQUERYEXTENSIONSSTRINGPROC =
    Option<unsafe extern "C" fn(_: *mut Display, _: libc::c_int) -> *const libc::c_char>;
pub type __GLXextproc = Option<unsafe extern "C" fn() -> ()>;
pub type PFNGLXGETPROCADDRESSPROC = Option<unsafe extern "C" fn(_: *const GLubyte) -> __GLXextproc>;
pub type PFNGLXQUERYVERSIONPROC = Option<
    unsafe extern "C" fn(_: *mut Display, _: *mut libc::c_int, _: *mut libc::c_int) -> libc::c_int,
>;
pub type PFNGLXQUERYEXTENSIONPROC = Option<
    unsafe extern "C" fn(_: *mut Display, _: *mut libc::c_int, _: *mut libc::c_int) -> libc::c_int,
>;
pub type PFNGLXCREATENEWCONTEXTPROC = Option<
    unsafe extern "C" fn(
        _: *mut Display,
        _: GLXFBConfig,
        _: libc::c_int,
        _: GLXContext,
        _: libc::c_int,
    ) -> GLXContext,
>;

pub struct LibGlx {
    pub module: module::Module,
    pub glxGetFBConfigs: PFNGLXGETFBCONFIGSPROC,
    pub glxGetFBConfigAttrib: PFNGLXGETFBCONFIGATTRIBPROC,
    pub glxGetClientString: PFNGLXGETCLIENTSTRINGPROC,
    pub glxQueryExtension: PFNGLXQUERYEXTENSIONPROC,
    pub glxQueryVersion: PFNGLXQUERYVERSIONPROC,
    pub glxDestroyContext: PFNGLXDESTROYCONTEXTPROC,
    pub glxMakeCurrent: PFNGLXMAKECURRENTPROC,
    pub glxSwapBuffers: PFNGLXSWAPBUFFERSPROC,
    pub glxQueryExtensionsString: PFNGLXQUERYEXTENSIONSSTRINGPROC,
    pub glxCreateNewContext: PFNGLXCREATENEWCONTEXTPROC,
    pub glxCreateWindow: PFNGLXCREATEWINDOWPROC,
    pub glxDestroyWindow: PFNGLXDESTROYWINDOWPROC,
    pub glxGetProcAddress: PFNGLXGETPROCADDRESSPROC,
    pub glxGetProcAddressARB: PFNGLXGETPROCADDRESSPROC,
    pub glxGetVisualFromFBConfig: PFNGLXGETVISUALFROMFBCONFIGPROC,
}

impl LibGlx {
    pub fn try_load() -> Result<LibGlx, module::Error> {
        module::Module::load("libGL.so")
            .or_else(|_| module::Module::load("libGL.so.1"))
            .map(|module| LibGlx {
                glxGetFBConfigs: module.get_symbol("glXGetFBConfigs").ok(),
                glxGetFBConfigAttrib: module.get_symbol("glXGetFBConfigAttrib").ok(),
                glxGetClientString: module.get_symbol("glXGetClientString").ok(),
                glxQueryExtension: module.get_symbol("glXQueryExtension").ok(),
                glxQueryVersion: module.get_symbol("glXQueryVersion").ok(),
                glxDestroyContext: module.get_symbol("glXDestroyContext").ok(),
                glxMakeCurrent: module.get_symbol("glXMakeCurrent").ok(),
                glxSwapBuffers: module.get_symbol("glXSwapBuffers").ok(),
                glxQueryExtensionsString: module.get_symbol("glXQueryExtensionsString").ok(),
                glxCreateNewContext: module.get_symbol("glXCreateNewContext").ok(),
                glxCreateWindow: module.get_symbol("glXCreateWindow").ok(),
                glxDestroyWindow: module.get_symbol("glXDestroyWindow").ok(),
                glxGetProcAddress: module.get_symbol("glXGetProcAddress").ok(),
                glxGetProcAddressARB: module.get_symbol("glXGetProcAddressARB").ok(),
                glxGetVisualFromFBConfig: module.get_symbol("glXGetVisualFromFBConfig").ok(),
                module,
            })
    }

    pub unsafe fn get_procaddr(&self, procname: &str) -> Option<unsafe extern "C" fn() -> ()> {
        unsafe {
            if let Some(glx_get_proc_address) = self.glxGetProcAddress {
                let name = std::ffi::CString::new(procname).unwrap();
                glx_get_proc_address(name.as_ptr() as _)
            } else if let Some(glx_get_proc_address_arb) = self.glxGetProcAddressARB {
                let name = std::ffi::CString::new(procname).unwrap();
                glx_get_proc_address_arb(name.as_ptr() as _)
            } else {
                self.module.get_symbol(procname).ok()
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct GLFBConfig {
    pub red_bits: libc::c_int,
    pub green_bits: libc::c_int,
    pub blue_bits: libc::c_int,
    pub alpha_bits: libc::c_int,
    pub depth_bits: libc::c_int,
    pub stencil_bits: libc::c_int,
    pub samples: libc::c_int,
    pub doublebuffer: bool,
    pub handle: libc::c_ulong,
}

impl Default for GLFBConfig {
    fn default() -> Self {
        GLFBConfig {
            red_bits: -1,
            green_bits: -1,
            blue_bits: -1,
            alpha_bits: -1,
            depth_bits: -1,
            stencil_bits: -1,
            samples: -1,
            doublebuffer: false,
            handle: 0,
        }
    }
}

pub struct GlxExtensions {
    pub extensions_string: String,
    pub glxSwapIntervalExt: PFNGLXSWAPINTERVALEXTPROC,
    pub glxSwapIntervalMesa: PFNGLXSWAPINTERVALMESAPROC,
    pub glxCreateContextAttribsARB: PFNGLXCREATECONTEXTATTRIBSARBPROC,
}

pub struct Glx {
    pub libgl: LibGlx,
    multisample: bool,
    extensions: GlxExtensions,
    fbconfig: GLXFBConfig,
    pub visual: *mut Visual,
    pub depth: i32,
}

impl Glx {
    pub unsafe fn init(
        libx11: &mut LibX11,
        display: *mut Display,
        screen: i32,
        conf: &crate::conf::Conf,
    ) -> Result<Glx, X11Error> {
        unsafe {
            let mut libgl = LibGlx::try_load()?;

            let mut errorbase = 0;
            let mut eventbase = 0;

            if (libgl.glxQueryExtension.unwrap())(display, &mut errorbase, &mut eventbase) == 0 {
                return Err(X11Error::GLXError("GLX extension not found".to_string()));
            }

            let mut glx_major = 0;
            let mut glx_minor = 0;

            if (libgl.glxQueryVersion.unwrap())(display, &mut glx_major, &mut glx_minor) == 0 {
                return Err(X11Error::GLXError(
                    "Failed to query GLX version".to_string(),
                ));
            }
            if glx_major == 1 && glx_minor < 3 {
                return Err(X11Error::GLXError(
                    "GLX version 1.3 is required".to_string(),
                ));
            }

            let exts = (libgl.glxQueryExtensionsString.unwrap())(display, screen);
            let extensions = std::ffi::CStr::from_ptr(exts).to_str().unwrap().to_owned();

            let multisample = extensions.contains("GLX_ARB_multisample");
            // let glx_ARB_framebuffer_sRGB =
            //     _sapp_glx_extsupported(b"GLX_ARB_framebuffer_sRGB\x00", exts);
            // _sapp_glx_EXT_framebuffer_sRGB =
            //     _sapp_glx_extsupported(b"GLX_EXT_framebuffer_sRGB\x00", exts);
            // if _sapp_glx_extsupported(b"GLX_ARB_create_context\x00", exts) {
            //     _sapp_glx_CreateContextAttribsARB =
            //         _sapp_glx_getprocaddr(b"glXCreateContextAttribsARB\x00");
            //     _sapp_glx_ARB_create_context = _sapp_glx_CreateContextAttribsARB.is_some()
            // }
            // _sapp_glx_ARB_create_context_profile =
            //     _sapp_glx_extsupported(b"GLX_ARB_create_context_profile\x00", exts);

            let fbconfig = choose_fbconfig(
                &mut libgl,
                libx11,
                display,
                screen,
                multisample,
                conf.sample_count,
            );
            assert!(
                !fbconfig.is_null(),
                "GLX: Failed to find a suitable GLXFBConfig"
            );

            let result = libgl.glxGetVisualFromFBConfig.unwrap()(display, fbconfig);
            assert!(
                !result.is_null(),
                "GLX: Failed to retrieve Visual for GLXFBConfig"
            );

            let visual = (*result).visual;
            let depth = (*result).depth;

            (libx11.XFree)(result as *mut libc::c_void);

            let extensions_string = extensions;
            let mut extensions = GlxExtensions {
                extensions_string: extensions_string.clone(),
                glxSwapIntervalExt: None,
                glxSwapIntervalMesa: None,
                glxCreateContextAttribsARB: None,
            };
            if extensions_string.contains("GLX_EXT_swap_control") {
                extensions.glxSwapIntervalExt =
                    std::mem::transmute_copy(&libgl.get_procaddr("glXSwapIntervalEXT"));
            }
            if extensions_string.contains("GLX_MESA_swap_control") {
                extensions.glxSwapIntervalMesa =
                    std::mem::transmute_copy(&libgl.get_procaddr("glXSwapIntervalMESA"));
            }

            if extensions_string.contains("GLX_ARB_create_context")
                && extensions_string.contains("GLX_ARB_create_context_profile")
            {
                extensions.glxCreateContextAttribsARB =
                    std::mem::transmute_copy(&libgl.get_procaddr("glXCreateContextAttribsARB"))
            };

            Ok(Glx {
                libgl,
                multisample,
                visual,
                depth,
                extensions,
                fbconfig,
            })
        }
    }

    pub unsafe fn create_context(
        &mut self,
        display: *mut Display,
        window: Window,
    ) -> (GLXContext, GLXWindow) {
        unsafe {
            if self.extensions.glxCreateContextAttribsARB.is_none() {
                panic!("GLX: ARB_create_context and ARB_create_context_profile required");
            }

            // _sapp_x11_grab_error_handler(libx11);
            let attribs: [libc::c_int; 8] = [
                GLX_CONTEXT_MAJOR_VERSION_ARB,
                2,
                GLX_CONTEXT_MINOR_VERSION_ARB,
                1,
                GLX_CONTEXT_FLAGS_ARB,
                GLX_CONTEXT_CORE_PROFILE_BIT_ARB,
                0,
                0,
            ];
            let glx_ctx = self.extensions.glxCreateContextAttribsARB.unwrap()(
                display,
                self.fbconfig,
                std::ptr::null_mut(),
                true as _,
                attribs.as_ptr(),
            );
            assert!(!glx_ctx.is_null(), "GLX: failed to create GL context");
            // _sapp_x11_release_error_handler(libx11);

            let glx_window = self.libgl.glxCreateWindow.unwrap()(
                display,
                self.fbconfig,
                window,
                std::ptr::null(),
            );
            assert!(glx_window != 0, "GLX: failed to create window");

            (glx_ctx, glx_window)
        }
    }

    pub unsafe fn destroy_context(
        &mut self,
        display: *mut Display,
        window: GLXWindow,
        ctx: GLXContext,
    ) {
        unsafe {
            if window != 0 {
                self.libgl.glxDestroyWindow.unwrap()(display, window);
            }
            if !ctx.is_null() {
                self.libgl.glxDestroyContext.unwrap()(display, ctx);
            };
        }
    }

    pub unsafe fn make_current(
        &mut self,
        display: *mut Display,
        window: GLXWindow,
        ctx: GLXContext,
    ) {
        unsafe {
            self.libgl.glxMakeCurrent.unwrap()(display, window, ctx);
        }
    }

    pub unsafe fn swap_buffers(&mut self, display: *mut Display, window: GLXWindow) {
        unsafe {
            self.libgl.glxSwapBuffers.unwrap()(display, window);
        }
    }

    pub unsafe fn swap_interval(
        &mut self,
        display: *mut Display,
        window: GLXWindow,
        ctx: GLXContext,
        interval: i32,
    ) {
        unsafe {
            self.libgl.glxMakeCurrent.unwrap()(display, window, ctx);

            if let Some(swap_interval) = self.extensions.glxSwapIntervalExt {
                swap_interval(display, window, interval);
            } else if let Some(swap_interval) = self.extensions.glxSwapIntervalMesa {
                swap_interval(interval);
            };
        }
    }
}

// TODO: this code came a long way from sokol_app, better reimplement it!
unsafe fn choose_fbconfig(
    libgl: &mut LibGlx,
    libx11: &mut super::LibX11,
    display: *mut Display,
    screen: i32,
    multisample: bool,
    desired_sample_count: i32,
) -> GLXFBConfig {
    unsafe {
        let mut native_count: libc::c_int = 0;
        let mut usable_count;
        let mut trust_window_bit = true;
        let vendor = (libgl.glxGetClientString.unwrap())(display, GLX_VENDOR);
        if !vendor.is_null()
            && libc::strcmp(vendor, b"Chromium\x00" as *const u8 as *const libc::c_char)
                == 0 as libc::c_int
        {
            trust_window_bit = false
        }
        let native_configs: *mut GLXFBConfig =
            (libgl.glxGetFBConfigs.unwrap())(display, screen, &mut native_count);

        if native_configs.is_null() || native_count == 0 {
            panic!("GLX: No GLXFBConfigs returned");
        }

        let mut usable_configs: Vec<GLFBConfig> = Vec::new();
        usable_count = 0 as libc::c_int;

        for i in 0..native_count {
            let n = *native_configs.offset(i as isize);
            let mut u = GLFBConfig::default();

            let glx_attrib = |fbconfig, attrib| {
                let mut value: libc::c_int = 0;
                (libgl.glxGetFBConfigAttrib.unwrap())(display, fbconfig, attrib, &mut value);
                value
            };

            if 0 == glx_attrib(n, GLX_RENDER_TYPE) & GLX_RGBA_BIT {
                continue;
            }
            if 0 == glx_attrib(n, GLX_DRAWABLE_TYPE) & GLX_WINDOW_BIT {
                if trust_window_bit {
                    continue;
                }
            }

            u.red_bits = glx_attrib(n, GLX_RED_SIZE);
            u.green_bits = glx_attrib(n, GLX_GREEN_SIZE);
            u.blue_bits = glx_attrib(n, GLX_BLUE_SIZE);
            u.alpha_bits = glx_attrib(n, GLX_ALPHA_SIZE);
            u.depth_bits = glx_attrib(n, GLX_DEPTH_SIZE);
            u.stencil_bits = glx_attrib(n, GLX_STENCIL_SIZE);
            if glx_attrib(n, GLX_DOUBLEBUFFER) != 0 {
                u.doublebuffer = true
            }
            if multisample {
                u.samples = glx_attrib(n, GLX_SAMPLES)
            }
            u.handle = n as libc::c_ulong;
            usable_configs.push(u);
            usable_count += 1
        }

        let mut result = 0 as GLXFBConfig;
        #[allow(clippy::field_reassign_with_default)]
        {
            let mut desired = GLFBConfig::default();
            desired.red_bits = 8;
            desired.green_bits = 8;
            desired.blue_bits = 8;
            desired.alpha_bits = 8;
            desired.depth_bits = 24;
            desired.stencil_bits = 8;
            desired.doublebuffer = true;
            desired.samples = if desired_sample_count > 1 {
                desired_sample_count
            } else {
                0
            };

            let closest: *const GLFBConfig = gl_choose_fbconfig(
                &desired,
                usable_configs.as_mut_ptr(),
                usable_count as libc::c_uint,
            );
            if !closest.is_null() {
                result = (*closest).handle as GLXFBConfig
            }
            (libx11.XFree)(native_configs as *mut libc::c_void);
        }
        result
    }
}

impl From<&GLFBConfig> for crate::native::fbconfig::FbConfigSpec {
    fn from(c: &GLFBConfig) -> Self {
        crate::native::fbconfig::FbConfigSpec {
            red_bits: c.red_bits,
            green_bits: c.green_bits,
            blue_bits: c.blue_bits,
            alpha_bits: c.alpha_bits,
            depth_bits: c.depth_bits,
            stencil_bits: c.stencil_bits,
            samples: c.samples,
            doublebuffer: c.doublebuffer,
        }
    }
}

pub unsafe extern "C" fn gl_choose_fbconfig(
    desired: *const GLFBConfig,
    alternatives: *const GLFBConfig,
    count: libc::c_uint,
) -> *const GLFBConfig {
    unsafe {
        let specs: Vec<crate::native::fbconfig::FbConfigSpec> = (0..count as usize)
            .map(|i| (&*alternatives.add(i)).into())
            .collect();
        crate::native::fbconfig::fbconfig_choose(&(&*desired).into(), &specs)
            .map_or(std::ptr::null(), |i| alternatives.add(i))
    }
}
