use std::ffi::{CStr, CString, c_char, c_void};
use std::mem::ManuallyDrop;
use std::ptr::null_mut;
use std::result::Result;
use std::sync::{Arc, Mutex, OnceLock};
use windows::Storage::ApplicationData;
use windows::Storage::Pickers::IFileOpenPicker_Vtbl;
use windows::Storage::Pickers::IFileOpenPicker2;
use windows::Storage::Pickers::{IFileOpenPicker, PickerLocationId, PickerViewMode};
use windows::activation::{IActivationFactory, IActivationFactory_Impl};

use windows::Storage::IStorageFile;
use windows::Storage::IStorageFile_Impl;
use windows::Storage::IStorageItem;
use windows::Storage::IStorageItem_Impl;
use windows::Storage::IStorageItemProperties;
use windows::Storage::IStorageItemProperties_Impl;
use windows::Storage::Pickers::FileOpenPicker;
use windows::Storage::Streams::IRandomAccessStreamReference_Impl;
use windows::Storage::Streams::{IInputStreamReference_Impl, IRandomAccessStreamReference};
use windows::objbase::CoInitialize;
use windows::shobjidl_core::{IInitializeWithWindow, IInitializeWithWindow_Impl};
use windows_collections::IVector;
use windows_future::IAsyncOperation;
use windows_sys::w;

use windows::minwindef::{LPARAM, LRESULT, WPARAM};
use windows::windef::{HMENU, HWND};
use windows::winuser::{
    AppendMenuW, CallWindowProcW, CreateMenu, DefWindowProcW, DrawMenuBar, EnumWindows,
    GWLP_WNDPROC, GetWindowLongPtrW, MB_OK, MF_STRING, MessageBoxW, SetMenu, SetWindowLongPtrW,
    WNDPROC,
};

use crate::com::{IXUserPlatform, XUserPlatformRemoteConnectEventHandlers};
use windows_core::{GUID, HRESULT, HSTRING, IInspectable, IUnknown, Interface, PCWSTR, implement};
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

    unsafe { CoInitialize(None) }.unwrap();

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

const CLASS_E_CLASSNOTAVAILABLE: HRESULT = HRESULT(0x80040111u32 as i32);

// struct FileOpenPickerRuntimeName;

// impl windows_core::RuntimeName for FileOpenPickerRuntimeName {
//     const NAME: &'static str =
//         "Windows.Storage.Pickers.FileOpenPicker";
// }

