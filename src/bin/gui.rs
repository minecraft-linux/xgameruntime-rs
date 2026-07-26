use core::slice;
use std::collections::HashMap;
use std::ffi::c_char;
use std::os::raw::c_void;
use std::ptr::null_mut;

use eframe::{CreationContext, WgpuConfiguration, egui};
use egui::{Id, Sense, TextureHandle};
use serde::{Deserialize, Serialize};
use wgpu::{Backends, InstanceFlags};
use windows::core::*;
use windows::{
    libloaderapi::GetModuleHandleW,
    minwindef::{LPARAM, LRESULT, WPARAM},
    processthreadsapi::GetCurrentThreadId,
    windef::{self, HWND},
    winuser::{
        CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, EnableWindow, GWLP_HWNDPARENT,
        GetActiveWindow, GetCapture, GetCursorPos, GetFocus, GetForegroundWindow, IsWindowEnabled,
        IsWindowVisible, PostQuitMessage, RegisterClassW, ReleaseCapture, SW_HIDE, SW_SHOW,
        ScreenToClient, SetActiveWindow, SetFocus, SetForegroundWindow, SetWindowLongPtrW,
        ShowWindow, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, WM_DESTROY, WNDCLASSW,
        WS_OVERLAPPEDWINDOW, WindowFromPoint,
    },
};
use winit::{platform::windows::EventLoopBuilderExtWindows, window::WindowId};
use windows::{minwindef::*, windef::*, wingdi::*, winuser::*, sysinfoapi::*};
use xgameruntime::{InitializeApiImplEx2, get_x_game_ui, xasync};
use xgameruntime::com::{IXUserPlatform, XUserGetTokenAndSignatureUtf16Data, XUserHandle, query_api_impl};
use xgameruntime::xasync::XAsyncBlock;

#[derive(Default)]
struct MyEguiApp {
    parent: HWND,
    cnt: i64,
    TextureHande: Option<TextureHandle>,
    users: HashMap<String, ProfileUser>,
}

impl MyEguiApp {
    fn new(cc: &CreationContext<'_>, mut users: HashMap<String, ProfileUser>, p: HWND) -> Self {
        let icon = eframe::icon_data::from_png_bytes(
            &include_bytes!("/Users/christopher/Documents/minecraft/xodus/assets/Icon/Icon512.png")[..],
        )
        .expect("Failed to load icon");

        egui_extras::install_image_loaders(&cc.egui_ctx);

        let image = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
        let texture = cc.egui_ctx.load_texture(
            "cover-image",
            image,
            egui::TextureOptions::LINEAR,
        );

        // for user in users.values_mut() {
        //     if !user.gamer_picture.is_empty() {
        //         let b = eframe::icon_data::from_png_bytes(
        //             &user.gamer_picture[..],
        //         ).unwrap();
        //         let image = egui::ColorImage::from_rgba_unmultiplied([b.width as usize, b.height as usize], &b.rgba);
        //         let texture = cc.egui_ctx.load_texture(
        //             format!("gamer-pic-{}", user.id),
        //             image,
        //             egui::TextureOptions::LINEAR,
        //         );
        //         user.texture = Some(texture);
        //     }
        // }

        Self { parent: p, cnt: 0, TextureHande: Some(texture), users }
    }
}

unsafe fn client_cursor(hwnd: HWND) -> Option<windef::POINT> {
    let mut point = windef::POINT::default();

    GetCursorPos(&mut point).ok();
    ScreenToClient(hwnd, &mut point).ok();

    Some(point)
}

unsafe fn dump_window_state(egui: HWND, owner: HWND) {
    println!("foreground = {:?}", GetForegroundWindow());
    println!("active     = {:?}", GetActiveWindow());
    println!("focus      = {:?}", GetFocus());

    println!(
        "egui:  enabled={} visible={}",
        IsWindowEnabled(egui).as_bool(),
        IsWindowVisible(egui).as_bool(),
    );

    println!(
        "owner: enabled={} visible={}",
        IsWindowEnabled(owner).as_bool(),
        IsWindowVisible(owner).as_bool(),
    );
}

