#![windows_subsystem = "console"]

use std::{
    ffi::OsStr,
    iter::once,
    os::windows::ffi::OsStrExt,
    ptr::null_mut,
    sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU32, Ordering},
    time::{Duration, Instant},
};

use windows::{minwindef::*, windef::*, wingdi::*, winuser::*, sysinfoapi::*};
use windows_core::PCWSTR;
use windows_sys::{libloaderapi::GetModuleHandleW, minwindef::ATOM, w};

static APP_HWND: AtomicIsize = AtomicIsize::new(0);
static LAST_MESSAGE: AtomicU32 = AtomicU32::new(0);
static LAST_MOUSE_X: AtomicI32 = AtomicI32::new(-1);
static LAST_MOUSE_Y: AtomicI32 = AtomicI32::new(-1);
static IN_SIZE_MOVE: AtomicBool = AtomicBool::new(false);
static INPUT_COUNT: AtomicU32 = AtomicU32::new(0);
static MOVE_COUNT: AtomicU32 = AtomicU32::new(0);
static BUTTON_COUNT: AtomicU32 = AtomicU32::new(0);
static KEY_COUNT: AtomicU32 = AtomicU32::new(0);

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

fn message_name(msg: u32) -> &'static str {
    match msg {
        WM_MOUSEMOVE => "WM_MOUSEMOVE",
        WM_NCMOUSEMOVE => "WM_NCMOUSEMOVE",
        WM_LBUTTONDOWN => "WM_LBUTTONDOWN",
        WM_LBUTTONUP => "WM_LBUTTONUP",
        WM_NCLBUTTONDOWN => "WM_NCLBUTTONDOWN",
        WM_NCLBUTTONUP => "WM_NCLBUTTONUP",
        WM_KEYDOWN => "WM_KEYDOWN",
        WM_KEYUP => "WM_KEYUP",
        WM_SYSKEYDOWN => "WM_SYSKEYDOWN",
        WM_SYSKEYUP => "WM_SYSKEYUP",
        WM_CHAR => "WM_CHAR",
        WM_SETFOCUS => "WM_SETFOCUS",
        WM_KILLFOCUS => "WM_KILLFOCUS",
        WM_ACTIVATE => "WM_ACTIVATE",
        WM_CAPTURECHANGED => "WM_CAPTURECHANGED",
        WM_ENTERSIZEMOVE => "WM_ENTERSIZEMOVE",
        WM_EXITSIZEMOVE => "WM_EXITSIZEMOVE",
        WM_SYSCOMMAND => "WM_SYSCOMMAND",
        WM_SIZE => "WM_SIZE",
        _ => "other",
    }
}