// windows_core::imp::define_interface!(IStorageFileImpl, IStorageFileImpl_Vtbl, 0x00000035_0000_0000_c000_000000000046);
// windows_core::imp::interface_hierarchy!(IStorageFileImpl, windows_core::IUnknown, windows_core::IInspectable, IStorageFile);
// impl IStorageFileImpl{}
// #[repr(C)]
// #[doc(hidden)]
// pub struct IStorageFileImpl_Vtbl {
//     pub base__: IStorageFile_Vtbl,
// }
// pub trait IStorageFileImpl_Impl: windows_core::IUnknownImpl {
//     unsafe fn ViewMode(&self, view_mode: *mut PickerViewMode) -> HRESULT;
//     unsafe fn SetViewMode(&self, view_mode: PickerViewMode) -> HRESULT;
//     unsafe fn SettingsIdentifier(&self, settings_identifier: *mut *mut c_void) -> HRESULT;
//     unsafe fn SetSettingsIdentifier(&self, settings_identifier: *mut c_void) -> HRESULT;
//     unsafe fn PickSingleFileAsync(&self, arg2: *mut *mut core::ffi::c_void) -> windows_core::HRESULT;
//     unsafe fn PickMultipleFilesAsync(&self, arg2: *mut *mut core::ffi::c_void) -> windows_core::HRESULT;
// }
// impl IStorageFileImpl_Vtbl {
//     pub const fn new<Identity: IStorageFileImpl_Impl, const OFFSET: isize>() -> Self {
//         Self { base__: IStorageFile_Vtbl {
//             base__: windows_core::IInspectable_Vtbl::new::<Identity, IStorageFileImpl, OFFSET>(),
//             ViewMode: ViewMode::<Identity, OFFSET>,
//             SetViewMode: SetViewMode::<Identity, OFFSET>,
//             SettingsIdentifier: SettingsIdentifier::<Identity, OFFSET>,
//             SetSettingsIdentifier: SetSettingsIdentifier::<Identity, OFFSET>,
//             SuggestedStartLocation: SuggestedStartLocation::<Identity, OFFSET>,
//             SetSuggestedStartLocation: SetSuggestedStartLocation::<Identity, OFFSET>,
//             CommitButtonText: CommitButtonText::<Identity, OFFSET>,
//             SetCommitButtonText: SetCommitButtonText::<Identity, OFFSET>,
//             FileTypeFilter: FileTypeFilter::<Identity, OFFSET>,
//             PickSingleFileAsync: PickSingleFileAsync::<Identity, OFFSET>,
//             PickMultipleFilesAsync: PickMultipleFilesAsync::<Identity, OFFSET>,
//         } }
//     }
//     // pub const fn new<Identity: IActivationFactory_Impl, const OFFSET: isize>() -> Self {
//     //     unsafe extern "system" fn ActivateInstance<Identity: IActivationFactory_Impl, const OFFSET: isize>(this: *mut core::ffi::c_void, instance: *mut *mut core::ffi::c_void) -> windows_core::HRESULT {
//     //         unsafe {
//     //             let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
//     //             match IActivationFactory_Impl::ActivateInstance(this) {
//     //                 Ok(ok__) => {
//     //                     instance.write(core::mem::transmute(ok__));
//     //                     windows_core::HRESULT(0)
//     //                 }
//     //                 Err(err) => err.into(),
//     //             }
//     //         }
//     //     }
//     //     Self { base__: windows_core::IInspectable_Vtbl::new::<Identity, IActivationFactory, OFFSET>(), ActivateInstance: ActivateInstance::<Identity, OFFSET> }
//     // }

//     pub fn matches(iid: &windows_core::GUID) -> bool {
//         println!("IStorageFileImpl::matches called with iid: {:?}", iid);
//         iid == &<IStorageFile as windows_core::Interface>::IID
//     }
// }
// impl windows_core::RuntimeName for IStorageFileImpl {}

