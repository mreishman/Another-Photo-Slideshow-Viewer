#![windows_subsystem = "windows"]

mod photos;
mod win;

use photos::{Job, Output, Photo, Worker};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::OsString,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    ptr::{null, null_mut},
    sync::Arc,
    time::{Duration, Instant},
};
use win::*;

const BACK: u32 = 0x14100e;
const PANEL: u32 = 0x26211c;
const LINE: u32 = 0x403831;
const TEXT: u32 = 0xf5f3f0;
const MUTED: u32 = 0xb8aea4;
const ACCENT: u32 = 0xf5d34f;
const INTERVALS: [u64; 4] = [3, 5, 10, 30];

thread_local! { static APP: RefCell<Option<App>> = const { RefCell::new(None) }; }

struct App {
    files: Vec<PathBuf>,
    index: usize,
    frame: Option<Arc<Photo>>,
    thumbs: HashMap<usize, Option<Photo>>,
    failed: HashSet<usize>,
    worker: Worker,
    generation: u64,
    playing: bool,
    loading: bool,
    interval: u64,
    deadline: Instant,
    status: String,
    loop_slides: bool,
    placement: Option<(Placement, i32)>,
    overlay_until: Option<Instant>,
    pointer: Point,
    focus: Option<Action>,
}

#[derive(Clone, Copy, PartialEq)]
enum Action { Open, Settings, Previous, Play, Next, Interval, Fullscreen, Thumb(usize) }

impl App {
    fn new() -> Self {
        Self {
            files: Vec::new(), index: 0, frame: None, thumbs: HashMap::new(),
            failed: HashSet::new(), worker: Worker::new(), generation: 0,
            playing: false, loading: false, interval: 5, deadline: Instant::now(),
            status: String::new(), loop_slides: true, placement: None,
            overlay_until: None, pointer: Point { x: -1, y: -1 }, focus: None,
        }
    }

    fn fullscreen(&self) -> bool { self.placement.is_some() }
    fn overlay(&self) -> bool { self.overlay_until.is_some() || self.focus.is_some() }
    fn reveal(&mut self) { self.overlay_until = Some(Instant::now() + Duration::from_secs(3)); }
    fn reset_timer(&mut self) { self.deadline = Instant::now() + Duration::from_secs(self.interval); }

    fn set_folder(&mut self, path: PathBuf) {
        match photos::folder(&path) {
            Ok(files) => {
                self.files = files;
                self.index = 0;
                self.frame = None;
                self.thumbs.clear();
                self.failed.clear();
                self.playing = false;
                self.status = if self.files.is_empty() { "This folder has no JPEG, PNG, or BMP photos.".into() } else { String::new() };
                self.request();
            }
            Err(error) => self.status = format!("Could not open folder: {error}"),
        }
    }

    fn first_thumb(&self) -> usize {
        self.index.saturating_sub(2).min(self.files.len().saturating_sub(5))
    }

    fn request(&mut self) {
        self.generation += 1;
        let start = self.first_thumb();
        let end = (start + 5).min(self.files.len());
        self.thumbs.retain(|index, _| (start..end).contains(index));
        let thumbs = (start..end).filter(|i| !self.thumbs.contains_key(i))
            .map(|i| (i, self.files[i].clone())).collect();
        let selected = self.files.get(self.index).cloned().map(|p| (self.index, p));
        self.loading = selected.is_some();
        let prefetch = self.next_index(1).map(|i| self.files[i].clone());
        self.worker.request(Job { generation: self.generation, selected, thumbs, prefetch });
    }

    fn next_index(&self, direction: isize) -> Option<usize> {
        let length = self.files.len() as isize;
        let mut index = self.index as isize;
        for _ in 1..length {
            index += direction;
            if !(0..length).contains(&index) {
                if !self.loop_slides { return None; }
                index = index.rem_euclid(length);
            }
            if !self.failed.contains(&(index as usize)) { return Some(index as usize); }
        }
        None
    }

    fn select(&mut self, index: usize) {
        self.index = index;
        self.status.clear();
        self.reset_timer();
        self.request();
    }

    fn step(&mut self, direction: isize) {
        if let Some(index) = self.next_index(direction) {
            self.select(index);
        } else if direction > 0 {
            self.playing = false;
        }
    }

