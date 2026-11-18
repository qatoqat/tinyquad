#![expect(non_upper_case_globals, non_snake_case)]

use super::{
    libx11::{self, _XPrivDisplay, Display, Window},
    xi_input,
};
use core::ffi::{c_char, c_double, c_int, c_uchar, c_ulong};

pub const XIAllDevices: c_int = 0 as c_int;
pub const XI_RawMotion: c_int = 17 as c_int;
pub const XI_RawMotionMask: c_int = (1 as c_int) << XI_RawMotion;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct XIEventMask {
    pub deviceid: c_int,
    pub mask_len: c_int,
    pub mask: *mut c_uchar,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct XIValuatorState {
    pub mask_len: c_int,
    pub mask: *mut c_uchar,
    pub values: *mut c_double,
}

pub type Time = c_ulong;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct XIRawEvent {
    pub type_0: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub extension: c_int,
    pub evtype: c_int,
    pub time: Time,
    pub deviceid: c_int,
    pub sourceid: c_int,
    pub detail: c_int,
    pub flags: c_int,
    pub valuators: XIValuatorState,
    pub raw_values: *mut c_double,
}

crate::declare_module!(
    LibXi,
    "libXi.so",
    "libXi.so.6",
    ...
    ...
    pub fn XIQueryVersion(*mut Display, *mut c_int, *mut c_int) -> c_int,
    pub fn XISelectEvents(*mut Display, Window, *mut XIEventMask, c_int),
    pub fn XGetEventData(*mut Display, *mut libx11::XGenericEventCookie) -> c_int,
    pub fn XFreeEventData(*mut Display, *mut libx11::XGenericEventCookie),
    ...
    ...
    pub xi_extension_opcode: Option<i32>,
);

impl LibXi {
    pub unsafe fn query_xi_extension(
        &mut self,
        libx11: &mut libx11::LibX11,
        display: *mut Display,
    ) {
        unsafe {
            let mut ev = 0;
            let mut err = 0;
            let mut xi_opcode = 0;

            if (libx11.XQueryExtension)(
                display,
                b"XInputExtension\x00" as *const u8 as *const c_char,
                &mut xi_opcode,
                &mut ev,
                &mut err,
            ) == 0
            {
                return;
            }

            // check the version of XInput
            let mut major = 2;
            let mut minor = 3;
            if (self.XIQueryVersion)(display, &mut major, &mut minor) != 0 {
                return;
            }

            // select events to listen
            let mut mask = XI_RawMotionMask;
            let mut masks = XIEventMask {
                deviceid: XIAllDevices,
                mask_len: size_of::<c_int>() as _,
                mask: &mut mask as *mut _ as *mut _,
            };

            (self.XISelectEvents)(
                display,
                // this weird pointers is macro expansion of DefaultRootWindow(display)
                (*(*(display as _XPrivDisplay))
                    .screens
                    .offset((*(display as _XPrivDisplay)).default_screen as isize))
                .root,
                &mut masks,
                1 as c_int,
            );
            self.xi_extension_opcode = Some(xi_opcode);
        }
    }

    /// Get mouse delta from XI_RawMotion's event XGenericEventCookie data
    pub unsafe fn read_cookie(
        &mut self,
        xcookie: &mut libx11::XGenericEventCookie,
        display: *mut Display,
    ) -> (f64, f64) {
        unsafe {
            assert!(xcookie.evtype == xi_input::XI_RawMotion);

            (self.XGetEventData)(display, xcookie);

            let raw_event = xcookie.data as *mut xi_input::XIRawEvent;

            // Data returned from Xlib is not guaranteed to be aligned
            let ptr = (*raw_event).raw_values as *const f64;
            let dx = std::ptr::read_unaligned(ptr);
            let dy = std::ptr::read_unaligned(ptr.add(1));

            (self.XFreeEventData)(display, &mut (*xcookie) as *mut _);

            (dx, dy)
        }
    }
}
