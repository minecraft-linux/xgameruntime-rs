use std::ffi::{c_char, c_void};

use windows_core::{BOOL, HRESULT, implement};
use crate::{E_NOTIMPL, threading::{XTaskQueueHandle, XTaskQueueRegistrationToken}, user::XUserHandle, xasync::XAsyncBlock, xgamesave::{IXGameSave, IXGameSave_Impl, XGameSaveBlob, XGameSaveBlobInfoCallback, XGameSaveContainerHandle, XGameSaveContainerInfoCallback, XGameSaveProviderHandle, XGameSaveUpdateHandle}, xpackage::{IXPackage_Impl, XPackageChunkAvailability, XPackageChunkSelector, XPackageInstallationMonitorHandle, XPackageInstallationProgress, XPackageInstallationProgressCallback, *}};

#[implement(IXGameSave, IXPackage)]
pub struct XStub;

impl IXGameSave_Impl for XStub_Impl {
    unsafe fn x_game_save_initialize_provider(&self,_requesting_user: XUserHandle,_configuration_id: *const c_char,_sync_on_demand: BOOL,_provider: *mut XGameSaveProviderHandle) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_initialize_provider_async(&self,_requesting_user: XUserHandle,_configuration_id: *const c_char,_sync_on_demand: BOOL,_async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_initialize_provider_result(&self,_async_: *mut XAsyncBlock,_provider: *mut XGameSaveProviderHandle) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_close_provider(&self,_provider: XGameSaveProviderHandle) -> () {
        todo!()
    }

    unsafe fn x_game_save_get_remaining_quota(&self,_provider: XGameSaveProviderHandle,_remaining_quota: *mut i64) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_get_remaining_quota_async(&self,_provider: XGameSaveProviderHandle,_async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_get_remaining_quota_result(&self,_async_: *mut XAsyncBlock,_remaining_quota: *mut i64) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_delete_container(&self,_provider: XGameSaveProviderHandle,_container_name: *const c_char) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_delete_container_async(&self,_provider: XGameSaveProviderHandle,_container_name: *const c_char,_async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_delete_container_result(&self,_async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_get_container_info(&self,_provider: XGameSaveProviderHandle,_container_name: *const c_char,_context: *mut c_void,_callback: Option<XGameSaveContainerInfoCallback>) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_enumerate_container_info(&self,_provider: XGameSaveProviderHandle,_context: *mut c_void,_callback: Option<XGameSaveContainerInfoCallback>) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_enumerate_container_info_by_name(&self,_provider: XGameSaveProviderHandle,_container_name_prefix: *const c_char,_context: *mut c_void,_callback: Option<XGameSaveContainerInfoCallback>) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_create_container(&self,_provider: XGameSaveProviderHandle,_container_name: *const c_char,_container_context: *mut XGameSaveContainerHandle) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_close_container(&self,_context: XGameSaveContainerHandle) -> () {
        todo!()
    }

    unsafe fn x_game_save_enumerate_blob_info(&self,_container: XGameSaveContainerHandle,_context: *mut c_void,_callback: Option<XGameSaveBlobInfoCallback>) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_enumerate_blob_info_by_name(&self,_container: XGameSaveContainerHandle,_blob_name_prefix: *const c_char,_context: *mut c_void,_callback: Option<XGameSaveBlobInfoCallback>) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_read_blob_data(&self,_container: XGameSaveContainerHandle,_blob_names: *const *mut c_char,_count_of_blobs: *mut u32,_blobs_size: usize,_blob_data: *mut XGameSaveBlob) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_read_blob_data_async(&self,_container: XGameSaveContainerHandle,_blob_names: *const *mut c_char,_count_of_blobs: u32,_async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_read_blob_data_result(&self,_async_: *mut XAsyncBlock,_blobs_size: usize,_blob_data: *mut XGameSaveBlob,_count_of_blobs: *mut u32) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_create_update(&self,_container: XGameSaveContainerHandle,_container_display_name: *const c_char,_update_context: *mut XGameSaveUpdateHandle) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_close_update(&self,_context: XGameSaveUpdateHandle) -> () {
        todo!()
    }

    unsafe fn x_game_save_submit_blob_write(&self,_update_context: XGameSaveUpdateHandle,_blob_name: *const c_char,_data: *const u8,_byte_count: usize) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_submit_blob_delete(&self,_update_context: XGameSaveUpdateHandle,_blob_name: *const c_char) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_submit_update(&self,_update_context: XGameSaveUpdateHandle) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_submit_update_async(&self,_update_context: XGameSaveUpdateHandle,_async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_submit_update_result(&self,_async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_files_get_folder_with_ui_async(&self,_requesting_user: XUserHandle,_configuration_id: *const char,_async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_files_get_folder_with_ui_result(&self,_async_: *mut XAsyncBlock,_folder_size: usize,_folder_result: *mut char) -> HRESULT {
        todo!()
    }

    unsafe fn x_game_save_files_get_remaining_quota(&self,_user_context: XUserHandle,_configuration_id: *const char,_remaining_quota: *mut i64) -> HRESULT {
        todo!()
    }

    unsafe fn __reserved_slot_33(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_34(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_35(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_36(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_37(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_38(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_39(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_40(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_41(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_42(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_43(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_44(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_45(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_46(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_47(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_48(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_49(&self,) {
        todo!()
    }
}

impl IXPackage_Impl for XStub_Impl {
    unsafe fn x_package_get_current_process_package_identifier(&self,_buffer_size: usize,_buffer: *mut c_char) -> HRESULT {
        // todo!()
        E_NOTIMPL
    }

    unsafe fn x_package_is_packaged_process(&self,) -> BOOL {
        true.into()
    }

    unsafe fn x_package_create_installation_monitor(&self,_package_identifier: *const c_char,_selector_count: u32,_selectors: *mut XPackageChunkSelector,_minimum_update_interval_ms: u32,_queue: XTaskQueueHandle,_installation_monitor: *mut XPackageInstallationMonitorHandle) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_close_installation_monitor_handle(&self,_installation_monitor: XPackageInstallationMonitorHandle) -> () {
        todo!()
    }

    unsafe fn x_package_get_installation_progress(&self,_installation_monitor: XPackageInstallationMonitorHandle,_progress: *mut XPackageInstallationProgress) -> () {
        todo!()
    }

    unsafe fn x_package_update_installation_monitor(&self,_installation_monitor: XPackageInstallationMonitorHandle) -> BOOL {
        todo!()
    }

    unsafe fn x_package_register_installation_progress_changed(&self,_installation_monitor: XPackageInstallationMonitorHandle,_context: *mut c_void,_callback: Option<XPackageInstallationProgressCallback> ,_token: *mut XTaskQueueRegistrationToken) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_unregister_installation_progress_changed(&self,_installation_monitor: XPackageInstallationMonitorHandle,_token: XTaskQueueRegistrationToken,_wait: BOOL) -> BOOL {
        todo!()
    }

    unsafe fn x_package_get_user_locale(&self,_locale_size: usize,_locale: *mut c_char) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_find_chunk_availability(&self,_package_identifier: *const c_char,_selector_count: u32,_selectors: *mut XPackageChunkSelector,_availability: *mut XPackageChunkAvailability) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_enumerate_chunk_availability(&self,_package_identifier: *const c_char,_type_: XPackageChunkSelectorType,_context: *mut c_void,_callback: Option<XPackageChunkAvailabilityCallback>) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_change_chunk_install_order(&self,_package_identifier: *const c_char,_selector_count: u32,_selectors: *mut XPackageChunkSelector) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_install_chunks(&self,_package_identifier: *const c_char,_selector_count: u32,_selectors: *mut XPackageChunkSelector,_minimum_update_interval_ms: u32,_suppress_user_confirmation: BOOL,_queue: XTaskQueueHandle,_installation_monitor: *mut XPackageInstallationMonitorHandle) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_install_chunks_async(&self,_package_identifier: *const c_char,_selector_count: u32,_selectors: *mut XPackageChunkSelector,_minimum_update_interval_ms: u32,_suppress_user_confirmation: BOOL,_async_block: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_install_chunks_result(&self,_async_block: *mut XAsyncBlock,_installation_monitor: *mut XPackageInstallationMonitorHandle) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_estimate_download_size(&self,_package_identifier: *const c_char,_selector_count: u32,_selectors: *mut XPackageChunkSelector,_download_size: *mut u64,_should_present_user_confirmation: *mut BOOL) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_uninstall_chunks(&self,_package_identifier: *const c_char,_selector_count: u32,_selectors: *mut XPackageChunkSelector) -> HRESULT {
        todo!()
    }

    unsafe fn __reserved_slot_20(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_21(&self,) {
        todo!()
    }

    unsafe fn x_package_unregister_package_installed(&self,_token: XTaskQueueRegistrationToken,_wait: BOOL) -> BOOL {
        todo!()
    }

    unsafe fn __reserved_slot_23(&self,) {
        todo!()
    }

    unsafe fn x_package_get_mount_path_size(&self,_mount: XPackageMountHandle,_path_size: *mut usize) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_get_mount_path(&self,_mount: XPackageMountHandle,_path_size: usize,_path: *mut c_char) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_close_mount_handle(&self,_mount: XPackageMountHandle) -> () {
        todo!()
    }

    unsafe fn __reserved_slot_27(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_28(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_29(&self,) {
        todo!()
    }

    unsafe fn x_package_get_write_stats(&self,_write_stats: *mut XPackageWriteStats) -> HRESULT {
        todo!()
    }

    unsafe fn __reserved_slot_31(&self,) {
        todo!()
    }

    unsafe fn x_package_uninstall_u_w_p_instance(&self,_package_name: *const c_char) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_enumerate_features(&self,_package_identifier: *const c_char,_context: *mut c_void,_callback: Option<XPackageFeatureEnumerationCallback>) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_uninstall_package(&self,_package_identifier: *const c_char) -> BOOL {
        todo!()
    }

    unsafe fn __reserved_slot_35(&self,) {
        todo!()
    }

    unsafe fn __reserved_slot_36(&self,) {
        todo!()
    }

    unsafe fn x_package_mount_with_ui_async(&self,_package_identifier: *const char,_async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_mount_with_ui_result(&self,_async_: *mut XAsyncBlock,_mount: *mut XPackageMountHandle) -> HRESULT {
        todo!()
    }

    unsafe fn x_package_enumerate_packages(&self,_kind: XPackageKind,_scope: XPackageEnumerationScope,_context: *mut c_void,_callback: Option<XPackageEnumerationCallback>) -> HRESULT {
        // todo!()
        E_NOTIMPL
    }

    unsafe fn x_package_register_package_installed(&self,_queue: XTaskQueueHandle,_context: *mut c_void,_callback: Option<XPackageInstalledCallback> ,_token: *mut XTaskQueueRegistrationToken) -> HRESULT {
        todo!()
    }
}