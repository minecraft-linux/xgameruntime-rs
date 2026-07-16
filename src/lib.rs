use std::ffi::{CString, c_char, c_void};
use std::result::Result;
use std::sync::Mutex;

use windows_core::{GUID, HRESULT, Interface};
use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

use crate::com::{IXUserPlatform, XUserPlatformRemoteConnectEventHandlers};

mod com;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsixvcFileEntry {
    pub path: String,
    pub offset: u64,
    pub length: u64,
    pub encrypted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsixvcInfo {
    pub content_id: String,
    pub files: Vec<MsixvcFileEntry>,
}

type Hinstance = *mut c_void;
type Bool = i32;
type Dword = u32;
type Ulong = u32;
type Char = i8;
type Lpcstr = *const c_char;

const TRUE: Bool = 1;
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

unsafe extern "system" fn show() {
    todo!("show");
}

unsafe extern "system" fn hide() {
    todo!("hide");
}

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