static mut LAST_TARGET: HWND = HWND(null_mut());

unsafe fn debug_mouse_target(hwnd: HWND) {
    let mut pt = windef::POINT::default();

    GetCursorPos(&mut pt);
    let target = WindowFromPoint(pt);

    if target != LAST_TARGET {
        println!(
            "cursor=({}, {}) target={:?} app={:?} capture={:?} fg={:?} active={:?} focus={:?}",
            pt.x,
            pt.y,
            target,
            hwnd,
            GetCapture(),
            GetForegroundWindow(),
            GetActiveWindow(),
            GetFocus(),
        );

        LAST_TARGET = target;
    }
}

impl eframe::App for MyEguiApp {    
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        //frame.winit_window().unwrap().
        //    ui.viewport(|v|v.builder.visible = Some(true));
        //ui.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        // let id: u64 = frame.winit_window().unwrap().id().into();
        // let hwnd = HWND(id as *mut core::ffi::c_void);
        // // unsafe { ReleaseCapture() };
        // let mut track = TRACKMOUSEEVENT {
        //     cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        //     dwFlags: TME_LEAVE,
        //     hwndTrack: HWND(id as *mut core::ffi::c_void),
        //     dwHoverTime: 0,
        // };

        // unsafe {
        //     TrackMouseEvent(&mut track).ok();
        // }
        // unsafe {
        //     debug_mouse_target(hwnd);
        // }
        // unsafe {
        // ShowWindow(hwnd, SW_HIDE as i32);
        // ShowWindow(hwnd, SW_SHOW as i32);
        // }
        // unsafe {
        //     SetForegroundWindow(hwnd);
        //     SetActiveWindow(hwnd);
        //     SetFocus(Some(hwnd));
        // }
        // unsafe { SetWindowLongPtrW(HWND(id as *mut core::ffi::c_void), GWLP_HWNDPARENT, self.parent.0 as isize) };
        egui::CentralPanel::default().show(ui, |ui| {
            // ui.heading("Hello World!");
            egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // for i in 0..10 {
                //     ui.horizontal(|ui| {
                //         let (_, rect) = ui.allocate_space(egui::vec2(50.0, 50.0));
                //         let painter = ui.painter();
                //         painter.image(self.TextureHande.as_ref().unwrap().id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                //         ui.label(format!("Item {}", i));
                //     });
                // }
                for user in self.users.values_mut() {
                    ui.horizontal(|ui| {
                        let (_, rect) = ui.allocate_space(egui::vec2(50.0, 50.0));
                        let mut r2 = rect;
                        r2.max.x += ui.available_width();

                        let response = ui.interact(r2, Id::new(user.id.clone()), Sense::click());
                        if response.clicked() {
                            user.selected = !user.selected;
                        }
                        let painter = ui.painter();
                        painter.rect_filled(r2, 5.0, if user.selected { egui::Color32::LIGHT_BLUE } else { egui::Color32::LIGHT_GRAY });
                        // if let Some(texture) = &user.texture {
                        //     painter.image(texture.id(), rect.shrink(5.0), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                        // }
                        if let Some(base_url) = user.settings.get("GameDisplayPicRaw") {
                            // ui.image(format!("{}&w=128&h=128", base_url));
                            egui::Image::from_uri(format!("{}&w=128&h=128", base_url))
                                .fit_to_exact_size(rect.shrink(5.0).size())
                                .paint_at(ui, rect.shrink(5.0));
                        }
                        ui.label(format!("{} {}", user.settings.get("Gamertag").unwrap_or(&"Unknown".to_string()), user.presense));
                    });
                }
            });
            // ui.label(format!(
            //     "pointer: {:?}",
            //     //ui.input(|i| i.pointer.hover_pos())
            //     ui.input(|input| {
            //         for event in &input.events {
            //             println!("{event:?}");
            //         }
            //         println!("pointer pos: {:?}", input.pointer.hover_pos());
            //         println!("pressed: {}", input.pointer.any_pressed());
            //         println!("down: {}", input.pointer.any_down());
            //         println!("released: {}", input.pointer.any_released());
            //         println!("focused: {}", input.focused);
            //     })
            // ));
            // ui.label(format!("client_cursor: {:?}", unsafe {
            //     client_cursor(HWND(id as *mut core::ffi::c_void))
            // }));
            if ui.button(format!("Click Me {}", self.cnt)).clicked() {
                self.cnt = self.cnt + 1;
            }
        });
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
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
            let x = (lparam.0 as i16) as i32;
            let y = ((lparam.0 >> 16) as i16) as i32;
            println!(
                "{:<18} {x} {y}",
                message_name(msg),
            );
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_NCLBUTTONDOWN | WM_NCLBUTTONUP => {
            println!(
                "{:<18} hwnd={:?} wp=0x{:x} lp=0x{:x}",
                message_name(msg),
                hwnd,
                wparam.0,
                lparam.0
            );
        }
        WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP | WM_CHAR => {
            println!(
                "{:<18} hwnd={:?} wp=0x{:x} lp=0x{:x}",
                message_name(msg),
                hwnd,
                wparam.0,
                lparam.0
            );
        }
        WM_ENTERSIZEMOVE => {
            println!("WM_ENTERSIZEMOVE hwnd={hwnd:?}");
        }
        WM_EXITSIZEMOVE => {
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
}

