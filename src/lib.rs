use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr::null_mut;
use std::result::Result;
use std::sync::{Arc, Mutex};
use eframe::WgpuConfiguration;
use wgpu::{Backends, InstanceFlags};
use windows_sys::w;
use winit::platform::windows::EventLoopBuilderExtWindows;

use windows::minwindef::LPARAM;
use windows::windef::{HMENU, HWND};
use windows::winuser::{AppendMenuW, CreateMenu, DrawMenuBar, EnumWindows, GWLP_WNDPROC, MB_OK, MF_STRING, MessageBoxW, SetMenu};

use crate::com::{IXUserPlatform, XUserPlatformRemoteConnectEventHandlers};
use windows_core::{GUID, HRESULT, Interface, PCWSTR};
use windows_sys::libloaderapi::{FreeLibrary, GetProcAddress, LoadLibraryA};
use windows_sys::minwindef::HMODULE;

pub mod com;
pub mod results;
pub mod xasync;

type Ulong = u32;
type Char = i8;
type Lpcstr = *const c_char;

const S_OK: HRESULT = HRESULT(0);
const E_FAIL: HRESULT = HRESULT(0x80004005u32 as i32);
const E_NOTIMPL: HRESULT = HRESULT(0x80004001u32 as i32);

#[repr(C)]
pub struct InitializeOptions;

type InitializeApiImplEx2Fn =
    unsafe extern "system" fn(Ulong, Ulong, Char, *mut InitializeOptions) -> HRESULT;
type QueryApiImplFn =
    unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;
type UninitializeApiImplFn = unsafe extern "system" fn() -> HRESULT;

struct DelegatedApi {
    module: HMODULE,
    initialize_api_impl_ex2: InitializeApiImplEx2Fn,
    query_api_impl: QueryApiImplFn,
    uninitialize_api_impl: UninitializeApiImplFn,
}

unsafe impl Send for DelegatedApi {}

struct DelegatedApiState {
    ref_count: usize,
    api: Option<DelegatedApi>,
}

static DELEGATED_API_STATE: Mutex<DelegatedApiState> = Mutex::new(DelegatedApiState {
    ref_count: 0,
    api: None,
});

#[cfg(test)]
static TEST_DELEGATED_DLL_PATH: Mutex<Option<CString>> = Mutex::new(None);

fn delegated_state() -> std::sync::MutexGuard<'static, DelegatedApiState> {
    DELEGATED_API_STATE
        .lock()
        .expect("delegated xgameruntime state poisoned")
}

#[cfg(test)]
fn delegated_dll_name() -> CString {
    TEST_DELEGATED_DLL_PATH
        .lock()
        .expect("delegated xgameruntime test path poisoned")
        .clone()
        .unwrap_or_else(|| CString::new("xgameruntime.gdk.dll").expect("static dll name"))
}

#[cfg(not(test))]
fn delegated_dll_name() -> CString {
    CString::new("xgameruntime.gdk.dll").expect("static dll name")
}

#[cfg(test)]
pub(crate) fn set_delegated_dll_path_for_test(path: Option<&str>) {
    let mut slot = TEST_DELEGATED_DLL_PATH
        .lock()
        .expect("delegated xgameruntime test path poisoned");
    *slot = path.map(|path| CString::new(path).expect("dll path contains interior NUL"));
}