windows_core::imp::define_interface!(
    IFileOpenPickerImpl,
    IFileOpenPickerImpl_Vtbl,
    0x00000035_0000_0000_c000_000000000046
);
windows_core::imp::interface_hierarchy!(
    IFileOpenPickerImpl,
    windows_core::IUnknown,
    windows_core::IInspectable,
    IFileOpenPicker
);
impl IFileOpenPickerImpl {}
#[repr(C)]
#[doc(hidden)]
pub struct IFileOpenPickerImpl_Vtbl {
    pub base__: IFileOpenPicker_Vtbl,
}
pub trait IFileOpenPickerImpl_Impl: windows_core::IUnknownImpl {
    unsafe fn ViewMode(&self, view_mode: *mut PickerViewMode) -> HRESULT;
    unsafe fn SetViewMode(&self, view_mode: PickerViewMode) -> HRESULT;
    unsafe fn SettingsIdentifier(&self, settings_identifier: *mut *mut c_void) -> HRESULT;
    unsafe fn SetSettingsIdentifier(&self, settings_identifier: *mut c_void) -> HRESULT;
    unsafe fn PickSingleFileAsync(
        &self,
        arg2: *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT;
    unsafe fn PickMultipleFilesAsync(
        &self,
        arg2: *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT;
}
impl IFileOpenPickerImpl_Vtbl {
    pub const fn new<Identity: IFileOpenPickerImpl_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn ViewMode<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            view_mode: *mut PickerViewMode,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                Identity::ViewMode(this, view_mode)
            }
        }
        unsafe extern "system" fn SetViewMode<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            view_mode: PickerViewMode,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                Identity::SetViewMode(this, view_mode)
            }
        }
        unsafe extern "system" fn SettingsIdentifier<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            settings_identifier: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                Identity::SettingsIdentifier(this, settings_identifier)
            }
        }
        unsafe extern "system" fn SetSettingsIdentifier<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            settings_identifier: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                Identity::SetSettingsIdentifier(this, settings_identifier)
            }
        }
        unsafe extern "system" fn CommitButtonText<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            commit_button_text: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            // unsafe {
            //     let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
            //     Identity::CommitButtonText(this, commit_button_text)
            // }
            println!("CommitButtonText not implemented");
            S_OK
        }
        unsafe extern "system" fn SuggestedStartLocation<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            suggested_start_location: *mut PickerLocationId,
        ) -> windows_core::HRESULT {
            // unsafe {
            //     let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
            //     Identity::SuggestedStartLocation(this, suggested_start_location)
            // }
            println!("SuggestedStartLocation not implemented");
            S_OK
        }
        unsafe extern "system" fn SetSuggestedStartLocation<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            suggested_start_location: PickerLocationId,
        ) -> windows_core::HRESULT {
            // unsafe {
            //     let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
            //     Identity::SetSuggestedStartLocation(this, suggested_start_location)
            // }
            println!("SetSuggestedStartLocation not implemented");
            S_OK
        }
        unsafe extern "system" fn SetCommitButtonText<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            commit_button_text: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            // unsafe {
            //     let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
            //     Identity::SetCommitButtonText(this, commit_button_text)
            // }
            println!("SetCommitButtonText not implemented");
            S_OK
        }
        unsafe extern "system" fn FileTypeFilter<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            file_type_filter: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            // unsafe {
            //     let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
            //     Identity::FileTypeFilter(this, file_type_filter)
            // }
            println!("FileTypeFilter not implemented");
            let vec = IVector::<HSTRING>::from(vec![]);
            unsafe { *file_type_filter = core::mem::transmute(vec) };
            S_OK
        }
        unsafe extern "system" fn PickSingleFileAsync<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            arg2: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                Identity::PickSingleFileAsync(this, arg2)
            }
        }
        unsafe extern "system" fn PickMultipleFilesAsync<
            Identity: IFileOpenPickerImpl_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            arg2: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                Identity::PickMultipleFilesAsync(this, arg2)
            }
        }
        Self {
            base__: IFileOpenPicker_Vtbl {
                base__: windows_core::IInspectable_Vtbl::new::<Identity, IFileOpenPickerImpl, OFFSET>(
                ),
                ViewMode: ViewMode::<Identity, OFFSET>,
                SetViewMode: SetViewMode::<Identity, OFFSET>,
                SettingsIdentifier: SettingsIdentifier::<Identity, OFFSET>,
                SetSettingsIdentifier: SetSettingsIdentifier::<Identity, OFFSET>,
                SuggestedStartLocation: SuggestedStartLocation::<Identity, OFFSET>,
                SetSuggestedStartLocation: SetSuggestedStartLocation::<Identity, OFFSET>,
                CommitButtonText: CommitButtonText::<Identity, OFFSET>,
                SetCommitButtonText: SetCommitButtonText::<Identity, OFFSET>,
                FileTypeFilter: FileTypeFilter::<Identity, OFFSET>,
                PickSingleFileAsync: PickSingleFileAsync::<Identity, OFFSET>,
                PickMultipleFilesAsync: PickMultipleFilesAsync::<Identity, OFFSET>,
            },
        }
    }
    // pub const fn new<Identity: IActivationFactory_Impl, const OFFSET: isize>() -> Self {
    //     unsafe extern "system" fn ActivateInstance<Identity: IActivationFactory_Impl, const OFFSET: isize>(this: *mut core::ffi::c_void, instance: *mut *mut core::ffi::c_void) -> windows_core::HRESULT {
    //         unsafe {
    //             let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
    //             match IActivationFactory_Impl::ActivateInstance(this) {
    //                 Ok(ok__) => {
    //                     instance.write(core::mem::transmute(ok__));
    //                     windows_core::HRESULT(0)
    //                 }
    //                 Err(err) => err.into(),
    //             }
    //         }
    //     }
    //     Self { base__: windows_core::IInspectable_Vtbl::new::<Identity, IActivationFactory, OFFSET>(), ActivateInstance: ActivateInstance::<Identity, OFFSET> }
    // }

    pub fn matches(iid: &windows_core::GUID) -> bool {
        println!("IFileOpenPickerImpl::matches called with iid: {:?}", iid);
        iid == &<IFileOpenPicker as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IFileOpenPickerImpl {}

// #[implement(IFileOpenPicker, IFileOpenPicker2)]
#[implement(IFileOpenPickerImpl, IInitializeWithWindow)]
struct MyFileOpenPicker;
// impl windows::Storage::Pickers::IFileOpenPicker_Impl for MyFileOpenPicker_Impl {

// }

impl IInitializeWithWindow_Impl for MyFileOpenPicker_Impl {
    fn Initialize(&self, hwnd: windows::windef::HWND) -> windows_core::Result<()> {
        println!(
            "IInitializeWithWindow::Initialize called with hwnd: {:?}",
            hwnd
        );
        Ok(())
    }
}

impl IFileOpenPickerImpl_Impl for MyFileOpenPicker_Impl {
    unsafe fn ViewMode(&self, view_mode: *mut PickerViewMode) -> HRESULT {
        // todo!()
        println!("ViewMode called, returning PickerViewMode::List");
        unsafe {
            *view_mode = PickerViewMode::List;
        }
        S_OK
    }

    unsafe fn SetViewMode(&self, view_mode: PickerViewMode) -> HRESULT {
        // todo!()
        println!("SetViewMode called with view_mode: {:?}", view_mode);
        S_OK
    }

    unsafe fn SettingsIdentifier(&self, settings_identifier: *mut *mut c_void) -> HRESULT {
        todo!()
    }

    unsafe fn SetSettingsIdentifier(&self, settings_identifier: *mut c_void) -> HRESULT {
        todo!()
    }

    unsafe fn PickSingleFileAsync(
        &self,
        arg2: *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        // todo!("hehe");
        println!("PickSingleFileAsync called");
        let op: IAsyncOperation<IStorageFile> = windows_future::IAsyncOperation::spawn(|| {
            // blocking or synchronous work performed on the Windows thread pool
            // obtain_storage_file()
            println!("PickSingleFileAsync async start");

            std::thread::sleep(std::time::Duration::from_secs(10));
            println!("PickSingleFileAsync async end");

            Ok(MyStorageFile {
                name: "test.txt".to_string(),
            }
            .into())
        });
        unsafe {
            *arg2 = op.into_raw();
            windows_core::HRESULT(0)
        }
    }

    unsafe fn PickMultipleFilesAsync(
        &self,
        arg2: *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        todo!()
    }
}

impl windows_core::RuntimeName for MyFileOpenPicker {
    const NAME: &'static str = "Windows.Storage.Pickers.FileOpenPicker";
}

// impl windows_core::RuntimeName for
// impl windows_core::RuntimeName for MyFileOpenPicker {
//     const NAME: &'static str = "Windows.Storage.ApplicationData";
// }
unsafe impl Send for MyFileOpenPicker {}
unsafe impl Sync for MyFileOpenPicker {}

#[implement(IStorageFile, IStorageItemProperties)]
struct MyStorageFile {
    name: String,
}

impl IStorageItemProperties_Impl for MyStorageFile_Impl {
    fn GetThumbnailAsyncOverloadDefaultSizeDefaultOptions(
        &self,
        mode: windows::Storage::FileProperties::ThumbnailMode,
    ) -> windows_core::Result<
        windows_future::IAsyncOperation<windows::Storage::FileProperties::StorageItemThumbnail>,
    > {
        todo!()
    }

    fn GetThumbnailAsyncOverloadDefaultOptions(
        &self,
        mode: windows::Storage::FileProperties::ThumbnailMode,
        requestedSize: u32,
    ) -> windows_core::Result<
        windows_future::IAsyncOperation<windows::Storage::FileProperties::StorageItemThumbnail>,
    > {
        todo!()
    }

    fn GetThumbnailAsync(
        &self,
        mode: windows::Storage::FileProperties::ThumbnailMode,
        requestedSize: u32,
        options: windows::Storage::FileProperties::ThumbnailOptions,
    ) -> windows_core::Result<
        windows_future::IAsyncOperation<windows::Storage::FileProperties::StorageItemThumbnail>,
    > {
        todo!()
    }

    fn DisplayName(&self) -> windows_core::Result<windows_core::HSTRING> {
        println!("DisplayName called, returning: {}", self.name);
        Ok(HSTRING::from(self.name.clone()))
    }

    fn DisplayType(&self) -> windows_core::Result<windows_core::HSTRING> {
        todo!()
    }

    fn FolderRelativeId(&self) -> windows_core::Result<windows_core::HSTRING> {
        todo!()
    }

    fn Properties(
        &self,
    ) -> windows_core::Result<windows::Storage::FileProperties::StorageItemContentProperties> {
        todo!()
    }
}

impl IInputStreamReference_Impl for MyStorageFile_Impl {
    fn OpenSequentialReadAsync(
        &self,
    ) -> windows_core::Result<
        windows_future::IAsyncOperation<windows::Storage::Streams::IInputStream>,
    > {
        todo!()
    }
}

impl IRandomAccessStreamReference_Impl for MyStorageFile_Impl {
    fn OpenReadAsync(
        &self,
    ) -> windows_core::Result<
        windows_future::IAsyncOperation<
            windows::Storage::Streams::IRandomAccessStreamWithContentType,
        >,
    > {
        todo!()
    }
}

impl IStorageItem_Impl for MyStorageFile_Impl {
    fn RenameAsyncOverloadDefaultOptions(
        &self,
        desiredName: &windows_core::HSTRING,
    ) -> windows_core::Result<windows_future::IAsyncAction> {
        todo!()
    }

    fn RenameAsync(
        &self,
        desiredName: &windows_core::HSTRING,
        option: windows::Storage::NameCollisionOption,
    ) -> windows_core::Result<windows_future::IAsyncAction> {
        todo!()
    }

    fn DeleteAsyncOverloadDefaultOptions(
        &self,
    ) -> windows_core::Result<windows_future::IAsyncAction> {
        todo!()
    }

    fn DeleteAsync(
        &self,
        option: windows::Storage::StorageDeleteOption,
    ) -> windows_core::Result<windows_future::IAsyncAction> {
        todo!()
    }

    fn GetBasicPropertiesAsync(
        &self,
    ) -> windows_core::Result<
        windows_future::IAsyncOperation<windows::Storage::FileProperties::BasicProperties>,
    > {
        todo!()
    }

    fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
        todo!()
    }

    fn Path(&self) -> windows_core::Result<windows_core::HSTRING> {
        todo!()
    }

    fn Attributes(&self) -> windows_core::Result<windows::Storage::FileAttributes> {
        todo!()
    }

    fn DateCreated(&self) -> windows_core::Result<windows_time::DateTime> {
        todo!()
    }

    fn IsOfType(&self, r#type: windows::Storage::StorageItemTypes) -> windows_core::Result<bool> {
        todo!()
    }
}