    fn toggle(&mut self) {
        if self.files.len() > 1 {
            self.playing = !self.playing;
            self.reset_timer();
        }
    }

    fn tick(&mut self) -> bool {
        let mut redraw = false;
        while let Ok((generation, output)) = self.worker.results.try_recv() {
            if generation != self.generation { continue; }
            match output {
                Output::Selected(index, result) => {
                    self.loading = false;
                    match result {
                        Ok(photo) => {
                            self.failed.remove(&index);
                            self.frame = Some(photo);
                            self.reset_timer();
                        }
                        Err(error) => {
                            self.failed.insert(index);
                            let filename = self.files[index].file_name().unwrap_or_default().to_string_lossy();
                            self.status = format!("Could not open {filename}: {error}");
                            if self.playing && self.next_index(1).is_some() {
                                self.step(1);
                            } else {
                                self.playing = false;
                            }
                        }
                    }
                }
                Output::Thumb(index, photo) => { self.thumbs.insert(index, photo); }
            }
            redraw = true;
        }
        if self.playing && !self.loading && Instant::now() >= self.deadline {
            self.step(1);
            redraw = true;
        }
        if self.fullscreen() && self.focus.is_none() && self.overlay_until.is_some_and(|deadline| Instant::now() >= deadline) {
            self.overlay_until = None;
            redraw = true;
        }
        redraw
    }

}


struct Layout {
    canvas: Rect,
    bar: Rect,
    tray: Rect,
    controls: Vec<(Action, Rect)>,
    counter: Rect,
    hint: Rect,
}

impl Layout {
    fn new(area: Rect, scale: i32, app: &App) -> Self {
        let d = |n| n * scale / 96;
        let (w, h) = (area.width(), area.height());
        let full = app.fullscreen();
        let tray_height = if app.files.is_empty() { 0 } else { d(100) };
        let footer = h - d(58);
        let tray_top = footer - tray_height;
        let mut controls = Vec::new();
        let mut layout = if full {
            Self {
                canvas: area, bar: Rect::default(), tray: Rect::default(), controls: Vec::new(),
                counter: Rect::new(w / 2 + d(71), h - d(61), d(75), d(38)),
                hint: Rect::new((w - d(168)).max(0), h - d(54), d(153), d(27)),
            }
        } else {
            controls.push((Action::Open, Rect::new(d(14), d(7), d(133), d(35))));
            controls.push((Action::Settings, Rect::new(w - d(112), d(7), d(98), d(35))));
            let start = app.first_thumb();
            let slots = (app.files.len() - start).min(5) as i32;
            let tile = (w - d(20) - d(7) * slots.saturating_sub(1)) / slots.max(1);
            for slot in 0..slots {
                controls.push((Action::Thumb(start + slot as usize),
                    Rect::new(d(10) + slot * (tile + d(7)), tray_top + d(8), tile, tray_height - d(16))));
            }
            Self {
                canvas: Rect::new(0, d(48), w, (tray_top - d(48)).max(0)),
                bar: Rect::new(0, 0, w, d(48)),
                tray: Rect::new(0, tray_top, w, tray_height), controls: Vec::new(),
                counter: Rect::new(w / 2 + d(119), footer + d(10), d(82), d(38)),
                hint: Rect::default(),
            }
        };
        if !full || app.overlay() {
            let left = w / 2 - d(if full { 145 } else { 199 });
            let top = if full { h - d(61) } else { footer + d(10) };
            for (i, action) in [Action::Previous, Action::Play, Action::Next].into_iter().enumerate() {
                controls.push((action, Rect::new(left + d(67) * i as i32, top, d(59), d(38))));
            }
            if !full {
                controls.push((Action::Interval, Rect::new(w / 2 + d(21), top, d(86), d(38))));
                controls.push((Action::Fullscreen, Rect::new(w - d(62), top, d(48), d(38))));
            }
        }
        layout.controls = controls;
        layout
    }
}

unsafe fn client(window: Handle) -> Rect {
    let mut area = Rect::default();
    GetClientRect(window, &mut area);
    area
}

unsafe fn dpi(window: Handle) -> i32 { GetDpiForWindow(window).max(96) as i32 }

