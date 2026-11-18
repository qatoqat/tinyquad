#![expect(non_snake_case, non_upper_case_globals)]
pub use X_h::*;
pub use Xlib_h::*;
pub use Xresource_h::*;
pub use Xutil_h::*;
use core::ffi::{c_char, c_int, c_long, c_uchar, c_uint, c_ulong, c_ushort, c_void};

pub mod Xlib_h {
    use core::ffi::{c_char, c_int, c_long, c_short, c_uchar, c_uint, c_ulong, c_void};

    pub type Display = _XDisplay;
    pub type XEvent = _XEvent;

    #[derive(Copy, Clone)]
    #[repr(C)]
    pub union _XEvent {
        pub type_0: c_int,
        pub xany: XAnyEvent,
        pub xkey: XKeyEvent,
        pub xbutton: XButtonEvent,
        pub xmotion: XMotionEvent,
        pub xcrossing: XCrossingEvent,
        pub xfocus: XFocusChangeEvent,
        pub xexpose: XExposeEvent,
        pub xgraphicsexpose: XGraphicsExposeEvent,
        pub xnoexpose: XNoExposeEvent,
        pub xvisibility: XVisibilityEvent,
        pub xcreatewindow: XCreateWindowEvent,
        pub xdestroywindow: XDestroyWindowEvent,
        pub xunmap: XUnmapEvent,
        pub xmap: XMapEvent,
        pub xmaprequest: XMapRequestEvent,
        pub xreparent: XReparentEvent,
        pub xconfigure: XConfigureEvent,
        pub xgravity: XGravityEvent,
        pub xresizerequest: XResizeRequestEvent,
        pub xconfigurerequest: XConfigureRequestEvent,
        pub xcirculate: XCirculateEvent,
        pub xcirculaterequest: XCirculateRequestEvent,
        pub xproperty: XPropertyEvent,
        pub xselectionclear: XSelectionClearEvent,
        pub xselectionrequest: XSelectionRequestEvent,
        pub xselection: XSelectionEvent,
        pub xcolormap: XColormapEvent,
        pub xclient: XClientMessageEvent,
        pub xmapping: XMappingEvent,
        pub xerror: XErrorEvent,
        pub xkeymap: XKeymapEvent,
        pub xgeneric: XGenericEvent,
        pub xcookie: XGenericEventCookie,
        pub pad: [c_long; 24],
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XGenericEventCookie {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub extension: c_int,
        pub evtype: c_int,
        pub cookie: c_uint,
        pub data: *mut c_void,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XGenericEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub extension: c_int,
        pub evtype: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XKeymapEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub key_vector: [c_char; 32],
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XErrorEvent {
        pub type_0: c_int,
        pub display: *mut Display,
        pub resourceid: XID,
        pub serial: c_ulong,
        pub error_code: c_uchar,
        pub request_code: c_uchar,
        pub minor_code: c_uchar,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XMappingEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub request: c_int,
        pub first_keycode: c_int,
        pub count: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XClientMessageEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub message_type: Atom,
        pub format: c_int,
        pub data: ClientMessageData,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub union ClientMessageData {
        pub b: [c_char; 20],
        pub s: [c_short; 10],
        pub l: [c_long; 5],
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XColormapEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub colormap: Colormap,
        pub new: c_int,
        pub state: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XSelectionEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub requestor: Window,
        pub selection: Atom,
        pub target: Atom,
        pub property: Atom,
        pub time: Time,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XSelectionRequestEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub owner: Window,
        pub requestor: Window,
        pub selection: Atom,
        pub target: Atom,
        pub property: Atom,
        pub time: Time,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XSelectionClearEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub selection: Atom,
        pub time: Time,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XPropertyEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub atom: Atom,
        pub time: Time,
        pub state: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XCirculateRequestEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub parent: Window,
        pub window: Window,
        pub place: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XCirculateEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub event: Window,
        pub window: Window,
        pub place: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XConfigureRequestEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub parent: Window,
        pub window: Window,
        pub x: c_int,
        pub y: c_int,
        pub width: c_int,
        pub height: c_int,
        pub border_width: c_int,
        pub above: Window,
        pub detail: c_int,
        pub value_mask: c_ulong,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XResizeRequestEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub width: c_int,
        pub height: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XGravityEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub event: Window,
        pub window: Window,
        pub x: c_int,
        pub y: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XConfigureEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub event: Window,
        pub window: Window,
        pub x: c_int,
        pub y: c_int,
        pub width: c_int,
        pub height: c_int,
        pub border_width: c_int,
        pub above: Window,
        pub override_redirect: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XReparentEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub event: Window,
        pub window: Window,
        pub parent: Window,
        pub x: c_int,
        pub y: c_int,
        pub override_redirect: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XMapRequestEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub parent: Window,
        pub window: Window,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XMapEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub event: Window,
        pub window: Window,
        pub override_redirect: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XUnmapEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub event: Window,
        pub window: Window,
        pub from_configure: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XDestroyWindowEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub event: Window,
        pub window: Window,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XCreateWindowEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub parent: Window,
        pub window: Window,
        pub x: c_int,
        pub y: c_int,
        pub width: c_int,
        pub height: c_int,
        pub border_width: c_int,
        pub override_redirect: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XVisibilityEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub state: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XNoExposeEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub drawable: Drawable,
        pub major_code: c_int,
        pub minor_code: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XGraphicsExposeEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub drawable: Drawable,
        pub x: c_int,
        pub y: c_int,
        pub width: c_int,
        pub height: c_int,
        pub count: c_int,
        pub major_code: c_int,
        pub minor_code: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XExposeEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub x: c_int,
        pub y: c_int,
        pub width: c_int,
        pub height: c_int,
        pub count: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XFocusChangeEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub mode: c_int,
        pub detail: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XCrossingEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub root: Window,
        pub subwindow: Window,
        pub time: Time,
        pub x: c_int,
        pub y: c_int,
        pub x_root: c_int,
        pub y_root: c_int,
        pub mode: c_int,
        pub detail: c_int,
        pub same_screen: c_int,
        pub focus: c_int,
        pub state: c_uint,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XMotionEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub root: Window,
        pub subwindow: Window,
        pub time: Time,
        pub x: c_int,
        pub y: c_int,
        pub x_root: c_int,
        pub y_root: c_int,
        pub state: c_uint,
        pub is_hint: c_char,
        pub same_screen: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XButtonEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub root: Window,
        pub subwindow: Window,
        pub time: Time,
        pub x: c_int,
        pub y: c_int,
        pub x_root: c_int,
        pub y_root: c_int,
        pub state: c_uint,
        pub button: c_uint,
        pub same_screen: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XKeyEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub root: Window,
        pub subwindow: Window,
        pub time: Time,
        pub x: c_int,
        pub y: c_int,
        pub x_root: c_int,
        pub y_root: c_int,
        pub state: c_uint,
        pub keycode: c_uint,
        pub same_screen: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XAnyEvent {
        pub type_0: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
    }
    pub type XPointer = *mut c_char;
    pub type XErrorHandler =
        Option<unsafe extern "C" fn(_: *mut Display, _: *mut XErrorEvent) -> c_int>;
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XWindowAttributes {
        pub x: c_int,
        pub y: c_int,
        pub width: c_int,
        pub height: c_int,
        pub border_width: c_int,
        pub depth: c_int,
        pub visual: *mut Visual,
        pub root: Window,
        pub class: c_int,
        pub bit_gravity: c_int,
        pub win_gravity: c_int,
        pub backing_store: c_int,
        pub backing_planes: c_ulong,
        pub backing_pixel: c_ulong,
        pub save_under: c_int,
        pub colormap: Colormap,
        pub map_installed: c_int,
        pub map_state: c_int,
        pub all_event_masks: c_long,
        pub your_event_mask: c_long,
        pub do_not_propagate_mask: c_long,
        pub override_redirect: c_int,
        pub screen: *mut Screen,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct Screen {
        pub ext_data: *mut XExtData,
        pub display: *mut _XDisplay,
        pub root: Window,
        pub width: c_int,
        pub height: c_int,
        pub mwidth: c_int,
        pub mheight: c_int,
        pub ndepths: c_int,
        pub depths: *mut Depth,
        pub root_depth: c_int,
        pub root_visual: *mut Visual,
        pub default_gc: GC,
        pub cmap: Colormap,
        pub white_pixel: c_ulong,
        pub black_pixel: c_ulong,
        pub max_maps: c_int,
        pub min_maps: c_int,
        pub backing_store: c_int,
        pub save_unders: c_int,
        pub root_input_mask: c_long,
    }
    pub type GC = *mut _XGC;
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct Visual {
        pub ext_data: *mut XExtData,
        pub visualid: VisualID,
        pub class: c_int,
        pub red_mask: c_ulong,
        pub green_mask: c_ulong,
        pub blue_mask: c_ulong,
        pub bits_per_rgb: c_int,
        pub map_entries: c_int,
    }
    pub type XExtData = _XExtData;
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct _XExtData {
        pub number: c_int,
        pub next: *mut _XExtData,
        pub free_private: Option<unsafe extern "C" fn(_: *mut _XExtData) -> c_int>,
        pub private_data: XPointer,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct Depth {
        pub depth: c_int,
        pub nvisuals: c_int,
        pub visuals: *mut Visual,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XSetWindowAttributes {
        pub background_pixmap: Pixmap,
        pub background_pixel: c_ulong,
        pub border_pixmap: Pixmap,
        pub border_pixel: c_ulong,
        pub bit_gravity: c_int,
        pub win_gravity: c_int,
        pub backing_store: c_int,
        pub backing_planes: c_ulong,
        pub backing_pixel: c_ulong,
        pub save_under: c_int,
        pub event_mask: c_long,
        pub do_not_propagate_mask: c_long,
        pub override_redirect: c_int,
        pub colormap: Colormap,
        pub cursor: Cursor,
    }
    pub type _XPrivDisplay = *mut C2RustUnnamed_3;
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct C2RustUnnamed_3 {
        pub ext_data: *mut XExtData,
        pub private1: *mut _XPrivate,
        pub fd: c_int,
        pub private2: c_int,
        pub proto_major_version: c_int,
        pub proto_minor_version: c_int,
        pub vendor: *mut c_char,
        pub private3: XID,
        pub private4: XID,
        pub private5: XID,
        pub private6: c_int,
        pub resource_alloc: Option<unsafe extern "C" fn(_: *mut _XDisplay) -> XID>,
        pub byte_order: c_int,
        pub bitmap_unit: c_int,
        pub bitmap_pad: c_int,
        pub bitmap_bit_order: c_int,
        pub nformats: c_int,
        pub pixmap_format: *mut ScreenFormat,
        pub private8: c_int,
        pub release: c_int,
        pub private9: *mut _XPrivate,
        pub private10: *mut _XPrivate,
        pub qlen: c_int,
        pub last_request_read: c_ulong,
        pub request: c_ulong,
        pub private11: XPointer,
        pub private12: XPointer,
        pub private13: XPointer,
        pub private14: XPointer,
        pub max_request_size: c_uint,
        pub db: *mut _XrmHashBucketRec,
        pub private15: Option<unsafe extern "C" fn(_: *mut _XDisplay) -> c_int>,
        pub display_name: *mut c_char,
        pub default_screen: c_int,
        pub nscreens: c_int,
        pub screens: *mut Screen,
        pub motion_buffer: c_ulong,
        pub private16: c_ulong,
        pub min_keycode: c_int,
        pub max_keycode: c_int,
        pub private17: XPointer,
        pub private18: XPointer,
        pub private19: c_int,
        pub xdefaults: *mut c_char,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct ScreenFormat {
        pub ext_data: *mut XExtData,
        pub depth: c_int,
        pub bits_per_pixel: c_int,
        pub scanline_pad: c_int,
    }

    use super::X_h::{Atom, Colormap, Cursor, Drawable, Pixmap, Time, VisualID, Window, XID};
    pub type _XDisplay = ();
    pub type _XGC = ();
    pub type _XrmHashBucketRec = ();
    pub type _XPrivate = ();
}

pub mod X_h {
    use core::ffi::{c_int, c_long, c_uchar, c_ulong};

    pub type Colormap = XID;
    pub type XID = c_ulong;
    pub type Window = XID;
    pub type Atom = c_ulong;
    pub type Time = c_ulong;
    pub type Drawable = XID;
    pub type Pixmap = XID;
    pub type KeySym = XID;
    pub type KeyCode = c_uchar;
    pub type VisualID = c_ulong;
    pub type Cursor = XID;
    pub const AllocNone: c_int = 0 as c_int;
    pub const StructureNotifyMask: c_long = (1 as c_long) << 17 as c_int;
    pub const SubstructureNotifyMask: c_long = (1 as c_long) << 19 as c_int;
    pub const SubstructureRedirectMask: c_long = (1 as c_long) << 20 as c_int;
    pub const NoEventMask: c_long = 0 as c_long;
    pub const KeyPressMask: c_long = (1 as c_long) << 0 as c_int;
    pub const KeyReleaseMask: c_long = (1 as c_long) << 1 as c_int;
    pub const PointerMotionMask: c_long = (1 as c_long) << 6 as c_int;
    pub const PointerMotionHintMask: c_long = (1 as c_long) << 7 as c_int;
    pub const Button1MotionMask: c_long = (1 as c_long) << 8 as c_int;
    pub const Button2MotionMask: c_long = (1 as c_long) << 9 as c_int;
    pub const Button3MotionMask: c_long = (1 as c_long) << 10 as c_int;
    pub const Button4MotionMask: c_long = (1 as c_long) << 11 as c_int;
    pub const Button5MotionMask: c_long = (1 as c_long) << 12 as c_int;
    pub const ButtonMotionMask: c_long = (1 as c_long) << 13 as c_int;
    pub const KeymapStateMask: c_long = (1 as c_long) << 14 as c_int;

    pub const GrabModeAsync: c_int = 1 as c_int;

    pub const ButtonPressMask: c_long = (1 as c_long) << 2 as c_int;
    pub const ButtonReleaseMask: c_long = (1 as c_long) << 3 as c_int;
    pub const ExposureMask: c_long = (1 as c_long) << 15 as c_int;
    pub const FocusChangeMask: c_long = (1 as c_long) << 21 as c_int;
    pub const VisibilityChangeMask: c_long = (1 as c_long) << 16 as c_int;
    pub const EnterWindowMask: c_long = (1 as c_long) << 4 as c_int;
    pub const LeaveWindowMask: c_long = (1 as c_long) << 5 as c_int;
    pub const PropertyChangeMask: c_long = (1 as c_long) << 22 as c_int;
    pub const InputOutput: c_int = 1 as c_int;
    pub const CWBorderPixel: c_long = (1 as c_long) << 3 as c_int;
    pub const CWColormap: c_long = (1 as c_long) << 13 as c_int;
    pub const CWEventMask: c_long = (1 as c_long) << 11 as c_int;
    pub const StaticGravity: c_int = 10 as c_int;
    pub const PropModeReplace: c_int = 0 as c_int;
    pub const Success: c_int = 0 as c_int;
    pub const IsViewable: c_int = 2 as c_int;
    pub const ShiftMask: c_int = (1 as c_int) << 0 as c_int;
    pub const ControlMask: c_int = (1 as c_int) << 2 as c_int;
    pub const Mod1Mask: c_int = (1 as c_int) << 3 as c_int;
    pub const Mod4Mask: c_int = (1 as c_int) << 6 as c_int;

    pub const AnyPropertyType: c_ulong = 0 as c_ulong;
    pub const CurrentTime: c_long = 0 as c_long;

    pub const PropertyNotify: c_int = 28 as c_int;
    pub const SelectionRequest: c_int = 30 as c_int;
    pub const SelectionNotify: c_int = 31 as c_int;
    pub const ClientMessage: c_int = 33 as c_int;

    pub const PropertyNewValue: c_int = 0 as c_int;
}

pub mod Xutil_h {
    use core::ffi::{c_char, c_int, c_long, c_ulong};

    pub type XComposeStatus = _XComposeStatus;
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct _XComposeStatus {
        pub compose_ptr: XPointer,
        pub chars_matched: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XClassHint {
        pub res_name: *mut c_char,
        pub res_class: *mut c_char,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XWMHints {
        pub flags: c_long,
        pub input: c_int,
        pub initial_state: c_int,
        pub icon_pixmap: Pixmap,
        pub icon_window: Window,
        pub icon_x: c_int,
        pub icon_y: c_int,
        pub icon_mask: Pixmap,
        pub window_group: XID,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XSizeHints {
        pub flags: c_long,
        pub x: c_int,
        pub y: c_int,
        pub width: c_int,
        pub height: c_int,
        pub min_width: c_int,
        pub min_height: c_int,
        pub max_width: c_int,
        pub max_height: c_int,
        pub width_inc: c_int,
        pub height_inc: c_int,
        pub min_aspect: C2RustUnnamed_2,
        pub max_aspect: C2RustUnnamed_2,
        pub base_width: c_int,
        pub base_height: c_int,
        pub win_gravity: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct C2RustUnnamed_2 {
        pub x: c_int,
        pub y: c_int,
    }
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XVisualInfo {
        pub visual: *mut Visual,
        pub visualid: VisualID,
        pub screen: c_int,
        pub depth: c_int,
        pub class: c_int,
        pub red_mask: c_ulong,
        pub green_mask: c_ulong,
        pub blue_mask: c_ulong,
        pub colormap_size: c_int,
        pub bits_per_rgb: c_int,
    }
    pub const PMinSize: c_long = (1 as c_long) << 4 as c_int;
    pub const PMaxSize: c_long = (1 as c_long) << 5 as c_int;
    pub const PWinGravity: c_long = (1 as c_long) << 9 as c_int;
    pub const IconicState: c_int = 3 as c_int;
    pub const WithdrawnState: c_int = 0 as c_int;
    pub const NormalState: c_int = 1 as c_int;
    use super::X_h::{Pixmap, VisualID, Window, XID};
    use super::Xlib_h::{Visual, XPointer};
}

pub mod Xresource_h {
    use core::ffi::c_uint;

    pub type XrmDatabase = *mut _XrmHashBucketRec;
    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct XrmValue {
        pub size: c_uint,
        pub addr: XPointer,
    }
    use super::Xlib_h::{_XrmHashBucketRec, XPointer};
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct XColor {
    pub pixel: c_ulong,
    pub red: c_ushort,
    pub green: c_ushort,
    pub blue: c_ushort,
    pub flags: c_char,
    pub pad: c_char,
}

// See https://tronche.com/gui/x/xlib/appendix/b/
pub const XC_crosshair: c_ushort = 34;
pub const XC_fleur: c_ushort = 52;
pub const XC_hand2: c_ushort = 60;
pub const XC_left_ptr: c_ushort = 68;
pub const XC_pirate: c_ushort = 88;
pub const XC_question_arrow: c_ushort = 92;
pub const XC_sb_h_double_arrow: c_ushort = 108;
pub const XC_sb_v_double_arrow: c_ushort = 116;
pub const XC_top_left_corner: c_ushort = 134;
pub const XC_top_right_corner: c_ushort = 136;
pub const XC_watch: c_ushort = 150;
pub const XC_xterm: c_ushort = 152;

macro_rules! declare_atoms {
    ($($name:ident: $atom:literal),*$(,)?) => {
        #[derive(Clone, Default)]
        pub struct X11Extensions {
            $(pub $name: Atom,)*
        }
        impl X11Extensions {
            pub unsafe fn load(libx11: &mut LibX11, display: *mut Display) -> Self { unsafe {
                Self {
                    $($name: {
                        let atom = std::ffi::CString::new($atom).unwrap();
                        (libx11.XInternAtom)(display, atom.as_ptr(), false as _)
                    },)*
                }
            }}
        }
    }
}

declare_atoms!(
    utf8_string: "UTF8_STRING",
    wm_protocols: "WM_PROTOCOLS",
    wm_delete_window: "WM_DELETE_WINDOW",
    _wm_state: "WM_STATE",
    net_wm_name: "_NET_WM_NAME",
    net_wm_icon_name: "_NET_WM_ICON_NAME",
    net_wm_icon: "_NET_WM_ICON",
    cardinal: "CARDINAL",
    // clipboard
    clipboard: "CLIPBOARD",
    xsel_data: "XSEL_DATA",
    incr: "INCR",
    // drag_n_drop related
    xdnd_action_copy: "XdndActionCopy",
    xdnd_aware: "XdndAware",
    xdnd_drop: "XdndDrop",
    xdnd_enter: "XdndEnter",
    xdnd_finished: "XdndFinished",
    xdnd_position: "XdndPosition",
    xdnd_selection: "XdndSelection",
    xdnd_status: "XdndStatus",
    xdnd_type_list: "XdndTypeList",
    mime_text_uri_lst: "text/uri-list",
);

crate::declare_module!(
    LibX11,
    "libX11.so",
    "libX11.so.6",
    ...
    ...
    pub fn XSetWMNormalHints(*mut Display, Window, *mut XSizeHints),
    pub fn XAllocSizeHints() -> *mut XSizeHints,
    pub fn XAllocClassHint() -> *mut XClassHint,
    pub fn XSetClassHint(*mut Display, Window, *mut XClassHint),
    pub fn Xutf8SetWMProperties(*mut Display, Window, *const c_char, *const c_char, *mut *mut c_char, c_int, *mut XSizeHints, *mut XWMHints, *mut XClassHint),
    pub fn XLookupString(*mut XKeyEvent, *mut c_char, c_int, *mut KeySym, *mut XComposeStatus) -> c_int,
    pub fn XInitThreads() -> c_int,
    pub fn XrmInitialize(),
    pub fn XOpenDisplay(*const c_char) -> *mut Display,
    pub fn XResourceManagerString(*mut Display) -> *mut c_char,
    pub fn XInternAtom(*mut Display, *const c_char, c_int) -> Atom,
    pub fn XGetAtomName(*mut Display, Atom) -> *mut c_char,
    pub fn XCreateColormap (*mut Display, Window, *mut Visual, c_int) -> Colormap,
    pub fn XCreateWindow(*mut Display, Window, c_int, c_int, c_uint, c_uint, c_uint, c_int, c_uint, *mut Visual, c_ulong, *mut XSetWindowAttributes) -> Window,
    pub fn XSetWMProtocols (*mut Display, Window, *mut Atom, c_int) -> c_int,
    pub fn XChangeProperty(*mut Display, Window, Atom, Atom, c_int, c_int, *const c_uchar, c_int) -> c_int,
    pub fn XSync(*mut Display, c_int) -> c_int,
    pub fn XSetErrorHandler(XErrorHandler) -> XErrorHandler,
    pub fn XGetWindowAttributes(*mut Display, Window, *mut XWindowAttributes) -> c_int,
    pub fn XMapWindow(*mut Display, Window) -> c_int,
    pub fn XLowerWindow(*mut Display, Window) -> c_int,
    pub fn XRaiseWindow(*mut Display, Window) -> c_int,
    pub fn XResizeWindow(*mut Display, Window, c_int, c_int) -> c_int,
    pub fn XMoveWindow(*mut Display, Window, c_int, c_int) -> c_int,
    pub fn XPending(*mut Display) -> c_int,
    pub fn XNextEvent(*mut Display, *mut XEvent) -> c_int,
    pub fn XGetKeyboardMapping(*mut Display, KeyCode, c_int, *mut c_int) -> *mut KeySym,
    pub fn XGetWindowProperty(*mut Display, Window, Atom, c_long, c_long, c_int, Atom, *mut Atom, *mut c_int, *mut c_ulong, *mut c_ulong, *mut *mut c_uchar) -> c_int,
    pub fn XDeleteProperty(*mut Display, Window, Atom) -> c_int,
    pub fn XFree(*mut c_void) -> c_int,
    pub fn XUnmapWindow(*mut Display, Window) -> c_int,
    pub fn XDestroyWindow(*mut Display, Window) -> c_int,
    pub fn XFreeColormap(*mut Display, Colormap) -> c_int,
    pub fn XFlush(*mut Display) -> c_int,
    pub fn XCloseDisplay(*mut Display) -> c_int,
    pub fn XGrabPointer(*mut Display, Window, c_int, c_uint, c_int, c_int, Window, Cursor, Time) -> c_int,
    pub fn XUngrabPointer(*mut Display, Time) -> c_int,
    pub fn XSendEvent(*mut Display, Window, c_int, c_long, *mut XEvent) -> c_int,
    pub fn XrmGetResource(XrmDatabase, *const c_char, *const c_char, *mut *mut c_char, *mut XrmValue) -> c_int,
    pub fn XrmDestroyDatabase(XrmDatabase),
    pub fn XrmGetStringDatabase(*const c_char) -> XrmDatabase,
    pub fn XkbSetDetectableAutoRepeat(*mut Display, c_int, *mut c_int) -> c_int,
    pub fn XQueryExtension(*mut Display, *const c_char, *mut c_int, *mut c_int, *mut c_int) -> c_int,
    pub fn XConvertSelection(*mut Display, Atom, Atom, Atom, Window, Time) -> c_int,
    pub fn XSetSelectionOwner(*mut Display, Atom, Window, Time) -> c_int,
    pub fn XCreateFontCursor(*mut Display, c_ushort) -> Cursor,
    pub fn XCreateBitmapFromData(*mut Display, Drawable, *const c_char, c_uint, c_uint) -> Pixmap,
    pub fn XCreatePixmapCursor(*mut Display, Pixmap, Pixmap, *mut XColor, *mut XColor, c_uint, c_uint) -> Cursor,
    pub fn XFreePixmap(*mut Display, Pixmap) -> c_int,
    pub fn XDefineCursor(*mut Display, Window, Cursor) -> c_int,
    ...
    ...
    pub extensions: X11Extensions,
);

impl LibX11 {
    pub unsafe fn load_extensions(&mut self, display: *mut Display) {
        unsafe {
            self.extensions = X11Extensions::load(self, display);
        }
    }
}

crate::declare_module!(
    LibXkbCommon,
    "libxkbcommon.so",
    "libxkbcommon.so.0",
    "libxkbcommon.so.0.0.0",
    "libxkbcommon.so.0.0.0.0",
    ...
    ...
    pub fn xkb_keysym_to_utf32(u32) -> u32,
    ...
    ...
);