unsafe fn load_symbol<T>(module: HMODULE, symbol: &'static [u8]) -> Result<T, HRESULT>
where
    T: Copy,
{
    let proc = unsafe { GetProcAddress(module, symbol.as_ptr()) };
    if let Some(proc) = proc {
        Ok(unsafe { std::mem::transmute_copy(&proc) })
    } else {
        Err(E_FAIL)
    }
}

unsafe extern "system" fn find_window(hwnd: HWND, lp: LPARAM) -> windows_core::BOOL {
    unsafe {
        let result: &mut HWND = &mut *(lp.0 as *mut HWND);
        *result = hwnd;
    }
    return false.into();
}

const IDM_OPEN: usize = 1001;
const IDM_EXIT: usize = 1002;

pub fn create_window_menu(hwnd: HWND) -> windows::core::Result<HMENU> {
    unsafe {
        let menu = CreateMenu();

        AppendMenuW(menu, MF_STRING, IDM_OPEN, PCWSTR::from_raw(w!("My Open")));
        AppendMenuW(menu, MF_STRING, IDM_EXIT, PCWSTR::from_raw(w!("My Exit")));

        SetMenu(hwnd, Some(menu));
        DrawMenuBar(hwnd);

        Ok(menu)
    }
}

unsafe extern "system" fn show(
    _context: *const c_void,
    _user_identifierr: u32,
    _operation: u32,
    url: *const c_char,
    code: *const c_char,
    _qr_code_size: usize,
    _qr_code: *const c_char,
) {
    unsafe {
        let url = CStr::from_ptr(url);
        let code = CStr::from_ptr(code);
        let mut search: HWND = HWND(null_mut());
        _ = EnumWindows(
            Some(find_window),
            LPARAM((&mut search as *mut HWND) as isize),
        );
        if !search.0.is_null() {
            //SetWindowlongPtrW(search, GWLP_WNDPROC, 0);
            create_window_menu(search).unwrap();
        }
        MessageBoxW(
            if search.0.is_null() {
                None
            } else {
                Some(search)
            },
            windows_strings::PCWSTR::from_raw(
                windows::core::HSTRING::from(format!(
                    "{} {}",
                    url.to_string_lossy(),
                    code.to_string_lossy()
                ))
                .as_ptr(),
            ),
            windows::core::h!("Xbox Live Remote Login"),
            MB_OK,
        );
    }
}

unsafe extern "system" fn hide() {}

unsafe fn load_delegated_api() -> Result<DelegatedApi, HRESULT> {
    let dll_name = delegated_dll_name();
    let module = unsafe { LoadLibraryA(dll_name.as_ptr().cast()) };
    if module.is_null() {
        return Err(E_FAIL);
    }

    let initialize_api_impl_ex2 =
        match unsafe { load_symbol::<InitializeApiImplEx2Fn>(module, b"InitializeApiImplEx2\0") } {
            Ok(symbol) => symbol,
            Err(error) => {
                unsafe {
                    FreeLibrary(module);
                }
                return Err(error);
            }
        };
    let query_api_impl = match unsafe { load_symbol::<QueryApiImplFn>(module, b"QueryApiImpl\0") } {
        Ok(symbol) => symbol,
        Err(error) => {
            unsafe {
                FreeLibrary(module);
            }
            return Err(error);
        }
    };
    let uninitialize_api_impl =
        match unsafe { load_symbol::<UninitializeApiImplFn>(module, b"UninitializeApiImpl\0") } {
            Ok(symbol) => symbol,
            Err(error) => {
                unsafe {
                    FreeLibrary(module);
                }
                return Err(error);
            }
        };
    Ok(DelegatedApi {
        module,
        initialize_api_impl_ex2,
        query_api_impl,
        uninitialize_api_impl,
    })
}

#[derive(Default)]
struct MyEguiApp {}

impl MyEguiApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_global_style.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self::default()
    }
}

impl eframe::App for MyEguiApp {
   fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        //frame.winit_window().unwrap().
        //    ui.viewport(|v|v.builder.visible = Some(true));
        ui.send_viewport_cmd(
            egui::ViewportCommand::Visible(true),
        );
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Hello World!");
        });
   }
}

fn initialize_delegate(
    gdk_ver: Ulong,
    gs_ver: Ulong,
    mode: Char,
    options: *mut InitializeOptions,
) -> HRESULT {
    let mut state = delegated_state();
    if state.ref_count > 0 {
        state.ref_count += 1;
        return S_OK;
    }

    let api = match unsafe { load_delegated_api() } {
        Ok(api) => api,
        Err(error) => return error,
    };

    let hr = unsafe {
        (api.initialize_api_impl_ex2)(gdk_ver, gs_ver, mode | 8 /* xplat mode */, options)
    };
    if hr != S_OK {
        unsafe {
            FreeLibrary(api.module);
        }
        return hr;
    }

    let mut out: *mut c_void = std::ptr::null_mut();

    let xuserguid = GUID::from_u128(0x01acd177_91f9_4763_a38e_ccbb55ce32e0);

    let hr = unsafe { (api.query_api_impl)(&xuserguid, &IXUserPlatform::IID, &mut out) };

    assert_eq!(hr, HRESULT(0));
    assert!(!out.is_null());

    if let Some(platform) = unsafe { IXUserPlatform::from_raw_borrowed(&out) } {
        let callback: XUserPlatformRemoteConnectEventHandlers =
            XUserPlatformRemoteConnectEventHandlers {
                show: Some(show),
                close: Some(hide),
                context: std::ptr::null_mut(),
            };
        let hr = unsafe {
            platform.XUserPlatformRemoteConnectSetEventHandlers(std::ptr::null_mut(), &callback)
        };
        assert_eq!(hr, HRESULT(0));
    }

    // let options = eframe::NativeOptions {
    //     viewport: egui::ViewportBuilder::default()
    //         .with_inner_size([420.0, 180.0])
    //         .with_resizable(false),

    //     window_builder: Some(Box::new(move |attributes| {
    //         return attributes.with_visible(false)
    //     })),

    //     ..Default::default()
    // };

    // eframe::run_native(
    //     "Confirm action",
    //     options,
    //     Box::new(|_cc| Ok(Box::new(MyEguiApp::default()))),
    // ).unwrap();

    state.ref_count = 1;
    state.api = Some(api);
    S_OK
}

