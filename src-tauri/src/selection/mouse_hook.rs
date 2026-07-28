//! Global low-level mouse/keyboard hook (WH_MOUSE_LL / WH_KEYBOARD_LL).
//! Ported from Easydict `MouseHookService.cs` — real edge detection, not polling.

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetAncestor, GetMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, WindowFromPoint, GA_PARENT, GA_ROOT, HHOOK,
    KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_SYSKEYDOWN,
};

/// Match clipboard synthetic marker — keyboard hook ignores our Ctrl+C.
pub const MOON_SYNTHETIC_KEY: usize = 0x4D4F_4F4E; // "MOON"

const MIN_DRAG_DISTANCE: i32 = 10;
const MAX_CLICK_DISTANCE: i32 = 4;

#[derive(Debug, Clone, Copy)]
pub struct ScreenPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone)]
pub enum MouseHookEvent {
    SelectionGesture(ScreenPoint),
    MouseDownOutsidePop,
    MouseDownOnPop,
    MouseScroll,
    RightMouseDown,
    KeyDown,
}

/// Store hook handles as isize so static Mutex is Send+Sync.
struct HookState {
    mouse: isize,
    keyboard: isize,
    tx: Sender<MouseHookEvent>,
}

static HOOK_STATE: Mutex<Option<HookState>> = Mutex::new(None);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static POP_HWND: AtomicIsize = AtomicIsize::new(0);
/// Physical screen rect of pop button (x, y, w, h) — hit-test fallback when HWND tree fails.
static POP_RECT: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);
/// After click on pop, ignore next LMB-up gesture (don't re-fetch selection).
static SUPPRESS_NEXT_GESTURE: AtomicBool = AtomicBool::new(false);
static DOUBLE_CLICK_MS: AtomicU32 = AtomicU32::new(500);
/// Tick count of last real keydown — hover skips briefly after typing.
static LAST_KEY_TICK: AtomicU32 = AtomicU32::new(0);

struct DragDetector {
    start: POINT,
    left_down: bool,
    dragging: bool,
}

impl DragDetector {
    fn new() -> Self {
        Self {
            start: POINT { x: 0, y: 0 },
            left_down: false,
            dragging: false,
        }
    }

    fn on_down(&mut self, pt: POINT) {
        self.start = pt;
        self.left_down = true;
        self.dragging = false;
    }

    fn on_move(&mut self, pt: POINT) {
        if !self.left_down || self.dragging {
            return;
        }
        let dx = (pt.x as i64) - (self.start.x as i64);
        let dy = (pt.y as i64) - (self.start.y as i64);
        let min = MIN_DRAG_DISTANCE as i64;
        if dx * dx + dy * dy >= min * min {
            self.dragging = true;
        }
    }

    fn on_up(&mut self) -> bool {
        let was = self.dragging;
        self.left_down = false;
        self.dragging = false;
        was
    }
}

struct MultiClickDetector {
    count: u32,
    last_at: Instant,
    last_pt: POINT,
    has_last: bool,
}

impl MultiClickDetector {
    fn new() -> Self {
        Self {
            count: 0,
            last_at: Instant::now(),
            last_pt: POINT { x: 0, y: 0 },
            has_last: false,
        }
    }

    fn on_click(&mut self, pt: POINT, dct_ms: u32) -> u32 {
        let elapsed_ms = self.last_at.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
        let within = if self.has_last {
            // i64 to avoid debug overflow (never use i32::MIN sentinel)
            let dx = (pt.x as i64) - (self.last_pt.x as i64);
            let dy = (pt.y as i64) - (self.last_pt.y as i64);
            dx * dx + dy * dy <= (MAX_CLICK_DISTANCE as i64) * (MAX_CLICK_DISTANCE as i64)
        } else {
            false
        };
        if self.has_last && elapsed_ms <= dct_ms && within {
            self.count = self.count.saturating_add(1);
        } else {
            self.count = 1;
        }
        self.last_at = Instant::now();
        self.last_pt = pt;
        self.has_last = true;
        self.count
    }

    fn reset(&mut self) {
        self.count = 0;
        self.last_at = Instant::now();
        self.last_pt = POINT { x: 0, y: 0 };
        self.has_last = false;
    }
}

struct Detectors {
    drag: DragDetector,
    multi: MultiClickDetector,
    multi_gen: u64,
}