fn log_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) {
    match msg {
        WM_MOUSEMOVE | WM_NCMOUSEMOVE => {
            MOVE_COUNT.fetch_add(1, Ordering::Relaxed);
            INPUT_COUNT.fetch_add(1, Ordering::Relaxed);

            let x = (lparam.0 as i16) as i32;
            let y = ((lparam.0 >> 16) as i16) as i32;
            LAST_MOUSE_X.store(x, Ordering::Relaxed);
            LAST_MOUSE_Y.store(y, Ordering::Relaxed);
            println!(
                "{:<18} {x} {y}",
                message_name(msg),
            );
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_NCLBUTTONDOWN | WM_NCLBUTTONUP => {
            BUTTON_COUNT.fetch_add(1, Ordering::Relaxed);
            INPUT_COUNT.fetch_add(1, Ordering::Relaxed);
            println!(
                "{:<18} hwnd={:?} wp=0x{:x} lp=0x{:x}",
                message_name(msg),
                hwnd,
                wparam.0,
                lparam.0
            );
        }
        WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP | WM_CHAR => {
            KEY_COUNT.fetch_add(1, Ordering::Relaxed);
            INPUT_COUNT.fetch_add(1, Ordering::Relaxed);
            println!(
                "{:<18} hwnd={:?} wp=0x{:x} lp=0x{:x}",
                message_name(msg),
                hwnd,
                wparam.0,
                lparam.0
            );
        }
        WM_ENTERSIZEMOVE => {
            IN_SIZE_MOVE.store(true, Ordering::Relaxed);
            println!("WM_ENTERSIZEMOVE hwnd={hwnd:?}");
        }
        WM_EXITSIZEMOVE => {
            IN_SIZE_MOVE.store(false, Ordering::Relaxed);
            println!("WM_EXITSIZEMOVE  hwnd={hwnd:?}");
        }
        WM_CAPTURECHANGED | WM_SETFOCUS | WM_KILLFOCUS | WM_ACTIVATE | WM_SYSCOMMAND => {
            println!(
                "{:<18} hwnd={:?} wp=0x{:x} lp=0x{:x}",
                message_name(msg),
                hwnd,
                wparam.0,
                lparam.0
            );
        }
        WM_SIZE if wparam.0 == SIZE_MINIMIZED as usize => {
            println!("WM_SIZE minimized hwnd={hwnd:?}");
        }
        _ => {}
    }

    LAST_MESSAGE.store(msg, Ordering::Relaxed);
}

unsafe fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);

    let background = CreateSolidBrush(COLORREF(0x00_20_20_20));
    FillRect(hdc, &client, background);
    let _ = DeleteObject(HGDIOBJ(background.0));

    // Moving marker makes it obvious that repainting continues even if input stops.
    let tick = GetTickCount();
    let width = (client.right - client.left).max(1);
    let marker_x = 20 + ((tick / 8) as i32 % (width - 60).max(1));

    let marker_rect = RECT {
        left: marker_x,
        top: 80,
        right: marker_x + 40,
        bottom: 120,
    };
    let marker = CreateSolidBrush(COLORREF(0x00_60_C0_FF));
    FillRect(hdc, &marker_rect, marker);
    let _ = DeleteObject(HGDIOBJ(marker.0));

    let mut cursor = POINT::default();
    let _ = GetCursorPos(&mut cursor);
    let target = WindowFromPoint(cursor);

    let mut client_cursor = cursor;
    let _ = ScreenToClient(hwnd, &mut client_cursor);

    let mut gui = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    let _ = GetGUIThreadInfo(0, &mut gui);

    let async_space = (GetAsyncKeyState(0x20) as u16 & 0x8000) != 0;
    let async_escape = (GetAsyncKeyState(0x1B) as u16 & 0x8000) != 0;

    let text = format!(
        "Native Win32 input test — drag the title bar, then move/click/type\n\
         \n\
         App HWND:              {:?}\n\
         Cursor screen:         ({}, {})\n\
         Cursor client:         ({}, {})\n\
         WindowFromPoint:       {:?}\n\
         Capture:               {:?}\n\
         Foreground:            {:?}\n\
         Active:                {:?}\n\
         Focus:                 {:?}\n\
         GUI hwndMoveSize:      {:?}\n\
         GUI hwndMenuOwner:     {:?}\n\
         In WM_ENTERSIZEMOVE:   {}\n\
         \n\
         Input messages:        {}\n\
         Mouse moves:           {}\n\
         Mouse buttons:         {}\n\
         Keyboard messages:     {}\n\
         Last mouse lParam:     ({}, {})\n\
         Last message:          0x{:04x} ({})\n\
         \n\
         GetAsyncKeyState SPACE: {}\n\
         GetAsyncKeyState ESC:   {}\n\
         \n\
         The moving rectangle proves WM_PAINT/timers are still running.\n\
         Press ESC to quit when keyboard delivery works.",
        hwnd,
        cursor.x,
        cursor.y,
        client_cursor.x,
        client_cursor.y,
        target,
        GetCapture(),
        GetForegroundWindow(),
        GetActiveWindow(),
        GetFocus(),
        gui.hwndMoveSize,
        gui.hwndMenuOwner,
        IN_SIZE_MOVE.load(Ordering::Relaxed),
        INPUT_COUNT.load(Ordering::Relaxed),
        MOVE_COUNT.load(Ordering::Relaxed),
        BUTTON_COUNT.load(Ordering::Relaxed),
        KEY_COUNT.load(Ordering::Relaxed),
        LAST_MOUSE_X.load(Ordering::Relaxed),
        LAST_MOUSE_Y.load(Ordering::Relaxed),
        LAST_MESSAGE.load(Ordering::Relaxed),
        message_name(LAST_MESSAGE.load(Ordering::Relaxed)),
        async_space,
        async_escape,
    );

    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, COLORREF(0x00_F0_F0_F0));

    let text_w = wide(&text);
    let mut text_rect = RECT {
        left: 20,
        top: 145,
        right: client.right - 20,
        bottom: client.bottom - 20,
    };
    DrawTextW(
        hdc,
        PCWSTR(text_w.as_ptr()),
        -1,
        &mut text_rect,
        DT_LEFT | DT_TOP | DT_WORDBREAK,
    );

    let _ = EndPaint(hwnd, &ps);
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    log_message(hwnd, msg, wparam, lparam);

    match msg {
        WM_TIMER => {
            // Repaint independently of mouse/keyboard input.
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 == 0x1B => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() -> windows::core::Result<()> {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(*PCWSTR::null()));
        let class_name = w!("Win32CanvasInputTest");

        let wc = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW),
            hInstance: instance,
            lpszClassName: PCWSTR(class_name),
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            ..Default::default()
        };

        if RegisterClassW(&wc) == ATOM(0) {
            return Err(windows::core::Error::from_thread());
        }

        let hwnd = CreateWindowExW(
            0,
            PCWSTR(class_name),
            PCWSTR(w!("Native Win32 Canvas Input Test")),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            900,
            720,
            None,
            None,
            Some(instance),
            None,
        );

        APP_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

        // 16 ms repaint timer; continues even when hardware input delivery stops.
        SetTimer(Some(hwnd), 1, 16, None);

        ShowWindow(hwnd, SW_SHOW as i32);
        let _ = UpdateWindow(hwnd);

        println!("App HWND: {hwnd:?}");
        println!("1. Move and click inside the client area.");
        println!("2. Type keys.");
        println!("3. Drag the native title bar.");
        println!("4. Repeat movement, clicks, and typing.");
        println!("5. Minimize/restore and test again.");

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}