#[derive(Serialize)]
struct UserProfileBatch<'t> {
    userIds: &'t [&'t str],
    settings: &'t [&'t str],
}

#[derive(Debug, Deserialize)]
struct UserProfileSettings {
    id: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct UserProfileEntry {
    id: String,
    hostId: String,
    settings: Vec<UserProfileSettings>,
}

#[derive(Debug, Deserialize)]
struct UserProfileBatchResponse {
    profileUsers: Vec<UserProfileEntry>,
}

#[derive(Debug, Deserialize)]
struct XblPresenceRecord {
    xuid: String,
    state: String,
}


struct ProfileUser {
    id: String,
    settings: std::collections::HashMap<String, String>,
    gamer_picture: Vec<u8>,
    texture: Option<TextureHandle>,
    selected: bool,
    presense: String,
}

fn main() /*-> eframe::Result<()>*/
{
    env_logger::init();
    
    let init_hr = InitializeApiImplEx2(2604, 100000, 10, std::ptr::null_mut());
    assert_eq!(init_hr, HRESULT(0));

    let mut out: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = query_api_impl(
        &GUID::from_u128(0x01acd177_91f9_4763_a38e_ccbb55ce32e0),
        &IXUserPlatform::IID,
        &mut out,
    );
    assert_eq!(hr, HRESULT(0));

    let user = unsafe { IXUserPlatform::from_raw_borrowed(&out).unwrap() };

    let mut async_block = XAsyncBlock {
        queue: std::ptr::null_mut(),
        context: std::ptr::null_mut(),
        callback: None,
        internal: [0; std::mem::size_of::<*mut c_void>() * 4],
    };

    unsafe { println!("{}", user.x_user_add_async(4, &mut async_block as *mut XAsyncBlock as *mut c_void)) };

    unsafe { println!("{}", xasync::get_status(&mut async_block as *mut XAsyncBlock, true).map(|_|HRESULT(0)).unwrap_or_else(|v|v)) };

    let mut user_out: *mut c_void = std::ptr::null_mut();
    unsafe {
        println!("{}", user.x_user_add_result(&mut async_block as *mut XAsyncBlock as *mut c_void, &mut user_out as *mut *mut c_void));

        println!(
            "user_out={:?}",
            user_out
        );
    }
    // let entry =  XUserGetTokenAndSignatureUtf16HttpHeader {
    //     name: w!("x-xbl-contract-version"),
    //     value: w!("1"),
    // };
    let mut async_block = XAsyncBlock {
        queue: std::ptr::null_mut(),
        context: std::ptr::null_mut(),
        callback: None,
        internal: [0; std::mem::size_of::<*mut c_void>() * 4],
    };
    println!("xuser_get_token_and_signature_utf16_async {}", unsafe { user.xuser_get_token_and_signature_utf16_async(user_out, 0, w!("POST").0, w!("https://profile.xboxlive.com/users/batch/profile/settings").0, 0, std::ptr::null(), 0, std::ptr::null(), &mut async_block as *mut XAsyncBlock as *mut c_void) });
    unsafe { println!("xasync::get_status {}", xasync::get_status(&mut async_block as *mut XAsyncBlock, true).map(|_|HRESULT(0)).unwrap_or_else(|v|v)) };
    let mut size: usize = 0;
    unsafe { println!("xuser_get_token_and_signature_utf16_result_size {}", user.xuser_get_token_and_signature_utf16_result_size(&mut async_block as *mut XAsyncBlock as *mut c_void, &mut size as *mut usize)) };

    println!("size={}", size);

    let mut buffer =    Vec::with_capacity(size);
    let mut ptr_to_buffer: *mut XUserGetTokenAndSignatureUtf16Data = std::ptr::null_mut();
    println!("xuser_get_token_and_signature_utf16_result {}", unsafe { user.xuser_get_token_and_signature_utf16_result(&mut async_block as *mut XAsyncBlock as *mut c_void, size, buffer.as_mut_ptr(), &mut ptr_to_buffer, std::ptr::null_mut()) });
    
    // println!("ptr_to_buffer={:?}", HSTRING::from(HSTR
    // let token = unsafe { String::from_raw_parts((*ptr_to_buffer).token, (*ptr_to_buffer).token_count, (*ptr_to_buffer).token_count) };
    let token = String::from_utf16_lossy(unsafe { slice::from_raw_parts((*ptr_to_buffer).token, (*ptr_to_buffer).token_count - 1) });
    println!("token={}", token);

    let xuser = user;

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let users = runtime.block_on(async {
        let client = reqwest::Client::new();
        let r = client
            .post("https://profile.xboxlive.com/users/batch/profile/settings")
            // .http1_only()
            .header("x-xbl-contract-version", "2")
            .header(
                "Authorization",
                token,
            )
            .json(&UserProfileBatch {
                userIds: &["2535418015510202"],
                settings: &[
                    "AppDisplayName",
                    "AppDisplayPicRaw",
                    "GameDisplayName",
                    "GameDisplayPicRaw",
                    "Gamerscore",
                    "Gamertag",
                    "ModernGamertag",
                    "ModernGamertagSuffix",
                    "UniqueModernGamertag",
                ],
            })
            .send()
            .await
            .unwrap()
            .json::<UserProfileBatchResponse>()
            .await
            .unwrap();

        println!("{:?}", r);

        let mut users = r.profileUsers
            .into_iter()
            .map(|user| {
                let mut settings_map = std::collections::HashMap::new();
                for setting in user.settings {
                    settings_map.insert(setting.id, setting.value);
                }
                (user.id.clone(), ProfileUser {
                    id: user.id,
                    settings: settings_map,
                    gamer_picture: Vec::new(),
                    texture: None,
                    selected: false,
                    presense: String::new(),
                })
            })
            .collect::<std::collections::HashMap<_, _>>();

        // for user in users.values_mut() {
        //     if let Some(url) = user.settings.get("GameDisplayPicRaw") {
        //         println!("Downloading gamer picture for user {} {}", user.id, url.to_owned() + "&w=64&h=64");
        //         let picture = 
        //             client
        //                 .get(url.to_owned() + "&w=64&h=64")
        //                 .send()
        //                 .await
        //                 .unwrap()
        //                 .bytes()
        //                 .await
        //                 .unwrap()
        //                 .to_vec();
        //         user.gamer_picture = picture;
        //     }
        // }
            for user in users.values_mut() {
            if let Some(url) = user.settings.get("GameDisplayPicRaw") {
                let mut async_block = XAsyncBlock {
                    queue: std::ptr::null_mut(),
                    context: std::ptr::null_mut(),
                    callback: None,
                    internal: [0; std::mem::size_of::<*mut c_void>() * 4],
                };
                println!("xuser_get_token_and_signature_utf16_async {}", unsafe { xuser.xuser_get_token_and_signature_utf16_async(user_out, 0, w!("GET").0, windows::core::HSTRING::from(format!(
                    "https://userpresence.xboxlive.com/users/xuid({})?level=all",
                    user.id))
                .as_ptr(), 0, std::ptr::null(), 0, std::ptr::null(), &mut async_block as *mut XAsyncBlock as *mut c_void) });
                unsafe { println!("xasync::get_status {}", xasync::get_status(&mut async_block as *mut XAsyncBlock, true).map(|_|HRESULT(0)).unwrap_or_else(|v|v)) };
                let mut size: usize = 0;
                unsafe { println!("xuser_get_token_and_signature_utf16_result_size {}", xuser.xuser_get_token_and_signature_utf16_result_size(&mut async_block as *mut XAsyncBlock as *mut c_void, &mut size as *mut usize)) };

                println!("size={}", size);

                let mut buffer =    Vec::with_capacity(size);
                let mut ptr_to_buffer: *mut XUserGetTokenAndSignatureUtf16Data = std::ptr::null_mut();
                println!("xuser_get_token_and_signature_utf16_result {}", unsafe { xuser.xuser_get_token_and_signature_utf16_result(&mut async_block as *mut XAsyncBlock as *mut c_void, size, buffer.as_mut_ptr(), &mut ptr_to_buffer, std::ptr::null_mut()) });
                
                // println!("ptr_to_buffer={:?}", HSTRING::from(HSTR
                // let token = unsafe { String::from_raw_parts((*ptr_to_buffer).token, (*ptr_to_buffer).token_count, (*ptr_to_buffer).token_count) };
                let token = String::from_utf16_lossy(unsafe { slice::from_raw_parts((*ptr_to_buffer).token, (*ptr_to_buffer).token_count - 1) });
                println!("token={}", token);

                println!("Downloading gamer picture for user {} {}", user.id, url.to_owned() + "&w=64&h=64");
                let content = 
                    client
                        .get(format!(
                            "https://userpresence.xboxlive.com/users/xuid({})?level=all",
                            user.id
                        ))
                        .header("x-xbl-contract-version", "2")
                        .header(
                            "Authorization",
                            token,
                        )
                        .send()
                        .await
                        .unwrap()
                        .json::<XblPresenceRecord>()
                        .await
                        .unwrap();
                user.presense = content.state;
            }
        }
        

        use std::time::Duration;

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .http1_only()
            .build()
            .unwrap();

        // for user in users.values_mut() {
        //     let Some(base_url) = user.settings.get("GameDisplayPicRaw") else {
        //         continue;
        //     };

        //     let url = format!("{base_url}&w=64&h=64");
        //     println!("Downloading gamer picture for {}: {url}", user.id);

        //     println!("before send");

        //     let response = client
        //         .get(&url)
        //         .send()
        //         .await
        //         .unwrap_or_else(|error| {
        //             panic!(
        //                 "image request failed: {error:?}, connect={}, timeout={}",
        //                 error.is_connect(),
        //                 error.is_timeout(),
        //             )
        //         });

        //     println!(
        //         "after send: status={}, final_url={}",
        //         response.status(),
        //         response.url()
        //     );

        //     let response = response
        //         .error_for_status()
        //         .unwrap_or_else(|error| panic!("image server returned an error: {error:?}"));

        //     println!("before body");

        //     let picture = response
        //         .bytes()
        //         .await
        //         .unwrap_or_else(|error| panic!("reading image body failed: {error:?}"))
        //         .to_vec();

        //     println!("downloaded {} bytes", picture.len());

        //     user.gamer_picture = picture;
        // }

        users
    });

    // let parent = unsafe {
    //     let instance = GetModuleHandleW(None);

    //     let class = w!("DummyParent");

    //     RegisterClassW(&WNDCLASSW {
    //         lpfnWndProc: Some(wndproc),
    //         hInstance: instance.into(),
    //         lpszClassName: class,
    //         ..Default::default()
    //     });

    //     let parent = CreateWindowExW(
    //         0,
    //         class,
    //         w!("Parent"),
    //         WS_OVERLAPPEDWINDOW,
    //         CW_USEDEFAULT,
    //         CW_USEDEFAULT,
    //         800,
    //         600,
    //         None,
    //         None,
    //         Some(instance.into()),
    //         None,
    //     );
    //     let _ = ShowWindow(parent, SW_SHOW as i32);
    //     EnableWindow(parent, false);

    //     parent
    // };

        let game_ui = get_x_game_ui();
        let mut async_ = xasync::XAsyncBlock {
            context: std::ptr::null_mut(),
            queue: std::ptr::null_mut(),
            callback: None,
            internal: [0; std::mem::size_of::<*mut c_void>() * 4],
        };

        println!("Showing player picker...");

        let hr = unsafe {
            game_ui.x_game_ui_show_player_picker_async(&mut async_, user_out as XUserHandle, c"Hello world".as_ptr() as *const c_char, 0, null_mut(), 0, null_mut(), 1, 1)
        };
        if hr.is_err() {
            println!("Failed to show player picker: {:?}", hr);
        } else {
            println!("waiting player picker...");
            unsafe { xasync::get_status(&mut async_, true) };

            let mut result_players_count: u32 = 1;
            let mut result_players: [u64; 10] = [0; 10];
            let mut result_players_used: u32 = 0;

            let hr = unsafe {
                game_ui.x_game_ui_show_player_picker_result(&mut async_, result_players_count, result_players.as_mut_ptr(), &mut result_players_used)
            };
            if hr.is_err() {
                println!("Failed to get player picker result: {:?}", hr);
            } else {
                println!("Player picker result: count={}, used={}, players={:?}", result_players_count, result_players_used, &result_players[..result_players_used as usize]);
            }
        }


    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,

        wgpu_options: WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
                eframe::egui_wgpu::WgpuSetupCreateNew {
                    instance_descriptor: wgpu::InstanceDescriptor {
                        backends: Backends::DX12,
                        backend_options: wgpu::BackendOptions {
                            dx12: wgpu::Dx12BackendOptions {
                                shader_compiler: wgpu::Dx12Compiler::DynamicDxc {
                                    dxc_path: "dxcompiler.dll".to_owned(),
                                },
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        display: None,
                        flags: InstanceFlags::from_env_or_default(),
                        memory_budget_thresholds: wgpu::MemoryBudgetThresholds {
                            for_resource_creation: None,
                            for_device_loss: None,
                        },
                    },
                    display_handle: None,
                    power_preference: wgpu::PowerPreference::None,
                    native_adapter_selector: None,
                    ..eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle()
                },
            ),
            ..Default::default()
        },

        viewport: egui::ViewportBuilder::default()
            .with_inner_size([420.0, 180.0])
            .with_resizable(true),

        // window_builder: Some(Box::new(move |attributes| {
        //     return attributes.with_visible(false);
        // })),
        event_loop_builder: Some(Box::new(|b| {
            b.with_any_thread(true)
            .with_msg_hook(|raw_msg: *const c_void| {
            let msg = unsafe { &*(raw_msg as *const windows::winuser::MSG) };

            // match msg.message {
            //     WM_MOUSEMOVE => {
            //         println!("HOOK WM_MOUSEMOVE hwnd={:?}", msg.hwnd);
            //     }
            //     WM_NCMOUSEMOVE => {
            //         println!("HOOK WM_NCMOUSEMOVE hwnd={:?}", msg.hwnd);
            //     }
            //     WM_ENTERSIZEMOVE => {
            //         println!("HOOK WM_ENTERSIZEMOVE hwnd={:?}", msg.hwnd);
            //     }
            //     WM_EXITSIZEMOVE => {
            //         println!("HOOK WM_EXITSIZEMOVE hwnd={:?}", msg.hwnd);
            //     }
            //     WM_CAPTURECHANGED => {
            //         println!(
            //             "HOOK WM_CAPTURECHANGED hwnd={:?} new={:?}",
            //             msg.hwnd,
            //             msg.lParam
            //         );
            //     }
            //     WM_CANCELMODE => {
            //         println!("HOOK WM_CANCELMODE hwnd={:?}", msg.hwnd);
            //     }
            //     _ => {}
            // }

            log_message(msg.hwnd, msg.message, msg.wParam, msg.lParam);
            // Important: false means let winit dispatch it normally.
            false
        });
        })),

        ..Default::default()
    };
    // let options = eframe::NativeOptions {
    //     renderer: eframe::Renderer::Wgpu,

