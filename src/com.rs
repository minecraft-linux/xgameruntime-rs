use std::env::temp_dir;
use std::ffi::{CStr, c_char, c_void};
use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::{Mutex, OnceLock};
use windows_core::{BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, implement, interface};

const CLSID_XSTORE: GUID = GUID::from_u128(0x0dd112ac_7c24_448c_b92b_3960fb5bd30c);
const CLSID_XNETWORKING: GUID = GUID::from_u128(0x37e56907_2f10_41e8_b72f_36edb185331a);
const CLSID_XPERSISTENT_LOCAL_STORAGE: GUID =
    GUID::from_u128(0xf4faf4d4_2d04_4fce_b3e0_474a713a3e84);

const CLSID_XUSER: GUID = GUID::from_u128(0x01acd177_91f9_4763_a38e_ccbb55ce32e0);

use crate::threading::{IXAsync, XAsyncBlock, XTaskQueueHandle, XTaskQueueRegistrationToken};
use crate::user::{IXUser, XUser, XUserHandle};
use crate::xasync::get_result;
use crate::xnetworking::{
    IXNetworking, IXNetworking_Impl, XNetworkingConfigurationSetting,
    XNetworkingConnectivityCostHint, XNetworkingConnectivityHint, XNetworkingConnectivityLevelHint,
    XNetworkingSecurityInformation, XNetworkingStatisticsBuffer, XNetworkingStatisticsType,
};
use crate::xpackage::XPackageMountHandle;
use crate::xpersistedlocalstorage::{
    IXPersistentLocalStorage, IXPersistentLocalStorage_Impl, XPersistentLocalStorageSpaceInfo,
};
use crate::xstore::{
    self, IXStore, IXStore_Impl, XStoreAddonLicense, XStoreCanAcquireLicenseResult,
    XStoreConsumableResult, XStoreContextHandle, XStoreGameLicense,
    XStoreGameLicenseChangedCallback, XStoreLicenseHandle, XStorePackageLicenseLostCallback,
    XStorePackageUpdate, XStoreProductKind, XStoreProductQueryCallback, XStoreProductQueryHandle,
    XStoreRateAndReviewResult,
};
use crate::{E_FAIL, results::*, threading, xasync};

#[interface("8836fe87-edb9-4fe3-8dad-05f0d2cd5b40")]
pub unsafe trait IXFeature: IUnknown {
    unsafe fn xgame_runtime_is_feature_available(&self, feature: u32) -> bool;
}

#[implement(IXFeature)]
pub struct XFeature;

impl IXFeature_Impl for XFeature_Impl {
    unsafe fn xgame_runtime_is_feature_available(&self, feature: u32) -> bool {
        return true || feature != 10;
    }
}

#[implement(IXPersistentLocalStorage)]
pub struct XPersistentLocalStorage {
    tmp_path: String,
}

impl IXPersistentLocalStorage_Impl for XPersistentLocalStorage_Impl {
    unsafe fn x_persistent_local_storage_get_path_size(&self, path_size: *mut usize) -> HRESULT {
        unsafe {
            *path_size = self.tmp_path.len() + 1;
        }
        return S_OK;
    }

    unsafe fn x_persistent_local_storage_get_path(
        &self,
        path_size: usize,
        path: *mut c_char,
        path_used: *mut usize,
    ) -> HRESULT {
        let bytes = self.tmp_path.as_bytes();
        let len = bytes.len().min(path_size.saturating_sub(1));
        for (index, byte) in bytes.iter().copied().take(len).enumerate() {
            unsafe {
                *path.add(index) = byte as c_char;
            }
        }
        if path_size != 0 {
            unsafe {
                *path.add(len) = 0;
            }
        }
        unsafe {
            *path_used = len + 1;
        }
        return S_OK;
    }

    unsafe fn x_persistent_local_storage_get_space_info(
        &self,
        info: *mut XPersistentLocalStorageSpaceInfo,
    ) -> HRESULT {
        unsafe {
            *info = XPersistentLocalStorageSpaceInfo {
                available_free_bytes: 1024 * 1024 * 1024,
                total_free_bytes: 1024 * 1024 * 1024,
                used_bytes: 512 * 1024 * 1024,
                total_bytes: 2 * 1024 * 1024 * 1024,
            };
        }
        return S_OK;
    }