fn detectors() -> std::sync::MutexGuard<'static, Detectors> {
    static CELL: OnceLock<Mutex<Detectors>> = OnceLock::new();
    CELL.get_or_init(|| {
        Mutex::new(Detectors {
            drag: DragDetector::new(),
            multi: MultiClickDetector::new(),
            multi_gen: 0,
        })
    })
    .lock()
    .unwrap_or_else(|e| e.into_inner())
}

fn emit(ev: MouseHookEvent) {
    if let Ok(g) = HOOK_STATE.lock() {
        if let Some(s) = g.as_ref() {
            let _ = s.tx.send(ev);
        }
    }
}

fn point_in_pop_rect(pt: POINT) -> bool {
    if let Ok(g) = POP_RECT.lock() {
        if let Some((x, y, w, h)) = *g {
            // Expand hit area for DPI / thin chrome (physical px)
            let pad = 6;
            return pt.x >= x - pad
                && pt.x <= x + w + pad
                && pt.y >= y - pad
                && pt.y <= y + h + pad;
        }
    }
    false
}

fn is_pop_click(pt: POINT) -> bool {
    // 1) Screen rect (most reliable for WebView2 child HWNDs)
    if point_in_pop_rect(pt) {
        return true;
    }
    let pop = POP_HWND.load(Ordering::SeqCst);
    if pop == 0 {
        return false;
    }
    unsafe {
        let at = WindowFromPoint(pt);
        if at.0.is_null() {
            return false;
        }
        let root = GetAncestor(at, GA_ROOT);
        let pop_h = HWND(pop as *mut _);
        if root == pop_h || at == pop_h {
            return true;
        }
        // Walk parents — WebView2 may nest several levels
        let mut cur = at;
        for _ in 0..8 {
            if cur == pop_h {
                return true;
            }
            let parent = GetAncestor(cur, GA_PARENT);
            if parent.0.is_null() || parent == cur {
                break;
            }
            cur = parent;
        }
        false
    }
}

fn process_mouse(message: u32, pt: POINT) {
    match message {
        m if m == WM_LBUTTONDOWN => {
            if is_pop_click(pt) {
                SUPPRESS_NEXT_GESTURE.store(true, Ordering::SeqCst);
                emit(MouseHookEvent::MouseDownOnPop);
                // Still arm drag detector so up is clean, but gesture will be suppressed
                detectors().drag.on_down(pt);
            } else {
                SUPPRESS_NEXT_GESTURE.store(false, Ordering::SeqCst);
                emit(MouseHookEvent::MouseDownOutsidePop);
                detectors().drag.on_down(pt);
            }
        },
        m if m == WM_MOUSEMOVE => {
            detectors().drag.on_move(pt);
        },
        m if m == WM_LBUTTONUP => {
            let suppress = SUPPRESS_NEXT_GESTURE.swap(false, Ordering::SeqCst);
            let mut det = detectors();
            let was_drag = det.drag.on_up();
            if suppress {
                det.multi.reset();
                det.multi_gen = det.multi_gen.wrapping_add(1);
                // Pop click: never start a new selection job
                return;
            }
            if was_drag {
                det.multi.reset();
                det.multi_gen = det.multi_gen.wrapping_add(1);
                drop(det);
                emit(MouseHookEvent::SelectionGesture(ScreenPoint {
                    x: pt.x,
                    y: pt.y,
                }));
            } else {
                let dct = DOUBLE_CLICK_MS.load(Ordering::SeqCst).max(200);
                let count = det.multi.on_click(pt, dct);
                if count >= 2 {
                    det.multi_gen = det.multi_gen.wrapping_add(1);
                    let gen = det.multi_gen;
                    drop(det);
                    let delay = dct as u64 + 50;
                    let sp = ScreenPoint { x: pt.x, y: pt.y };
                    thread::spawn(move || {
                        thread::sleep(Duration::from_millis(delay));
                        if detectors().multi_gen == gen {
                            emit(MouseHookEvent::SelectionGesture(sp));
                        }
                    });
                }
            }
        },
        m if m == WM_MOUSEWHEEL => {
            emit(MouseHookEvent::MouseScroll);
        },
        m if m == WM_RBUTTONDOWN => {
            emit(MouseHookEvent::RightMouseDown);
        },
        _ => {},
    }
}