    //     wgpu_options: WgpuConfiguration {
    //         wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
    //             eframe::egui_wgpu::WgpuSetupCreateNew { instance_descriptor: wgpu::InstanceDescriptor { backends: Backends::VULKAN, backend_options: wgpu::BackendOptions {
    //                 ..Default::default()
    //             },
    //             display: None,
    //             flags: InstanceFlags::empty(),
    //         memory_budget_thresholds: wgpu::MemoryBudgetThresholds { for_resource_creation: None, for_device_loss: None } },
    //                 display_handle: None,
    //                 power_preference: wgpu::PowerPreference::None,
    //                 native_adapter_selector: None,
    //                 ..eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle() }
    //         ),
    //         ..Default::default()
    //     },

    //     viewport: egui::ViewportBuilder::default()
    //         .with_inner_size([420.0, 180.0])
    //         .with_resizable(true),

    //     window_builder: Some(Box::new(move |attributes| {
    //         return attributes.with_visible(false);
    //     })),
    //     event_loop_builder: Some(Box::new(|b|{
    //         b.with_any_thread(true);
    //     })),

    //     ..Default::default()
    // };

    eframe::run_native(
        "Confirm action",
        options,
        Box::new(|cc| Ok(Box::new(MyEguiApp::new(cc, users, HWND(null_mut()))))),
    )
    .unwrap();

    // // unsafe {
    // //     std::env::set_var("WGPU_BACKEND", "dx12");
    // // }

    // // println!(
    // //     "main thread: {:?}, name: {:?}",
    // //     std::thread::current().id(),
    // //     std::thread::current().name(),
    // // );

    // // eframe::run_native(
    // //     "egui test",
    // //     eframe::NativeOptions {
    // //         renderer: eframe::Renderer::Wgpu,
    // //         wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
    // //             // supported_backends: eframe::wgpu::Backends::DX12,
    // //             ..Default::default()
    // //         },
    // //         viewport: egui::ViewportBuilder::default()
    // //             .with_inner_size([400.0, 200.0]),
    // //         ..Default::default()
    // //     },
    // //     Box::new(|_| Ok(Box::new(TestApp))),
    // // )
    // println!(
    //     "Rust thread: {:?}, name: {:?}",
    //     std::thread::current().id(),
    //     std::thread::current().name(),
    // );

    // unsafe {
    //     println!("Win32 thread ID: {}", GetCurrentThreadId());
    // }

    // winit::event_loop::EventLoop::new().unwrap();
}