pub(crate) fn delegated_query_api_impl(
    runtime_class_id: *const GUID,
    interface_id: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let state = delegated_state();
    let Some(api) = state.api.as_ref() else {
        unsafe {
            *out = std::ptr::null_mut();
        }
        return E_NOTIMPL;
    };

    unsafe { (api.query_api_impl)(runtime_class_id, interface_id, out) }
}

fn uninitialize_delegate() -> HRESULT {
    let mut state = delegated_state();
    if state.ref_count == 0 {
        return E_NOTIMPL;
    }

    state.ref_count -= 1;
    if state.ref_count > 0 {
        return S_OK;
    }

    let Some(api) = state.api.take() else {
        return E_FAIL;
    };

    let hr = unsafe { (api.uninitialize_api_impl)() };
    unsafe {
        FreeLibrary(api.module);
    }
    hr
}

#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_OK
}

#[unsafe(no_mangle)]
pub extern "system" fn InitializeApiImplEx2(
    gdk_ver: Ulong,
    gs_ver: Ulong,
    mode: Char,
    options: *mut InitializeOptions,
) -> HRESULT {
    initialize_delegate(gdk_ver, gs_ver, mode, options)
}

#[unsafe(no_mangle)]
pub extern "system" fn InitializeApiImplEx(gdk_ver: Ulong, gs_ver: Ulong, mode: Char) -> HRESULT {
    InitializeApiImplEx2(gdk_ver, gs_ver, mode, std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn InitializeApiImpl(gdk_ver: Ulong, gs_ver: Ulong) -> HRESULT {
    InitializeApiImplEx(gdk_ver, gs_ver, 0)
}

#[unsafe(no_mangle)]
pub extern "system" fn QueryApiImpl(
    runtime_class_id: *const GUID,
    interface_id: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    com::query_api_impl(runtime_class_id, interface_id, out)
}

#[unsafe(no_mangle)]
pub extern "system" fn UninitializeApiImpl() -> HRESULT {
    uninitialize_delegate()
}

#[unsafe(no_mangle)]
pub extern "system" fn XErrorReport(_status: HRESULT, _message: Lpcstr) -> HRESULT {
    E_NOTIMPL
}

#[test]
fn test() {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,

        wgpu_options: WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
                eframe::egui_wgpu::WgpuSetupCreateNew { instance_descriptor: wgpu::InstanceDescriptor { backends: Backends::DX12, backend_options: wgpu::BackendOptions {
                    dx12: wgpu::Dx12BackendOptions {
                        shader_compiler:
                            wgpu::Dx12Compiler::DynamicDxc { dxc_path: "Z:/Users/christopher/Documents/minecraft/out2/dxcompiler.dll".to_owned() },
                        ..Default::default()
                    },
                    ..Default::default()
                },
                display: None,
                flags: InstanceFlags::from_env_or_default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds { for_resource_creation: None, for_device_loss: None } },
                    display_handle: None,
                    power_preference: wgpu::PowerPreference::None,
                    native_adapter_selector: None,
                    ..eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle() }
            ),
            ..Default::default()
        },

        viewport: egui::ViewportBuilder::default()
            .with_inner_size([420.0, 180.0])
            .with_resizable(false),

        window_builder: Some(Box::new(move |attributes| {
            return attributes.with_visible(false);
        })),
        event_loop_builder: Some(Box::new(|b|{
            b.with_any_thread(true);
        })),

        ..Default::default()
    };

    eframe::run_native(
        "Confirm action",
        options,
        Box::new(|_cc| Ok(Box::new(MyEguiApp::default()))),
    ).unwrap();
}