impl IStorageFile_Impl for MyStorageFile_Impl {
    fn FileType(&self) -> windows_core::Result<windows_core::HSTRING> {
        todo!()
    }

    fn ContentType(&self) -> windows_core::Result<windows_core::HSTRING> {
        todo!()
    }

    fn OpenAsync(
        &self,
        accessMode: windows::Storage::FileAccessMode,
    ) -> windows_core::Result<
        windows_future::IAsyncOperation<windows::Storage::Streams::IRandomAccessStream>,
    > {
        todo!()
    }

    fn OpenTransactedWriteAsync(
        &self,
    ) -> windows_core::Result<
        windows_future::IAsyncOperation<windows::Storage::StorageStreamTransaction>,
    > {
        todo!()
    }

    fn CopyOverloadDefaultNameAndOptions(
        &self,
        destinationFolder: windows_core::Ref<windows::Storage::IStorageFolder>,
    ) -> windows_core::Result<windows_future::IAsyncOperation<windows::Storage::StorageFile>> {
        todo!()
    }

    fn CopyOverloadDefaultOptions(
        &self,
        destinationFolder: windows_core::Ref<windows::Storage::IStorageFolder>,
        desiredNewName: &windows_core::HSTRING,
    ) -> windows_core::Result<windows_future::IAsyncOperation<windows::Storage::StorageFile>> {
        todo!()
    }