fn hook_from_isize(v: isize) -> HHOOK {
    HHOOK(v as *mut _)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && lparam.0 != 0 {
        let hs = *(lparam.0 as *const MSLLHOOKSTRUCT);
        process_mouse(wparam.0 as u32, hs.pt);
    }
    let hook = HOOK_STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.mouse))
        .unwrap_or(0);
    CallNextHookEx(hook_from_isize(hook), code, wparam, lparam)
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && lparam.0 != 0 {
        let ks = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        if ks.dwExtraInfo != MOON_SYNTHETIC_KEY {
            let msg = wparam.0 as u32;
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                LAST_KEY_TICK.store(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u32)
                        .unwrap_or(0),
                    Ordering::SeqCst,
                );
                emit(MouseHookEvent::KeyDown);
            }
        }
    }
    let hook = HOOK_STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.keyboard))
        .unwrap_or(0);
    CallNextHookEx(hook_from_isize(hook), code, wparam, lparam)
}

pub fn set_pop_hwnd(hwnd: isize) {
    POP_HWND.store(hwnd, Ordering::SeqCst);
}

/// Physical screen rect of the pop chip (x, y, width, height).
pub fn set_pop_rect(x: i32, y: i32, w: i32, h: i32) {
    if let Ok(mut g) = POP_RECT.lock() {
        *g = Some((x, y, w, h));
    }
}

pub fn clear_pop_hwnd() {
    POP_HWND.store(0, Ordering::SeqCst);
    if let Ok(mut g) = POP_RECT.lock() {
        *g = None;
    }
    SUPPRESS_NEXT_GESTURE.store(false, Ordering::SeqCst);
}

/// True if a non-synthetic key was pressed within the last `ms` milliseconds.
pub fn key_pressed_within_ms(ms: u64) -> bool {
    let last = LAST_KEY_TICK.load(Ordering::SeqCst);
    if last == 0 {
        return false;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u32)
        .unwrap_or(0);
    now.saturating_sub(last) < ms as u32
}

pub fn install() -> Option<Receiver<MouseHookEvent>> {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return None;
    }

    let (tx, rx) = mpsc::channel::<MouseHookEvent>();

    let join = thread::Builder::new()
        .name("moon-mouse-hook".into())
        .spawn(move || unsafe {
            let dct = GetDoubleClickTime();
            DOUBLE_CLICK_MS.store(if dct == 0 { 500 } else { dct }, Ordering::SeqCst);

            let module = match GetModuleHandleW(None) {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("[mouse_hook] GetModuleHandleW: {e}");
                    INSTALLED.store(false, Ordering::SeqCst);
                    return;
                },
            };
            let hmod = HINSTANCE(module.0);

            let mouse = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hmod, 0) {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("[mouse_hook] MOUSE_LL failed: {e}");
                    INSTALLED.store(false, Ordering::SeqCst);
                    return;
                },
            };
            let keyboard = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), hmod, 0) {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("[mouse_hook] KEYBOARD_LL failed: {e}");
                    let _ = UnhookWindowsHookEx(mouse);
                    INSTALLED.store(false, Ordering::SeqCst);
                    return;
                },
            };

            if let Ok(mut g) = HOOK_STATE.lock() {
                *g = Some(HookState {
                    mouse: mouse.0 as isize,
                    keyboard: keyboard.0 as isize,
                    tx,
                });
            }
            tracing::info!("[mouse_hook] WH_MOUSE_LL + WH_KEYBOARD_LL installed");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            if let Ok(mut g) = HOOK_STATE.lock() {
                if let Some(s) = g.take() {
                    let _ = UnhookWindowsHookEx(hook_from_isize(s.mouse));
                    let _ = UnhookWindowsHookEx(hook_from_isize(s.keyboard));
                }
            }
            INSTALLED.store(false, Ordering::SeqCst);
        });

    if join.is_err() {
        INSTALLED.store(false, Ordering::SeqCst);
        return None;
    }

    thread::sleep(Duration::from_millis(40));
    if !INSTALLED.load(Ordering::SeqCst) {
        return None;
    }
    Some(rx)
}

pub fn uninstall() {
    if let Ok(mut g) = HOOK_STATE.lock() {
        if let Some(s) = g.take() {
            unsafe {
                let _ = UnhookWindowsHookEx(hook_from_isize(s.mouse));
                let _ = UnhookWindowsHookEx(hook_from_isize(s.keyboard));
            }
        }
    }
    INSTALLED.store(false, Ordering::SeqCst);
    clear_pop_hwnd();
}
