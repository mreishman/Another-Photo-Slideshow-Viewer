use std::ffi::c_void;

pub type Handle = *mut c_void;
pub type Proc = unsafe extern "system" fn(Handle, u32, usize, isize) -> isize;
pub const WINDOW_STYLE: u32 = 0x00CF0000;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Point { pub x: i32, pub y: i32 }

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Rect { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }
impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { left: x, top: y, right: x + w, bottom: y + h }
    }
    pub fn width(self) -> i32 { self.right - self.left }
    pub fn height(self) -> i32 { self.bottom - self.top }
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.left && p.x < self.right && p.y >= self.top && p.y < self.bottom
    }
}

#[repr(C)]
pub struct Class {
    pub style: u32, pub proc: Proc, pub cls_extra: i32, pub wnd_extra: i32,
    pub instance: Handle, pub icon: Handle, pub cursor: Handle, pub background: Handle,
    pub menu: *const u16, pub name: *const u16,
}

#[repr(C)]
pub struct Message {
    pub window: Handle, pub message: u32, pub wparam: usize, pub lparam: isize,
    pub time: u32, pub pt: Point, pub private: u32,
}

#[repr(C)]
pub struct Paint {
    pub dc: Handle, pub erase: i32, pub area: Rect, pub restore: i32,
    pub update: i32, pub reserved: [u8; 32],
}

#[repr(C)]
pub struct Monitor {
    pub size: u32, pub bounds: Rect, pub work: Rect, pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Placement {
    pub length: u32, pub flags: u32, pub show: u32, pub min: Point,
    pub max: Point, pub normal: Rect,
}

#[repr(C)]
pub struct MinMax {
    pub reserved: Point, pub max_size: Point, pub max_position: Point,
    pub min_track: Point, pub max_track: Point,
}

#[repr(C)]
pub struct Bitmap {
    pub size: u32, pub width: i32, pub height: i32, pub planes: u16,
    pub bits: u16, pub compression: u32, pub image_size: u32, pub xppm: i32,
    pub yppm: i32, pub used: u32, pub important: u32,
}

#[repr(C)]
pub struct Browse {
    pub owner: Handle, pub root: Handle, pub display: *mut u16, pub title: *const u16,
    pub flags: u32, pub callback: Handle, pub arg: isize, pub image: i32,
}

pub fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }
pub fn rgb(r: u32, g: u32, b: u32) -> u32 { r | g << 8 | b << 16 }