    fn CopyOverload(
        &self,
        destinationFolder: windows_core::Ref<windows::Storage::IStorageFolder>,
        desiredNewName: &windows_core::HSTRING,
        option: windows::Storage::NameCollisionOption,
    ) -> windows_core::Result<windows_future::IAsyncOperation<windows::Storage::StorageFile>> {
        todo!()
    }

    fn CopyAndReplaceAsync(
        &self,
        fileToReplace: windows_core::Ref<IStorageFile>,
    ) -> windows_core::Result<windows_future::IAsyncAction> {
        todo!()
    }

    fn MoveOverloadDefaultNameAndOptions(
        &self,
        destinationFolder: windows_core::Ref<windows::Storage::IStorageFolder>,
    ) -> windows_core::Result<windows_future::IAsyncAction> {
        todo!()
    }

    fn MoveOverloadDefaultOptions(
        &self,
        destinationFolder: windows_core::Ref<windows::Storage::IStorageFolder>,
        desiredNewName: &windows_core::HSTRING,
    ) -> windows_core::Result<windows_future::IAsyncAction> {
        todo!()
    }

    fn MoveOverload(
        &self,
        destinationFolder: windows_core::Ref<windows::Storage::IStorageFolder>,
        desiredNewName: &windows_core::HSTRING,
        option: windows::Storage::NameCollisionOption,
    ) -> windows_core::Result<windows_future::IAsyncAction> {
        todo!()
    }

