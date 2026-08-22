use std::ffi::c_char;

use windows_core::{HRESULT, IUnknown, interface};

use crate::{
    com::XPersistentLocalStorageSpaceInfo, xasync::XAsyncBlock, xpackage::XPackageMountHandle,
};

// Class _GUID_f4faf4d4_2d04_4fce_b3e0_474a713a3e84
// IID _GUID_f4faf4d4_2d04_4fce_b3e0_474a713a3e84
#[interface("f4faf4d4-2d04-4fce-b3e0-474a713a3e84")]
pub unsafe trait IXPersistentLocalStorage: IUnknown {
    // XPersistentLocalStorageGetPathSize
    unsafe fn x_persistent_local_storage_get_path_size(
        self: &Self,
        _path_size: *mut usize,
    ) -> HRESULT;
    // XPersistentLocalStorageGetPath
    unsafe fn x_persistent_local_storage_get_path(
        self: &Self,
        _path_size: usize,
        _path: *mut c_char,
        _path_used: *mut usize,
    ) -> HRESULT;
    // XPersistentLocalStorageGetSpaceInfo
    unsafe fn x_persistent_local_storage_get_space_info(
        self: &Self,
        _info: *mut XPersistentLocalStorageSpaceInfo,
    ) -> HRESULT;
    // XPersistentLocalStoragePromptUserForSpaceAsync
    unsafe fn x_persistent_local_storage_prompt_user_for_space_async(
        self: &Self,
        _requested_bytes: u64,
        _async_block: *mut XAsyncBlock,
    ) -> HRESULT;
    // XPersistentLocalStoragePromptUserForSpaceResult
    unsafe fn x_persistent_local_storage_prompt_user_for_space_result(
        self: &Self,
        _async_block: *mut XAsyncBlock,
    ) -> HRESULT;
    // XPersistentLocalStorageMountForPackage
    unsafe fn x_persistent_local_storage_mount_for_package(
        self: &Self,
        _package_identifier: *const c_char,
        _mount_handle: *mut XPackageMountHandle,
    ) -> HRESULT;
}