#[link(name = "kernel32")]
extern "system" { pub fn GetModuleHandleW(name: *const u16) -> Handle; }

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassW(class: *const Class) -> u16;
    pub fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32,
        x: i32, y: i32, w: i32, h: i32, parent: Handle, menu: Handle, instance: Handle,
        param: Handle) -> Handle;
    pub fn DefWindowProcW(w: Handle, msg: u32, wp: usize, lp: isize) -> isize;
    pub fn ShowWindow(w: Handle, command: i32) -> i32;
    pub fn LoadCursorW(instance: Handle, name: *const u16) -> Handle;
    pub fn GetMessageW(msg: *mut Message, w: Handle, low: u32, high: u32) -> i32;
    pub fn TranslateMessage(msg: *const Message) -> i32;
    pub fn DispatchMessageW(msg: *const Message) -> isize;
    pub fn PostQuitMessage(code: i32);
    pub fn BeginPaint(w: Handle, paint: *mut Paint) -> Handle;
    pub fn EndPaint(w: Handle, paint: *const Paint) -> i32;
    pub fn GetClientRect(w: Handle, rect: *mut Rect) -> i32;
    pub fn InvalidateRect(w: Handle, rect: *const Rect, erase: i32) -> i32;
    pub fn FillRect(dc: Handle, area: *const Rect, brush: Handle) -> i32;
    pub fn DrawTextW(dc: Handle, text: *const u16, len: i32, rect: *mut Rect, flags: u32) -> i32;
    pub fn SetCapture(window: Handle) -> Handle;
    pub fn ReleaseCapture() -> i32;
    pub fn SetTimer(w: Handle, id: usize, ms: u32, callback: Handle) -> usize;
    pub fn KillTimer(w: Handle, id: usize) -> i32;
    pub fn GetKeyState(key: i32) -> i16;
    pub fn GetWindowLongW(w: Handle, index: i32) -> i32;
    pub fn SetWindowLongW(w: Handle, index: i32, value: i32) -> i32;
    pub fn GetWindowPlacement(w: Handle, placement: *mut Placement) -> i32;
    pub fn SetWindowPlacement(w: Handle, placement: *const Placement) -> i32;
    pub fn SetWindowPos(w: Handle, insert: Handle, x: i32, y: i32, width: i32, height: i32, flags: u32) -> i32;
    pub fn MonitorFromWindow(w: Handle, flags: u32) -> Handle;
    pub fn GetMonitorInfoW(monitor: Handle, info: *mut Monitor) -> i32;
    pub fn SetProcessDpiAwarenessContext(context: Handle) -> i32;
    pub fn GetDpiForWindow(w: Handle) -> u32;
    pub fn CreatePopupMenu() -> Handle;
    pub fn AppendMenuW(menu: Handle, flags: u32, id: usize, text: *const u16) -> i32;
    pub fn TrackPopupMenu(menu: Handle, flags: u32, x: i32, y: i32, reserved: i32, w: Handle, rect: *const Rect) -> u32;
    pub fn DestroyMenu(menu: Handle) -> i32;
    pub fn ClientToScreen(w: Handle, p: *mut Point) -> i32;
    pub fn MessageBoxW(w: Handle, text: *const u16, title: *const u16, flags: u32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn SaveDC(dc: Handle) -> i32;
    pub fn RestoreDC(dc: Handle, saved: i32) -> i32;
    pub fn IntersectClipRect(dc: Handle, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    pub fn CreateSolidBrush(color: u32) -> Handle;
    pub fn DeleteObject(object: Handle) -> i32;
    pub fn CreateCompatibleDC(dc: Handle) -> Handle;
    pub fn DeleteDC(dc: Handle) -> i32;
    pub fn CreateCompatibleBitmap(dc: Handle, w: i32, h: i32) -> Handle;
    pub fn SelectObject(dc: Handle, object: Handle) -> Handle;
    pub fn BitBlt(dst: Handle, x: i32, y: i32, w: i32, h: i32, src: Handle, sx: i32, sy: i32, op: u32) -> i32;
    pub fn StretchDIBits(dc: Handle, x: i32, y: i32, w: i32, h: i32, sx: i32, sy: i32,
        sw: i32, sh: i32, bits: *const c_void, header: *const Bitmap, usage: u32, op: u32) -> i32;
    pub fn SetStretchBltMode(dc: Handle, mode: i32) -> i32;
    pub fn SetBkMode(dc: Handle, mode: i32) -> i32;
    pub fn SetTextColor(dc: Handle, color: u32) -> u32;
    pub fn CreateFontW(height: i32, width: i32, escapement: i32, orientation: i32,
        weight: i32, italic: u32, underline: u32, strike: u32, charset: u32,
        output: u32, clip: u32, quality: u32, family: u32, face: *const u16) -> Handle;
}

#[link(name = "shell32")]
extern "system" {
    pub fn DragAcceptFiles(window: Handle, accept: i32);
    pub fn DragQueryFileW(drop: Handle, index: u32, path: *mut u16, length: u32) -> u32;
    pub fn DragFinish(drop: Handle);
    pub fn SHBrowseForFolderW(info: *mut Browse) -> Handle;
    pub fn SHGetPathFromIDListEx(pidl: Handle, path: *mut u16, length: u32, flags: u32) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    pub fn CoInitializeEx(reserved: Handle, mode: u32) -> i32;
    pub fn CoUninitialize();
    pub fn CoTaskMemFree(memory: Handle);
}

#[link(name = "dwmapi")]
extern "system" {
    pub fn DwmSetWindowAttribute(w: Handle, attr: u32, value: *const c_void, size: u32) -> i32;
}