    unsafe fn x_persistent_local_storage_prompt_user_for_space_async(
        &self,
        _requested_bytes: u64,
        _async_block: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_persistent_local_storage_prompt_user_for_space_result(
        &self,
        _async_block: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_persistent_local_storage_mount_for_package(
        &self,
        _package_identifier: *const c_char,
        _mount_handle: *mut XPackageMountHandle,
    ) -> HRESULT {
        todo!()
    }
}

#[interface("5c48dedf-0b67-4492-a4b5-6829b8e796e1")]
pub unsafe trait IXStoreAlias1: xstore::IXStore {}

#[interface("b09d803c-2414-4a05-82c6-66dfdc9e9a44")]
pub unsafe trait IXStoreAlias2: xstore::IXStore {}

#[interface("2d42fea5-e71d-4b76-97cd-c50afbb3ae5d")]
pub unsafe trait IXStoreAlias3: xstore::IXStore {}

// XNetworkingConnectivityHintChangedCallback
pub type XNetworkingConnectivityHintChangedCallback = unsafe extern "system" fn(
    context: *mut c_void,
    connectivity_hint: *const XNetworkingConnectivityHint,
) -> ();

// XNetworkingPreferredLocalUdpMultiplayerPortChangedCallback
pub type XNetworkingPreferredLocalUdpMultiplayerPortChangedCallback =
    unsafe extern "system" fn(
        context: *mut c_void,
        preferred_local_udp_multiplayer_port: u16,
    ) -> ();

#[interface("bf2346b2-39af-4658-b5ea-44713c7e83b3")]
pub unsafe trait IXNetworking2: IXNetworking {}

#[implement(xstore::IXStore, IXStoreAlias1, IXStoreAlias2, IXStoreAlias3)]
pub struct XStoreObject;

impl IXStore_Impl for XStoreObject_Impl {
    unsafe fn x_store_create_context(
        &self,
        _user: XUserHandle,
        store_context_handle: *mut XStoreContextHandle,
    ) -> HRESULT {
        unsafe {
            *store_context_handle = 1;
        };
        HRESULT(0)
    }

    unsafe fn x_store_query_game_license_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        unsafe {
            xasync::run_sync(async_.cast(), move || {
                // println!("storeContextHandle: {storeContextHandle}");
                return Ok(XStoreGameLicense::default());
            })
        }
    }

    unsafe fn x_store_query_game_license_result(
        &self,
        async_: *mut XAsyncBlock,
        license: *mut XStoreGameLicense,
    ) -> HRESULT {
        // println!("XStoreQueryGameLicenseResult");
        if async_.is_null() || license.is_null() {
            return E_POINTER;
        }

        let mut payload = XStoreGameLicense::default();
        match unsafe { get_result(async_, null_mut(), &mut payload) } {
            Ok(_) => {
                unsafe {
                    *license = payload;
                }
                S_OK
            }
            Err(hr) => return hr,
        }
    }

    unsafe fn x_store_close_context_handle(
        &self,
        _store_context_handle: XStoreContextHandle,
    ) -> () {
        todo!()
    }

    unsafe fn x_store_query_associated_products_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _product_kinds: XStoreProductKind,
        _max_items_to_retrieve_per_page: u32,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_associated_products_result(
        &self,
        _async_: *mut XAsyncBlock,
        _product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_products_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _product_kinds: XStoreProductKind,
        _store_ids: *const *mut c_char,
        _store_ids_count: usize,
        _action_filters: *const *mut c_char,
        _action_filters_count: usize,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_products_result(
        &self,
        _async_: *mut XAsyncBlock,
        _product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_entitled_products_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _product_kinds: XStoreProductKind,
        _max_items_to_retrieve_per_page: u32,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_entitled_products_result(
        &self,
        _async_: *mut XAsyncBlock,
        _product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_product_for_current_game_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_product_for_current_game_result(
        &self,
        _async_: *mut XAsyncBlock,
        _product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_product_for_package_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _product_kinds: XStoreProductKind,
        _package_identifier: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_product_for_package_result(
        &self,
        _async_: *mut XAsyncBlock,
        _product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_enumerate_products_query(
        &self,
        _product_query_handle: XStoreProductQueryHandle,
        _context: *mut c_void,
        _callback: Option<XStoreProductQueryCallback>,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_products_query_has_more_pages(
        &self,
        _product_query_handle: XStoreProductQueryHandle,
    ) -> BOOL {
        todo!()
    }

    unsafe fn x_store_products_query_next_page_async(
        &self,
        _product_query_handle: XStoreProductQueryHandle,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_products_query_next_page_result(
        &self,
        _async_: *mut XAsyncBlock,
        _product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_close_products_query_handle(
        &self,
        _product_query_handle: XStoreProductQueryHandle,
    ) -> () {
        todo!()
    }

    unsafe fn x_store_acquire_license_for_package_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _package_identifier: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_acquire_license_for_package_result(
        &self,
        _async_: *mut XAsyncBlock,
        _store_license_handle: *mut XStoreLicenseHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_is_license_valid(&self, _store_license_handle: XStoreLicenseHandle) -> BOOL {
        todo!()
    }

    unsafe fn x_store_close_license_handle(
        &self,
        _store_license_handle: XStoreLicenseHandle,
    ) -> () {
        todo!()
    }

    unsafe fn x_store_can_acquire_license_for_store_id_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _store_product_id: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_can_acquire_license_for_store_id_result(
        &self,
        _async_: *mut XAsyncBlock,
        _store_can_acquire_license: *mut XStoreCanAcquireLicenseResult,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_can_acquire_license_for_package_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _package_identifier: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_can_acquire_license_for_package_result(
        &self,
        _async_: *mut XAsyncBlock,
        _store_can_acquire_license: *mut XStoreCanAcquireLicenseResult,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_add_on_licenses_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_add_on_licenses_result_count(
        &self,
        _async_: *mut XAsyncBlock,
        _count: *mut u32,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_add_on_licenses_result(
        &self,
        _async_: *mut XAsyncBlock,
        _count: u32,
        _add_on_licenses: *mut XStoreAddonLicense,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_consumable_balance_remaining_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _store_product_id: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_consumable_balance_remaining_result(
        &self,
        _async_: *mut XAsyncBlock,
        _consumable_result: *mut XStoreConsumableResult,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn __reserved_slot_35(&self) {
        todo!()
    }

    unsafe fn x_store_report_consumable_fulfillment_result(
        &self,
        _async_: *mut XAsyncBlock,
        _consumable_result: *mut XStoreConsumableResult,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_collections_id_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _service_ticket: *const c_char,
        _publisher_user_id: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_collections_id_result_size(
        &self,
        _async_: *mut XAsyncBlock,
        _size: *mut usize,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_collections_id_result(
        &self,
        _async_: *mut XAsyncBlock,
        _size: usize,
        _result: *mut c_char,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_purchase_id_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _service_ticket: *const c_char,
        _publisher_user_id: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_purchase_id_result_size(
        &self,
        _async_: *mut XAsyncBlock,
        _size: *mut usize,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_purchase_id_result(
        &self,
        _async_: *mut XAsyncBlock,
        _size: usize,
        _result: *mut c_char,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_license_token_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _product_ids: *const *mut c_char,
        _product_ids_count: usize,
        _custom_developer_string: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_license_token_result_size(
        &self,
        _async_: *mut XAsyncBlock,
        _size: *mut usize,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_license_token_result(
        &self,
        _async_: *mut XAsyncBlock,
        _size: usize,
        _result: *mut c_char,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn __reserved_slot_46(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_47(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_48(&self) {
        todo!()
    }

    unsafe fn x_store_show_purchase_u_i_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _store_id: *const c_char,
        _name: *const c_char,
        _extended_json_data: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_purchase_u_i_result(&self, _async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_rate_and_review_u_i_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_rate_and_review_u_i_result(
        &self,
        _async_: *mut XAsyncBlock,
        _result: *mut XStoreRateAndReviewResult,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_redeem_token_u_i_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _token: *const c_char,
        _allowed_store_ids: *const *mut c_char,
        _allowed_store_ids_count: usize,
        _disallow_csv_redemption: BOOL,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_redeem_token_u_i_result(&self, _async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_game_and_dlc_package_updates_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_game_and_dlc_package_updates_result_count(
        &self,
        _async_: *mut XAsyncBlock,
        _count: *mut u32,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_game_and_dlc_package_updates_result(
        &self,
        _async_: *mut XAsyncBlock,
        _count: u32,
        _package_updates: *mut XStorePackageUpdate,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_package_updates_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _package_identifiers: *const *mut c_char,
        _package_identifiers_count: usize,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_package_updates_result(&self, _async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_and_install_package_updates_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _package_identifiers: *const *mut c_char,
        _package_identifiers_count: usize,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_and_install_package_updates_result(
        &self,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_and_install_packages_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _store_ids: *const *mut c_char,
        _store_ids_count: usize,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_and_install_packages_result_count(
        &self,
        _async_: *mut XAsyncBlock,
        _count: *mut u32,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_and_install_packages_result(
        &self,
        _async_: *mut XAsyncBlock,
        _count: u32,
        _package_identifiers: *mut *mut c_char,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_package_identifier(
        &self,
        _store_id: *const c_char,
        _size: usize,
        _package_identifier: *mut c_char,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_register_game_license_changed(
        &self,
        _store_context_handle: XStoreContextHandle,
        _queue: XTaskQueueHandle,
        _context: *mut c_void,
        _callback: Option<XStoreGameLicenseChangedCallback>,
        _token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_unregister_game_license_changed(
        &self,
        _store_context_handle: XStoreContextHandle,
        _token: XTaskQueueRegistrationToken,
        _wait: BOOL,
    ) -> BOOL {
        todo!()
    }

    unsafe fn x_store_register_package_license_lost(
        &self,
        _license_handle: XStoreLicenseHandle,
        _queue: XTaskQueueHandle,
        _context: *mut c_void,
        _callback: Option<XStorePackageLicenseLostCallback>,
        _token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_unregister_package_license_lost(
        &self,
        _license_handle: XStoreLicenseHandle,
        _token: XTaskQueueRegistrationToken,
        _wait: BOOL,
    ) -> BOOL {
        todo!()
    }

    unsafe fn __reserved_slot_70(&self) {
        todo!()
    }

    unsafe fn x_store_acquire_license_for_durables_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _store_id: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_acquire_license_for_durables_result(
        &self,
        _async_: *mut XAsyncBlock,
        _store_license_handle: *mut XStoreLicenseHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_associated_products_u_i_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _store_id: *const c_char,
        _product_kinds: XStoreProductKind,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_associated_products_u_i_result(
        &self,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_product_page_u_i_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _store_id: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_product_page_u_i_result(&self, _async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_associated_products_for_store_id_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _store_id: *const c_char,
        _product_kinds: XStoreProductKind,
        _max_items_to_retrieve_per_page: u32,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_associated_products_for_store_id_result(
        &self,
        _async_: *mut XAsyncBlock,
        _product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_package_updates_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _package_identifiers: *const *mut c_char,
        _package_identifiers_count: usize,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_package_updates_result_count(
        &self,
        _async_: *mut XAsyncBlock,
        _count: *mut u32,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_package_updates_result(
        &self,
        _async_: *mut XAsyncBlock,
        _count: u32,
        _package_updates: *mut XStorePackageUpdate,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_gifting_u_i_async(
        &self,
        _store_context_handle: XStoreContextHandle,
        _store_id: *const c_char,
        _name: *const c_char,
        _extended_json_data: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_gifting_u_i_result(&self, _async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }
}

impl IXStoreAlias1_Impl for XStoreObject_Impl {}
impl IXStoreAlias2_Impl for XStoreObject_Impl {}
impl IXStoreAlias3_Impl for XStoreObject_Impl {}

#[implement(IXNetworking, IXNetworking2)]
pub struct XNetworkingObject;

impl IXNetworking_Impl for XNetworkingObject_Impl {
    unsafe fn x_networking_get_connectivity_hint(
        &self,
        connectivity_hint: *mut XNetworkingConnectivityHint,
    ) -> HRESULT {
        if connectivity_hint.is_null() {
            return E_POINTER;
        }
        unsafe {
            *connectivity_hint = XNetworkingConnectivityHint {
                connectivity_level: XNetworkingConnectivityLevelHint::InternetAccess,
                connectivity_cost: XNetworkingConnectivityCostHint::Unrestricted,
                iana_interface_type: 6,
                network_initialized: true.into(),
                approaching_data_limit: false.into(),
                over_data_limit: false.into(),
                roaming: false.into(),
            };
        }
        S_OK
    }

    unsafe fn x_networking_query_security_information_for_url_async(
        &self,
        url: *const c_char,
        async_block: *mut XAsyncBlock,
    ) -> HRESULT {
        let url = unsafe { CStr::from_ptr(url) };
        println!(
            "XNetworkingQuerySecurityInformationForUrlAsync {}",
            url.to_string_lossy()
        );
        unsafe {
            let storage = url.to_str().unwrap_or_default();
            xasync::run_sync(async_block, move || {
                println!(
                    "XNetworkingQuerySecurityInformationForUrlAsync: storage: {}",
                    storage
                );
                Ok(XNetworkingSecurityInformation {
                    enabled_http_security_protocol_flags: 0x00000080
                        | 0x00000200
                        | 0x00000800
                        | 0x00002000,
                    thumbprint_count: 0,
                    thumbprints: null_mut(),
                })
            })
        }
    }

    unsafe fn x_networking_query_security_information_for_url_async_result(
        &self,
        async_block: *mut XAsyncBlock,
        security_information_buffer_byte_count: usize,
        security_information_buffer_byte_count_used: *mut usize,
        security_information_buffer: *mut u8,
        security_information: *mut *mut XNetworkingSecurityInformation,
    ) -> HRESULT {
        if security_information_buffer_byte_count < size_of::<XNetworkingSecurityInformation>() {
            return E_FAIL;
        }
        if !security_information_buffer_byte_count_used.is_null() {
            unsafe { *security_information_buffer_byte_count_used = 0 };
        }
        match unsafe {
            get_result(
                async_block,
                null_mut(),
                security_information_buffer.cast::<XNetworkingSecurityInformation>(),
            )
        } {
            Ok(_) => {
                if !security_information_buffer_byte_count_used.is_null() {
                    unsafe {
                        *security_information_buffer_byte_count_used =
                            size_of::<XNetworkingSecurityInformation>()
                    };
                }
                unsafe { *security_information = security_information_buffer.cast() };
                S_OK
            }
            Err(hr) => hr,
        }
    }

    unsafe fn x_networking_query_security_information_for_url_async_result_size(
        &self,
        async_block: *mut XAsyncBlock,
        security_information_buffer_byte_count: *mut usize,
    ) -> HRESULT {
        let r = unsafe { xasync::get_result_size(async_block) };
        match r {
            Ok(size) => unsafe {
                *security_information_buffer_byte_count = size;
                S_OK
            },
            Err(hr) => hr,
        }
    }

    unsafe fn x_networking_query_security_information_for_url_utf16_async(
        &self,
        url: *const u16,
        async_block: *mut XAsyncBlock,
    ) -> HRESULT {
        let url = PCWSTR::from_raw(url);
        println!(
            "XNetworkingQuerySecurityInformationForUrlUtf16Async {} thread: {:?}",
            unsafe { url.to_string() }.unwrap(),
            std::thread::current().id(),
        );
        unsafe {
            let storage = url.to_string().unwrap();
            xasync::run_sync(async_block, move || {
                println!(
                    "XNetworkingQuerySecurityInformationForUrlUtf16Async: storage: {} thread: {:?}",
                    storage,
                    std::thread::current().id()
                );
                Ok(XNetworkingSecurityInformation {
                    enabled_http_security_protocol_flags: 0x00000080
                        | 0x00000200
                        | 0x00000800
                        | 0x00002000,
                    thumbprint_count: 0,
                    thumbprints: null_mut(),
                })
            })
        }
    }

    unsafe fn x_networking_query_security_information_for_url_utf16_async_result(
        &self,
        async_block: *mut XAsyncBlock,
        security_information_buffer_byte_count: usize,
        security_information_buffer_byte_count_used: *mut usize,
        security_information_buffer: *mut u8,
        security_information: *mut *mut XNetworkingSecurityInformation,
    ) -> HRESULT {
        println!(
            "XNetworkingQuerySecurityInformationForUrlUtf16AsyncResult thread: {:?}",
            std::thread::current().id()
        );
        if security_information_buffer_byte_count < size_of::<XNetworkingSecurityInformation>() {
            return E_FAIL;
        }
        if !security_information_buffer_byte_count_used.is_null() {
            unsafe { *security_information_buffer_byte_count_used = 0 };
        }
        match unsafe {
            get_result(
                async_block,
                null_mut(),
                security_information_buffer.cast::<XNetworkingSecurityInformation>(),
            )
        } {
            Ok(_) => {
                if !security_information_buffer_byte_count_used.is_null() {
                    unsafe {
                        *security_information_buffer_byte_count_used =
                            size_of::<XNetworkingSecurityInformation>()
                    };
                }
                unsafe { *security_information = security_information_buffer.cast() };
                println!(
                    "XNetworkingQuerySecurityInformationForUrlUtf16AsyncResult: OK thread: {:?}",
                    std::thread::current().id()
                );
                S_OK
            }
            Err(hr) => hr,
        }
    }

    unsafe fn x_networking_query_security_information_for_url_utf16_async_result_size(
        &self,
        async_block: *mut XAsyncBlock,
        security_information_buffer_byte_count: *mut usize,
    ) -> HRESULT {
        println!(
            "XNetworkingQuerySecurityInformationForUrlUtf16AsyncResultSize thread: {:?}",
            std::thread::current().id()
        );
        let r = unsafe { xasync::get_result_size(async_block) };
        match r {
            Ok(size) => unsafe {
                *security_information_buffer_byte_count = size;
                S_OK
            },
            Err(hr) => hr,
        }
    }

    unsafe fn x_networking_register_connectivity_hint_changed(
        &self,
        _queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: Option<XNetworkingConnectivityHintChangedCallback>,
        _token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT {
        if let Some(callback) = callback {
            // println!("XNetworkingRegisterConnectivityHintChanged");
            unsafe {
                callback(
                    context,
                    &XNetworkingConnectivityHint {
                        connectivity_level: XNetworkingConnectivityLevelHint::InternetAccess,
                        connectivity_cost: XNetworkingConnectivityCostHint::Unrestricted,
                        iana_interface_type: 6,
                        network_initialized: true.into(),
                        approaching_data_limit: false.into(),
                        over_data_limit: false.into(),
                        roaming: false.into(),
                    },
                )
            };
        }
        S_OK
    }

    unsafe fn x_networking_verify_server_certificate(
        &self,
        _request_handle: *mut c_void,
        _security_information: *const XNetworkingSecurityInformation,
    ) -> HRESULT {
        S_OK
    }

    unsafe fn x_networking_query_preferred_local_udp_multiplayer_port(
        &self,
        _preferred_local_udp_multiplayer_port: *mut u16,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_query_preferred_local_udp_multiplayer_port_async(
        &self,
        _async_block: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_query_preferred_local_udp_multiplayer_port_async_result(
        &self,
        _async_block: *mut XAsyncBlock,
        _preferred_local_udp_multiplayer_port: *mut u16,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_register_preferred_local_udp_multiplayer_port_changed(
        &self,
        _queue: XTaskQueueHandle,
        _context: *mut c_void,
        _callback: Option<XNetworkingPreferredLocalUdpMultiplayerPortChangedCallback>,
        _token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_unregister_preferred_local_udp_multiplayer_port_changed(
        &self,
        _token: XTaskQueueRegistrationToken,
        _wait: BOOL,
    ) -> BOOL {
        todo!()
    }

    unsafe fn x_networking_unregister_connectivity_hint_changed(
        &self,
        _token: XTaskQueueRegistrationToken,
        _wait: BOOL,
    ) -> BOOL {
        todo!()
    }

    unsafe fn x_networking_query_configuration_setting(
        &self,
        _configuration_setting: XNetworkingConfigurationSetting,
        _value: *mut u64,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_set_configuration_setting(
        &self,
        _configuration_parameter: XNetworkingConfigurationSetting,
        _value: u64,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_query_statistics(
        &self,
        _statistics_type: XNetworkingStatisticsType,
        _statistics_buffer: *mut XNetworkingStatisticsBuffer,
    ) -> HRESULT {
        todo!()
    }
}

impl IXNetworking2_Impl for XNetworkingObject_Impl {}

struct GlobalInterface<T>(T);

unsafe impl<T> Send for GlobalInterface<T> {}
unsafe impl<T> Sync for GlobalInterface<T> {}

static XFEATURE_SINGLETON: OnceLock<GlobalInterface<IXFeature>> = OnceLock::new();
static XSTORE_SINGLETON: OnceLock<GlobalInterface<IXStore>> = OnceLock::new();
static XNETWORKING_SINGLETON: OnceLock<GlobalInterface<IXNetworking>> = OnceLock::new();
static XPERSISTENT_LOCAL_STORAGE_SINGLETON: OnceLock<GlobalInterface<IXPersistentLocalStorage>> =
    OnceLock::new();
static XUSER_SINGLETON: OnceLock<GlobalInterface<IXUser>> = OnceLock::new();
static XASYNC_SINGLETON: OnceLock<GlobalInterface<IXAsync>> = OnceLock::new();
fn xfeature_singleton() -> &'static IXFeature {
    &XFEATURE_SINGLETON
        .get_or_init(|| GlobalInterface(XFeature.into()))
        .0
}

fn xstore_singleton() -> &'static IXStore {
    &XSTORE_SINGLETON
        .get_or_init(|| GlobalInterface(XStoreObject.into()))
        .0
}

fn xnetworking_singleton() -> &'static IXNetworking {
    &XNETWORKING_SINGLETON
        .get_or_init(|| GlobalInterface(XNetworkingObject.into()))
        .0
}

fn xpersistent_local_storage_singleton() -> &'static IXPersistentLocalStorage {
    &XPERSISTENT_LOCAL_STORAGE_SINGLETON
        .get_or_init(|| {
            GlobalInterface(
                XPersistentLocalStorage {
                    tmp_path: temp_dir().to_string_lossy().into_owned(),
                }
                .into(),
            )
        })
        .0
}

fn xuser_singleton() -> &'static IXUser {
    &XUSER_SINGLETON
        .get_or_init(|| {
            GlobalInterface(
                XUser {
                    runtime: tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .build()
                        .unwrap(),
                }
                .into(),
            )
        })
        .0
}

fn xasync_singleton() -> &'static IXAsync {
    &XASYNC_SINGLETON
        .get_or_init(|| {
            let async_: threading::IXAsync = threading::XAsync {
                process_queue: Mutex::new(null_mut()),
                runtime: tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap(),
            }
            .into();

            let mut queue: *mut c_void = std::ptr::null_mut();
            let _ = unsafe {
                async_.x_task_queue_create(
                    threading::XTaskQueueDispatchMode::ThreadPool,
                    threading::XTaskQueueDispatchMode::ThreadPool,
                    &mut queue,
                )
            };
            let _ = unsafe { async_.x_task_queue_set_current_process_task_queue(queue) };
            GlobalInterface(async_)
        })
        .0
}

fn query<T: Interface + Clone>(
    object: &T,
    interface_id: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if interface_id.is_null() || out.is_null() {
        return E_POINTER;
    }
    let object = object.clone();
    let interface_id = unsafe { *interface_id };
    if unsafe { object.query(&interface_id, out) }.is_ok() {
        // println!("query: ack {:#32x}", interface_id.to_u128());
        S_OK
    } else {
        println!("query: nack {:#32x}", interface_id.to_u128());
        unsafe {
            *out = std::ptr::null_mut();
        }
        E_NOINTERFACE
    }
}

pub fn query_api_impl(
    runtime_class_id: *const GUID,
    interface_id: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if runtime_class_id.is_null() || interface_id.is_null() || out.is_null() {
        return E_POINTER;
    }

    let class_id = unsafe { *runtime_class_id };
    // println!("query_api_impl: {:#8x}-{:#4x}-{:#4x}-{:#4x}", class_id.data1, class_id.data2, class_id.data3, class_id.data4);
    let res = match class_id {
        IXFeature::IID => {
            // println!("query_api_impl: {:#32x} {:#32x}", class_id.to_u128(), unsafe { *interface_id }.to_u128());
            query(xfeature_singleton(), interface_id, out)
        }
        CLSID_XSTORE => {
            // println!("query_api_impl: {:#32x} {:#32x}", class_id.to_u128(), unsafe { *interface_id }.to_u128());
            query(xstore_singleton(), interface_id, out)
        }
        CLSID_XNETWORKING => {
            // println!(
            //     "query_api_impl: {:#32x} {:#32x}",
            //     class_id.to_u128(),
            //     unsafe { *interface_id }.to_u128()
            // );
            query(xnetworking_singleton(), interface_id, out)
        }
        CLSID_XPERSISTENT_LOCAL_STORAGE => {
            query(xpersistent_local_storage_singleton(), interface_id, out)
        }
        CLSID_XUSER => query(xuser_singleton(), interface_id, out),
        xasync::CLSID_XASYNC => query(xasync_singleton(), interface_id, out),
        _ => crate::delegated_query_api_impl(runtime_class_id, interface_id, out),
    };
    res
}

#[cfg(test)]
mod tests {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    use crate::com::{IXStore, XStoreGameLicense, get_result, query_api_impl};
    use crate::xasync::{XAsyncBlock, get_status, run};
    use crate::{
        E_FAIL, InitializeApiImplEx2, UninitializeApiImpl, set_delegated_dll_path_for_test,
    };
    use windows_core::{GUID, HRESULT, Interface};

    #[test]
    fn test() {
        let mut out: *mut c_void = std::ptr::null_mut();
        let hr = query_api_impl(
            &crate::com::CLSID_XSTORE,
            &crate::com::IXStore::IID,
            &mut out,
        );

        assert_eq!(hr, HRESULT(0));

        let store: IXStore = unsafe { IXStore::from_raw(out) };

        unsafe {
            let mut store_ctx: u64 = 0;
            let hr = store.x_store_create_context(null_mut(), &mut store_ctx);
            assert_eq!(hr, HRESULT(0));
            let hr = store.x_store_query_game_license_async(store_ctx, std::ptr::null_mut());
            assert_eq!(hr, HRESULT(0));
        };
    }

    #[test]
    #[ignore = "requires xgameruntime.gdk.dll delegate support in the Wine environment"]
    fn query_game_license_async_blocks_via_xasync() {
        let init_hr = InitializeApiImplEx2(2604, 100000, 10, std::ptr::null_mut());
        assert_eq!(init_hr, HRESULT(0));

        let mut out = std::ptr::null_mut();
        let hr = query_api_impl(
            &crate::com::CLSID_XSTORE,
            &crate::com::IXStore::IID,
            &mut out,
        );
        assert_eq!(hr, HRESULT(0));

        let store: IXStore = unsafe { IXStore::from_raw(out) };
        let mut store_ctx: u64 = 0;
        let hr = unsafe { store.x_store_create_context(null_mut(), &mut store_ctx) };
        assert_eq!(hr, HRESULT(0));

        let mut async_block = XAsyncBlock {
            queue: std::ptr::null_mut(),
            context: std::ptr::null_mut(),
            callback: None,
            internal: [0; std::mem::size_of::<*mut c_void>() * 4],
        };
        let hr = unsafe { store.x_store_query_game_license_async(store_ctx, &mut async_block) };
        assert_eq!(hr, HRESULT(0));

        unsafe { get_status(&mut async_block, true) }.unwrap();

        let mut license = XStoreGameLicense::default();
        let result_hr =
            unsafe { store.x_store_query_game_license_result(&mut async_block, &mut license) };
        assert_eq!(result_hr, HRESULT(0));
        // assert_eq!(read_c_string(&license.skuStoreId), "TRIAL-SKU-001");
        assert!(license.is_active);
        assert!(!license.is_trial_owned_by_this_user);
        assert!(!license.is_trial);
        assert!(!license.is_disc_license);
        assert_eq!(license.trial_time_remaining_in_seconds, 0);
        // assert_eq!(read_c_string(&license.trialUniqueId), "trial-license");

        let mut async_block = XAsyncBlock {
            queue: std::ptr::null_mut(),
            context: std::ptr::null_mut(),
            callback: None,
            internal: [0; std::mem::size_of::<*mut c_void>() * 4],
        };

        let tokio = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create Tokio runtime");

        let handle = tokio.handle().clone();
        #[derive(Debug)]
        struct Payload {
            v: i32,
            v2: i64,
            v3: GUID,
        }

        let hr = unsafe {
            run(&mut async_block, async move {
                println!("starting");

                let task = handle.spawn(async {
                    let client = reqwest::Client::new();

                    let response = client
                        .get("http://google.com")
                        .send()
                        .await
                        .map_err(|_| E_FAIL)?;

                    println!("finished {}", response.status());

                    Ok::<Payload, HRESULT>(Payload {
                        v: 0,
                        v2: 323,
                        v3: GUID::zeroed(),
                    })
                });

                task.await.map_err(|_| E_FAIL)?
            })
        };
        assert_eq!(hr, HRESULT(0));

        unsafe { get_status(&mut async_block, true) }.unwrap();

        let mut payload: Payload = Payload {
            v: 0,
            v2: 0,
            v3: GUID::zeroed(),
        };
        unsafe { get_result(&mut async_block, std::ptr::null(), &mut payload) }.unwrap();

        println!("res {:?}", payload);

        assert_eq!(payload.v, 0);
        assert_eq!(payload.v2, 323);
        assert_eq!(payload.v3, GUID::zeroed());

        let uninit_hr = UninitializeApiImpl();
        assert_eq!(uninit_hr, HRESULT(0));
        set_delegated_dll_path_for_test(None);
    }
}