    fn MoveAndReplaceAsync(
        &self,
        fileToReplace: windows_core::Ref<IStorageFile>,
    ) -> windows_core::Result<windows_future::IAsyncAction> {
        todo!()
    }
}

impl windows_core::RuntimeName for MyStorageFile {
    const NAME: &'static str = "Windows.Storage.StorageFile";
}

// #[implement(IVector<HSTRING>)]
// struct FileTypeFilter {
//     filter: Vec<String>
// }

#[implement(IActivationFactory)]
struct MyActivationFactory;

impl IActivationFactory_Impl for MyActivationFactory_Impl {
    fn ActivateInstance(&self) -> windows_core::Result<windows_core::IInspectable> {
        let p = MyFileOpenPicker {};
        let i: IInspectable = p.into();
        Ok(i)
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    return CLASS_E_CLASSNOTAVAILABLE;
}

// static FACTORY: MyActivationFactory = MyActivationFactory {};

#[unsafe(no_mangle)]
pub extern "system" fn DllGetActivationFactory(
    classid: ManuallyDrop<HSTRING>,
    factory: *mut *mut IActivationFactory,
) -> HRESULT {
    println!(
        "DllGetActivationFactory called with classid: {}",
        classid.to_string_lossy()
    );

    unsafe {
        *factory = std::ptr::null_mut();
    }
    // if classid.as_ref() == windows_strings::h!("Windows.Storage.Pickers.FileOpenPicker") {
    let t_f: MyActivationFactory = MyActivationFactory {};
    let ukn: IInspectable = t_f.into();
    let hr = unsafe { ukn.query(&IActivationFactory::IID, factory as *mut *mut c_void) };
    // std::mem::forget(ukn);
    println!(
        "DllGetActivationFactory done with classid: {} {}",
        classid.to_string_lossy(),
        hr
    );
    return hr;
    // }
    // if (!wcscmp( buffer, RuntimeClass_Windows_Management_Deployment_PackageManager ))
    //     IActivationFactory_QueryInterface( package_manager_factory, &IID_IActivationFactory, (void **)factory );

    // if (*factory) return S_OK;
    return CLASS_E_CLASSNOTAVAILABLE;
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