unsafe fn fill(dc: Handle, area: Rect, color: u32) {
    if area.width() <= 0 || area.height() <= 0 { return; }
    let brush = CreateSolidBrush(color);
    if !brush.is_null() {
        FillRect(dc, &area, brush);
        DeleteObject(brush);
    }
}

unsafe fn label(dc: Handle, mut area: Rect, value: &str, color: u32) {
    SetTextColor(dc, color);
    let chars = wide(value);
    DrawTextW(dc, chars.as_ptr(), -1, &mut area, 0x8825);
}

unsafe fn fitted(dc: Handle, area: Rect, photo: &Photo) {
    if area.width() <= 0 || area.height() <= 0 { return; }
    let ratio = (f64::from(area.width()) / f64::from(photo.width))
        .min(f64::from(area.height()) / f64::from(photo.height));
    let w = (f64::from(photo.width) * ratio).round().max(1.0) as i32;
    let h = (f64::from(photo.height) * ratio).round().max(1.0) as i32;
    let info = Bitmap {
        size: size_of::<Bitmap>() as u32, width: photo.width, height: -photo.height,
        planes: 1, bits: 32, compression: 0, image_size: 0, xppm: 0, yppm: 0,
        used: 0, important: 0,
    };
    StretchDIBits(dc, area.left + (area.width() - w) / 2,
        area.top + (area.height() - h) / 2, w, h, 0, 0, photo.width, photo.height,
        photo.pixels.as_ptr().cast(), &info, 0, 0x00CC0020);
}

fn inset(area: Rect, amount: i32) -> Rect {
    Rect::new(area.left + amount, area.top + amount, area.width() - 2 * amount, area.height() - 2 * amount)
}

