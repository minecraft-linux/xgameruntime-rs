use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    platform::windows::EventLoopBuilderExtWindows,
    window::Window,
};

use std::ffi::c_void;
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

fn main() {
    let mut builder = EventLoop::builder();

    builder.with_any_thread(true);

    builder.with_msg_hook(|msg| {
        let msg = unsafe { &*(msg as *const c_void as *const MSG) };

        match msg.message {
            WM_MOUSEMOVE =>
                println!("WM_MOUSEMOVE hwnd={:?}", msg.hwnd),
            WM_NCMOUSEMOVE =>
                println!("WM_NCMOUSEMOVE hwnd={:?}", msg.hwnd),
            _ => {}
        }

        false
    });

    let event_loop = builder.build().unwrap();

    let window = event_loop.create_window(
        Window::default_attributes()
            .with_title("winit mouse test")
    ).unwrap();

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CursorMoved { position, .. } => {
                    println!("CursorMoved {:?}", position);
                }

                WindowEvent::CloseRequested => {
                    elwt.exit();
                }

                _ => {}
            },

            Event::AboutToWait => {
                window.request_redraw();
            }

            _ => {}
        }
    }).unwrap();
}