unsafe fn draw(dc: Handle, area: Rect, scale: i32, app: &App) {
    let d = |n| n * scale / 96;
    let layout = Layout::new(area, scale, app);
    let font = CreateFontW(-d(17), 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe UI").as_ptr());
    let old_font = if font.is_null() { null_mut() } else { SelectObject(dc, font) };
    SetBkMode(dc, 1);
    SetStretchBltMode(dc, 3);
    fill(dc, area, BACK);
    if let Some(photo) = &app.frame {
        fitted(dc, layout.canvas, photo);
    } else if app.status.is_empty() {
        let prompt = if app.loading { "Loading photo…" } else { "Open a folder to view your photos" };
        label(dc, layout.canvas, prompt, MUTED);
    }
    if !app.fullscreen() {
        fill(dc, layout.bar, PANEL);
        fill(dc, layout.tray, PANEL);
        let footer_top = area.bottom - d(58);
        fill(dc, Rect::new(0, footer_top, area.width(), d(58)), PANEL);
        fill(dc, Rect::new(0, footer_top, area.width(), d(1).max(1)), LINE);
    } else if app.overlay() {
        fill(dc, Rect::new(area.width() / 2 - d(156), area.bottom - d(68), d(315), d(52)), PANEL);
        fill(dc, layout.hint, PANEL);
        label(dc, layout.hint, "Esc  Exit fullscreen", TEXT);
    }
    for (action, bounds) in layout.controls {
        if let Action::Thumb(index) = action {
            let border = if index == app.index || app.focus == Some(action) { ACCENT } else { LINE };
            fill(dc, bounds, border);
            let inside = inset(bounds, d(2).max(1));
            fill(dc, inside, BACK);
            if let Some(Some(photo)) = app.thumbs.get(&index) {
                fitted(dc, inside, photo);
            } else {
                label(dc, inside, if app.thumbs.contains_key(&index) { "Unavailable" } else { "…" }, MUTED);
            }
            continue;
        }
        let disabled = match action {
            Action::Previous => app.next_index(-1).is_none(),
            Action::Next => app.next_index(1).is_none(),
            Action::Play => app.files.len() < 2,
            _ => false,
        };
        let border = if app.focus == Some(action) { ACCENT } else { LINE };
        let face = if bounds.contains(app.pointer) && !disabled { rgb(57, 67, 77) } else { rgb(42, 49, 57) };
        fill(dc, bounds, border);
        fill(dc, inset(bounds, d(1).max(1)), face);
        let caption = match action {
            Action::Open => "Open folder".to_owned(),
            Action::Settings => "Settings".to_owned(),
            Action::Previous => "◀|".to_owned(),
            Action::Play if app.playing => "Ⅱ".to_owned(),
            Action::Play => "▶".to_owned(),
            Action::Next => "|▶".to_owned(),
            Action::Interval => format!("{} s  ▾", app.interval),
            Action::Fullscreen => "⛶".to_owned(),
            Action::Thumb(_) => continue,
        };
        label(dc, bounds, &caption, if disabled { MUTED } else { TEXT });
    }
    if !app.fullscreen() || app.overlay() {
        let index = if app.files.is_empty() { 0 } else { app.index + 1 };
        label(dc, layout.counter, &format!("{index} / {}", app.files.len()), TEXT);
    }
    if !app.status.is_empty() || (app.loading && app.frame.is_some()) {
        let status = if app.status.is_empty() { "Loading photo…" } else { &app.status };
        let bounds = Rect::new(d(10), layout.canvas.top + d(10), (area.width() - d(20)).max(0), d(34));
        fill(dc, bounds, PANEL);
        label(dc, inset(bounds, d(5)), status, TEXT);
    }
    if !font.is_null() {
        SelectObject(dc, old_font);
        DeleteObject(font);
    }
}

unsafe fn paint(window: Handle) {
    let mut data: Paint = zeroed();
    let target = BeginPaint(window, &mut data);
    let area = client(window);
    if !target.is_null() && area.width() > 0 && area.height() > 0 {
        let memory = CreateCompatibleDC(target);
        let bitmap = if memory.is_null() { null_mut() } else {
            CreateCompatibleBitmap(target, area.width(), area.height())
        };
        let buffered = !bitmap.is_null();
        let previous = if buffered { SelectObject(memory, bitmap) } else { null_mut() };
        APP.with(|state| {
            if let Ok(state) = state.try_borrow() {
                if let Some(app) = state.as_ref() {
                    draw(if buffered { memory } else { target }, area, dpi(window), app);
                }
            }
        });
        if buffered {
            BitBlt(target, 0, 0, area.width(), area.height(), memory, 0, 0, 0x00CC0020);
            SelectObject(memory, previous);
            DeleteObject(bitmap);
        }
        if !memory.is_null() { DeleteDC(memory); }
    }
    EndPaint(window, &data);
}

unsafe fn choose_folder(window: Handle) -> Option<PathBuf> {
    let title = wide("Choose a folder of photos");
    let mut display = [0u16; 260];
    let mut browse = Browse {
        owner: window, root: null_mut(), display: display.as_mut_ptr(), title: title.as_ptr(),
        flags: 0x41, callback: null_mut(), arg: 0, image: 0,
    };
    let item = SHBrowseForFolderW(&mut browse);
    if item.is_null() { return None; }
    let mut path = vec![0u16; 32_768];
    let found = SHGetPathFromIDListEx(item, path.as_mut_ptr(), path.len() as u32, 0);
    CoTaskMemFree(item);
    if found == 0 { return None; }
    let length = path.iter().position(|c| *c == 0)?;
    Some(PathBuf::from(OsString::from_wide(&path[..length])))
}

unsafe fn fullscreen(window: Handle) {
    let saved = APP.with(|state| state.borrow().as_ref().and_then(|app| app.placement));
    if let Some((placement, style)) = saved {
        APP.with(|state| {
            let mut state = state.borrow_mut();
            let app = state.as_mut().unwrap();
            app.placement = None;
            app.focus = None;
        });
        SetWindowLongW(window, -16, style);
        SetWindowPlacement(window, &placement);
        SetWindowPos(window, null_mut(), 0, 0, 0, 0, 0x37);
        return;
    }
    let mut placement: Placement = zeroed();
    placement.length = size_of::<Placement>() as u32;
    let mut monitor: Monitor = zeroed();
    monitor.size = size_of::<Monitor>() as u32;
    if GetWindowPlacement(window, &mut placement) == 0 ||
        GetMonitorInfoW(MonitorFromWindow(window, 2), &mut monitor) == 0 {
        return;
    }
    let style = GetWindowLongW(window, -16);
    APP.with(|state| {
        let mut state = state.borrow_mut();
        let app = state.as_mut().unwrap();
        app.placement = Some((placement, style));
        app.focus = None;
        app.reveal();
    });
    SetWindowLongW(window, -16, (style & !(WINDOW_STYLE as i32)) | (0x80000000u32 as i32));
    let bounds = monitor.bounds;
    SetWindowPos(window, null_mut(), bounds.left, bounds.top, bounds.width(), bounds.height(), 0x60);
}

unsafe fn settings(window: Handle, action: Action) {
    let (interval, looping, mut point) = APP.with(|state| {
        let state = state.borrow();
        let app = state.as_ref().unwrap();
        let layout = Layout::new(client(window), dpi(window), app);
        let bounds = layout.controls.iter().find(|(item, _)| *item == action)
            .map(|(_, bounds)| *bounds).unwrap_or_default();
        (app.interval, app.loop_slides, Point { x: bounds.left, y: bounds.bottom })
    });
    let menu = CreatePopupMenu();
    if menu.is_null() { return; }
    for (i, seconds) in INTERVALS.iter().enumerate() {
        AppendMenuW(menu, if *seconds == interval { 8 } else { 0 }, i + 1,
            wide(&format!("{seconds} seconds per photo")).as_ptr());
    }
    if action == Action::Settings {
        AppendMenuW(menu, 0x800, 0, null());
        AppendMenuW(menu, if looping { 8 } else { 0 }, 5, wide("Loop slideshow").as_ptr());
    }
    ClientToScreen(window, &mut point);
    let command = TrackPopupMenu(menu, 0x180, point.x, point.y, 0, window, null());
    DestroyMenu(menu);
    APP.with(|state| {
        let mut state = state.borrow_mut();
        let app = state.as_mut().unwrap();
        match command {
            1..=4 => {
                app.interval = INTERVALS[command as usize - 1];
                app.reset_timer();
            }
            5 => app.loop_slides = !app.loop_slides,
            _ => {}
        }
    });
}

unsafe fn activate(window: Handle, action: Action) {
    match action {
        Action::Open => {
            let was_playing = APP.with(|state| {
                let mut state = state.borrow_mut();
                let app = state.as_mut().unwrap();
                std::mem::replace(&mut app.playing, false)
            });
            let path = choose_folder(window);
            APP.with(|state| {
                let mut state = state.borrow_mut();
                let app = state.as_mut().unwrap();
                if let Some(path) = path {
                    app.set_folder(path);
                } else {
                    app.playing = was_playing;
                    app.reset_timer();
                }
            });
        }
        Action::Settings | Action::Interval => settings(window, action),
        Action::Fullscreen => fullscreen(window),
        action => APP.with(|state| {
            let mut state = state.borrow_mut();
            let app = state.as_mut().unwrap();
            match action {
                Action::Previous => app.step(-1),
                Action::Next => app.step(1),
                Action::Play => app.toggle(),
                Action::Thumb(index) if index < app.files.len() => app.select(index),
                _ => {}
            }
            app.reveal();
        }),
    }
    InvalidateRect(window, null(), 0);
}

unsafe fn key(window: Handle, key: usize) -> bool {
    let action = match key {
        0x4f if GetKeyState(0x11) < 0 => Some(Action::Open),
        0x7a => Some(Action::Fullscreen),
        0x25 => Some(Action::Previous),
        0x27 => Some(Action::Next),
        0x20 => Some(Action::Play),
        0x0d => APP.with(|state| state.borrow().as_ref().and_then(|app| app.focus)),
        0x1b => APP.with(|state| state.borrow().as_ref().filter(|app| app.fullscreen()).map(|_| Action::Fullscreen)),
        9 => {
            let backwards = GetKeyState(0x10) < 0;
            APP.with(|state| {
                let mut state = state.borrow_mut();
                let app = state.as_mut().unwrap();
                app.reveal();
                let controls = Layout::new(client(window), dpi(window), app).controls;
                let length = controls.len();
                if length == 0 { return; }
                let position = controls.iter().position(|(action, _)| Some(*action) == app.focus);
                let index = match (position, backwards) {
                    (Some(i), true) => (i + length - 1) % length,
                    (Some(i), false) => (i + 1) % length,
                    (None, true) => length - 1,
                    (None, false) => 0,
                };
                app.focus = Some(controls[index].0);
            });
            InvalidateRect(window, null(), 0);
            return true;
        }
        _ => None,
    };
    if let Some(action) = action {
        activate(window, action);
        true
    } else {
        false
    }
}

unsafe extern "system" fn window_proc(window: Handle, msg: u32, wp: usize, lp: isize) -> isize {
    match msg {
        0x0002 => {
            KillTimer(window, 1);
            PostQuitMessage(0);
        }
        0x0005 => { InvalidateRect(window, null(), 0); }
        0x0008 => {
            APP.with(|state| {
                if let Ok(mut state) = state.try_borrow_mut() {
                    if let Some(app) = state.as_mut() { app.focus = None; }
                }
            });
            InvalidateRect(window, null(), 0);
        }
        0x000f => paint(window),
        0x0014 => return 1,
        0x0024 => {
            let minimum = &mut *(lp as *mut MinMax);
            minimum.min_track = Point { x: 600 * dpi(window) / 96, y: 390 * dpi(window) / 96 };
        }
        0x0100 if key(window, wp) => {}
        0x0113 if wp == 1 => {
            let redraw = APP.with(|state| state.borrow_mut().as_mut().is_some_and(App::tick));
            if redraw { InvalidateRect(window, null(), 0); }
        }
        0x0200 | 0x0202 => {
            let pointer = Point { x: lp as i16 as i32, y: (lp >> 16) as i16 as i32 };
            let action = APP.with(|state| {
                let mut state = state.borrow_mut();
                let app = state.as_mut()?;
                app.pointer = pointer;
                let layout = Layout::new(client(window), dpi(window), app);
                app.reveal();
                if msg == 0x0202 {
                    app.focus = None;
                    return layout.controls.into_iter().find(|(_, rect)| rect.contains(pointer)).map(|(action, _)| action);
                }
                None
            });
            if let Some(action) = action { activate(window, action); }
            InvalidateRect(window, null(), 0);
        }
        0x02e0 => {
            let bounds = *(lp as *const Rect);
            let full = APP.with(|state| state.try_borrow().ok().and_then(|s| s.as_ref().map(App::fullscreen)).unwrap_or(false));
            if !full {
                SetWindowPos(window, null_mut(), bounds.left, bounds.top, bounds.width(), bounds.height(), 0x14);
            }
            InvalidateRect(window, null(), 0);
        }
        _ => return DefWindowProcW(window, msg, wp, lp),
    }
    0
}

fn main() {
    unsafe {
        SetProcessDpiAwarenessContext(-4isize as Handle);
        let com = CoInitializeEx(null_mut(), 2);
        let instance = GetModuleHandleW(null());
        let name = wide("AnotherPhotoSlideshowViewerWindow");
        let class = Class {
            style: 0, proc: window_proc, cls_extra: 0, wnd_extra: 0,
            instance, icon: null_mut(), cursor: LoadCursorW(null_mut(), 32512usize as *const u16),
            background: null_mut(), menu: null(), name: name.as_ptr(),
        };
        let window = if RegisterClassW(&class) == 0 {
            null_mut()
        } else {
            CreateWindowExW(0, name.as_ptr(), wide("Another Photo Slideshow Viewer").as_ptr(), WINDOW_STYLE,
                i32::MIN, i32::MIN, 1100, 740, null_mut(), null_mut(), instance, null_mut())
        };
        if window.is_null() {
            MessageBoxW(null_mut(), wide("Could not create the Another Photo Slideshow Viewer window.").as_ptr(),
                wide("Another Photo Slideshow Viewer").as_ptr(), 0x10);
        } else {
            let mut app = App::new();
            if let Some(path) = std::env::args_os().nth(1) {
                app.set_folder(PathBuf::from(path));
            }
            APP.with(|state| *state.borrow_mut() = Some(app));
            let dark = 1i32;
            DwmSetWindowAttribute(window, 20, (&dark as *const i32).cast(), 4);
            SetTimer(window, 1, 45, null_mut());
            ShowWindow(window, 5);
            let mut message: Message = zeroed();
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            APP.with(|state| *state.borrow_mut() = None);
        }
        if com >= 0 { CoUninitialize(); }
    }
}


