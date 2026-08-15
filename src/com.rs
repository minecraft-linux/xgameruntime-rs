#[allow(nonstandard_style, non_snake_case)]
use super::E_NOTIMPL;
use std::env::temp_dir;
use std::ffi::{CStr, c_char, c_void};
use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::{Mutex, OnceLock};
use windows_core::{GUID, HRESULT, IUnknown, Interface, PCWSTR, implement, interface};
use windows_sys::core::BOOL;

const CLSID_XSTORE: GUID = GUID::from_u128(0x0dd112ac_7c24_448c_b92b_3960fb5bd30c);
const CLSID_XNETWORKING: GUID = GUID::from_u128(0x37e56907_2f10_41e8_b72f_36edb185331a);
const CLSID_XPERSISTENT_LOCAL_STORAGE: GUID =
    GUID::from_u128(0xf4faf4d4_2d04_4fce_b3e0_474a713a3e84);

const CLSID_XUSER: GUID = GUID::from_u128(0x01acd177_91f9_4763_a38e_ccbb55ce32e0);
const STORE_SKU_ID_SIZE: usize = 18;
const TRIAL_UNIQUE_ID_MAX_SIZE: usize = 64;

use crate::threading::{IXAsync, XAsyncBlock, XTaskQueueHandle, XTaskQueueRegistrationToken};
use crate::user::{IXUser, XUser, XUserHandle};
use crate::xasync::get_result;
use crate::{E_FAIL, results::*, threading, xasync};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XStoreGameLicense {
    pub sku_store_id: [c_char; STORE_SKU_ID_SIZE],
    pub is_active: bool,
    pub is_trial_owned_by_this_user: bool,
    pub is_disc_license: bool,
    pub is_trial: bool,
    pub trial_time_remaining_in_seconds: u32,
    pub trial_unique_id: [c_char; TRIAL_UNIQUE_ID_MAX_SIZE],
    pub expiration_date: i64,
}

impl Default for XStoreGameLicense {
    fn default() -> Self {
        Self {
            sku_store_id: [0; STORE_SKU_ID_SIZE],
            is_active: false,
            is_trial_owned_by_this_user: false,
            is_disc_license: false,
            is_trial: false,
            trial_time_remaining_in_seconds: 0,
            trial_unique_id: [0; TRIAL_UNIQUE_ID_MAX_SIZE],
            expiration_date: 0,
        }
    }
}

fn write_c_string<const N: usize>(dst: &mut [c_char; N], value: &[u8]) {
    let len = value.len().min(N.saturating_sub(1));
    for (index, byte) in value.iter().copied().take(len).enumerate() {
        dst[index] = byte as c_char;
    }
    if N != 0 {
        dst[len] = 0;
    }
}

fn build_trial_game_license() -> XStoreGameLicense {
    let mut license = XStoreGameLicense {
        is_active: true,
        is_trial_owned_by_this_user: true,
        is_disc_license: false,
        is_trial: true,
        trial_time_remaining_in_seconds: 3600,
        expiration_date: 4_102_444_800,
        ..XStoreGameLicense::default()
    };
    write_c_string(&mut license.sku_store_id, b"TRIAL-SKU-001");
    write_c_string(&mut license.trial_unique_id, b"trial-license");
    license
}

#[repr(C)]
struct XStoreQueryGameLicenseAsyncResultPayload {
    license: XStoreGameLicense,
}

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

#[repr(C)]
pub struct XPersistentLocalStorageSpaceInfo {
    pub available_free_bytes: u64,
    pub total_free_bytes: u64,
    pub used_bytes: u64,
    pub total_bytes: u64,
}

pub type XPackageMountHandle = u64;

#[interface("41a4e10c-5a7e-41d9-8c37-37bde62a07d6")]
pub unsafe trait IXPersistentLocalStorage: IUnknown {
    pub unsafe fn x_persistent_local_storage_get_path_size(self: &Self, path_size: *mut usize);
    pub unsafe fn x_persistent_local_storage_get_path(
        self: &Self,
        path_size: usize,
        path: *mut c_char,
        path_used: *mut usize,
    );
    pub unsafe fn x_persistent_local_storage_get_space_info(
        self: &Self,
        info: *mut XPersistentLocalStorageSpaceInfo,
    );
    pub unsafe fn x_persistent_local_storage_prompt_user_for_space_async(
        self: &Self,
        requested_bytes: u64,
        async_block: *mut XAsyncBlock,
    );
    pub unsafe fn x_persistent_local_storage_prompt_user_for_space_result(
        self: &Self,
        async_block: *mut XAsyncBlock,
    );
    pub unsafe fn x_persistent_local_storage_mount_for_package(
        self: &Self,
        package_identifier: *const c_char,
        mount_handle: *mut XPackageMountHandle,
    );
}

#[implement(IXPersistentLocalStorage)]
pub struct XPersistentLocalStorage {
    tmp_path: String,
}

impl IXPersistentLocalStorage_Impl for XPersistentLocalStorage_Impl {
    unsafe fn x_persistent_local_storage_get_path_size(&self, path_size: *mut usize) {
        unsafe {
            *path_size = self.tmp_path.len() + 1;
        }
    }

    unsafe fn x_persistent_local_storage_get_path(
        &self,
        path_size: usize,
        path: *mut c_char,
        path_used: *mut usize,
    ) {
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
    }

    unsafe fn x_persistent_local_storage_get_space_info(
        &self,
        info: *mut XPersistentLocalStorageSpaceInfo,
    ) {
        unsafe {
            *info = XPersistentLocalStorageSpaceInfo {
                available_free_bytes: 1024 * 1024 * 1024,
                total_free_bytes: 1024 * 1024 * 1024,
                used_bytes: 512 * 1024 * 1024,
                total_bytes: 2 * 1024 * 1024 * 1024,
            };
        }
    }

    unsafe fn x_persistent_local_storage_prompt_user_for_space_async(
        &self,
        requested_bytes: u64,
        async_block: *mut XAsyncBlock,
    ) {
        todo!()
    }

    unsafe fn x_persistent_local_storage_prompt_user_for_space_result(
        &self,
        async_block: *mut XAsyncBlock,
    ) {
        todo!()
    }

    unsafe fn x_persistent_local_storage_mount_for_package(
        &self,
        package_identifier: *const c_char,
        mount_handle: *mut XPackageMountHandle,
    ) {
        todo!()
    }
}

#[repr(u32)]
pub enum XStoreCanLicenseStatus {
    NotLicensableToUser = 0,
    Licensable = 1,
    LicenseActionNotApplicableToProduct = 2,
}
#[repr(u32)]
pub enum XStoreDurationUnit {
    Minute = 0,
    Hour = 1,
    Day = 2,
    Week = 3,
    Month = 4,
    Year = 5,
}
#[repr(u32)]
pub enum XStoreProductKind {
    None = 0x00,
    Consumable = 0x01,
    Durable = 0x02,
    Game = 0x04,
    Pass = 0x08,
    UnmanagedConsumable = 0x10,
}

#[repr(C)]
pub struct XStorePrice {}

#[repr(C)]
pub struct XVersion {}

pub type XStoreContextHandle = u64;
pub type XStoreLicenseHandle = u64;
pub type XStoreProductQueryHandle = u64;

#[repr(C)]
pub struct XStoreAvailability {
    pub availability_id: *const c_char,
    pub price: XStorePrice,
    pub end_date: libc::time_t,
}
#[repr(C)]
pub struct XStoreCollectionData {
    pub acquired_date: libc::time_t,
    pub start_date: libc::time_t,
    pub end_date: libc::time_t,
    pub is_trial: bool,
    pub trial_time_remaining_in_seconds: u32,
    pub quantity: u32,
    pub campaign_id: *const c_char,
    pub developer_offer_id: *const c_char,
}
#[repr(C)]
pub struct XStoreConsumableResult {
    pub quantity: u32,
}
#[repr(C)]
pub struct XStoreImage {
    pub uri: *const c_char,
    pub height: u32,
    pub width: u32,
    pub caption: *const c_char,
    pub image_purpose_tag: *const c_char,
}
#[repr(C)]
pub struct XStoreProduct {
    pub store_id: *const c_char,
    pub title: *const c_char,
    pub description: *const c_char,
    pub language: *const c_char,
    pub in_app_offer_token: *const c_char,
    pub link_uri: *mut c_char,
    pub product_kind: XStoreProductKind,
    pub price: XStorePrice,
    pub has_digital_download: bool,
    pub is_in_user_collection: bool,
    pub keywords_count: u32,
    pub keywords: *const *mut c_char,
    pub skus_count: u32,
    pub skus: *mut XStoreSku,
    pub images_count: u32,
    pub images: *mut XStoreImage,
    pub videos_count: u32,
    pub videos: *mut XStoreVideo,
}
#[repr(C)]
pub struct XStoreRateAndReviewResult {
    pub was_updated: bool,
}
#[repr(C)]
pub struct XStoreSku {
    pub sku_id: *const c_char,
    pub title: *const c_char,
    pub description: *const c_char,
    pub language: *const c_char,
    pub price: XStorePrice,
    pub is_trial: bool,
    pub is_in_user_collection: bool,
    pub collection_data: XStoreCollectionData,
    pub is_subscription: bool,
    pub subscription_info: XStoreSubscriptionInfo,
    pub bundled_skus_count: u32,
    pub bundled_skus: *const *mut c_char,
    pub images_count: u32,
    pub images: *mut XStoreImage,
    pub videos_count: u32,
    pub videos: *mut XStoreVideo,
    pub availabilities_count: u32,
    pub availabilities: *mut XStoreAvailability,
}
#[repr(C)]
pub struct XStoreSubscriptionInfo {
    pub has_trial_period: bool,
    pub trial_period_unit: XStoreDurationUnit,
    pub trial_period: u32,
    pub billing_period_unit: XStoreDurationUnit,
    pub billing_period: u32,
}
#[repr(C)]
pub struct XStoreVideo {
    pub uri: *const c_char,
    pub height: u32,
    pub width: u32,
    pub caption: *const c_char,
    pub video_purpose_tag: *const c_char,
    pub preview_image: XStoreImage,
}
#[repr(C)]
pub struct XSystemRuntimeInfo {
    pub runtime_version: XVersion,
    pub available_version: XVersion,
}

// XStoreGameLicenseChangedCallback
pub type XStoreGameLicenseChangedCallback = unsafe extern "system" fn(context: *mut c_void) -> ();
// XStorePackageLicenseLostCallback
pub type XStorePackageLicenseLostCallback = unsafe extern "system" fn(context: *mut c_void) -> ();
// XStoreProductQueryCallback
pub type XStoreProductQueryCallback =
    unsafe extern "system" fn(product: *const XStoreProduct, context: *mut c_void) -> bool;

pub struct XStorePackageUpdate {}
pub struct XStoreCanAcquireLicenseResult {}
pub struct XStoreAddonLicense {}

#[interface("2d42fea5-e71d-4b76-97cd-c50afbb3ae5d")]
pub unsafe trait IXStore: IUnknown {
    // XStoreCreateContext
    pub unsafe fn x_store_create_context(
        self: &Self,
        user: XUserHandle,
        store_context_handle: *mut XStoreContextHandle,
    ) -> HRESULT;
    // XStoreCloseContextHandle
    pub unsafe fn x_store_close_context_handle(
        self: &Self,
        store_context_handle: XStoreContextHandle,
    ) -> ();
    // XStoreQueryAssociatedProductsAsync
    pub unsafe fn x_store_query_associated_products_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        product_kinds: XStoreProductKind,
        max_items_to_retrieve_per_page: u32,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryAssociatedProductsResult
    pub unsafe fn x_store_query_associated_products_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT;
    // XStoreQueryProductsAsync
    pub unsafe fn x_store_query_products_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        product_kinds: XStoreProductKind,
        store_ids: *const *mut c_char,
        store_ids_count: usize,
        action_filters: *const *mut c_char,
        action_filters_count: usize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryProductsResult
    pub unsafe fn x_store_query_products_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT;
    // XStoreQueryEntitledProductsAsync
    pub unsafe fn x_store_query_entitled_products_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        product_kinds: XStoreProductKind,
        max_items_to_retrieve_per_page: u32,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryEntitledProductsResult
    pub unsafe fn x_store_query_entitled_products_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT;
    // XStoreQueryProductForCurrentGameAsync
    pub unsafe fn x_store_query_product_for_current_game_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryProductForCurrentGameResult
    pub unsafe fn x_store_query_product_for_current_game_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT;
    // XStoreQueryProductForPackageAsync
    pub unsafe fn x_store_query_product_for_package_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        product_kinds: XStoreProductKind,
        package_identifier: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryProductForPackageResult
    pub unsafe fn x_store_query_product_for_package_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT;

    // XStoreAcquireLicenseForDurablesAsync
    pub unsafe fn x_store_acquire_license_for_durables_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreAcquireLicenseForDurablesResult
    pub unsafe fn x_store_acquire_license_for_durables_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        store_license_handle: *mut XStoreLicenseHandle,
    ) -> HRESULT;
    // XStoreAcquireLicenseForPackageAsync
    pub unsafe fn x_store_acquire_license_for_package_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        package_identifier: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreAcquireLicenseForPackageResult
    pub unsafe fn x_store_acquire_license_for_package_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        store_license_handle: *mut XStoreLicenseHandle,
    ) -> HRESULT;
    // XStoreCanAcquireLicenseForPackageAsync
    pub unsafe fn x_store_can_acquire_license_for_package_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        package_identifier: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreCanAcquireLicenseForPackageResult
    pub unsafe fn x_store_can_acquire_license_for_package_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        store_can_acquire_license: *mut XStoreCanAcquireLicenseResult,
    ) -> HRESULT;
    // XStoreCanAcquireLicenseForStoreIdAsync
    pub unsafe fn x_store_can_acquire_license_for_store_id_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        store_product_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreCanAcquireLicenseForStoreIdResult
    pub unsafe fn x_store_can_acquire_license_for_store_id_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        store_can_acquire_license: *mut XStoreCanAcquireLicenseResult,
    ) -> HRESULT;
    // XStoreCloseLicenseHandle
    pub unsafe fn x_store_close_license_handle(
        self: &Self,
        store_license_handle: XStoreLicenseHandle,
    ) -> ();
    // XStoreCloseProductsQueryHandle
    pub unsafe fn x_store_close_products_query_handle(
        self: &Self,
        product_query_handle: XStoreProductQueryHandle,
    ) -> ();
    // XStoreDownloadAndInstallPackagesAsync
    pub unsafe fn x_store_download_and_install_packages_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        store_ids: *const *mut c_char,
        store_ids_count: usize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreDownloadAndInstallPackagesResultCount
    pub unsafe fn x_store_download_and_install_packages_result_count(
        self: &Self,
        async_: *mut XAsyncBlock,
        count: *mut u32,
    ) -> HRESULT;
    // XStoreDownloadAndInstallPackageUpdatesAsync
    pub unsafe fn x_store_download_and_install_package_updates_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        package_identifiers: *const *mut c_char,
        package_identifiers_count: usize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreDownloadAndInstallPackageUpdatesResult
    pub unsafe fn x_store_download_and_install_package_updates_result(
        self: &Self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreDownloadPackageUpdatesAsync
    pub unsafe fn x_store_download_package_updates_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        package_identifiers: *const *mut c_char,
        package_identifiers_count: usize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreDownloadPackageUpdatesResult
    pub unsafe fn x_store_download_package_updates_result(
        self: &Self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreEnumerateProductsQuery
    pub unsafe fn x_store_enumerate_products_query(
        self: &Self,
        product_query_handle: XStoreProductQueryHandle,
        context: *mut c_void,
        callback: Option<XStoreProductQueryCallback>,
    ) -> HRESULT;
    // XStoreGetUserCollectionsIdAsync
    pub unsafe fn x_store_get_user_collections_id_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        service_ticket: *const c_char,
        publisher_user_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreGetUserCollectionsIdResult
    pub unsafe fn x_store_get_user_collections_id_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        size: usize,
        result: *mut c_char,
    ) -> HRESULT;
    // XStoreGetUserCollectionsIdResultSize
    pub unsafe fn x_store_get_user_collections_id_result_size(
        self: &Self,
        async_: *mut XAsyncBlock,
        size: *mut usize,
    ) -> HRESULT;
    // XStoreGetUserPurchaseIdAsync
    pub unsafe fn x_store_get_user_purchase_id_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        service_ticket: *const c_char,
        publisher_user_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreGetUserPurchaseIdResult
    pub unsafe fn x_store_get_user_purchase_id_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        size: usize,
        result: *mut c_char,
    ) -> HRESULT;
    // XStoreGetUserPurchaseIdResultSize
    pub unsafe fn x_store_get_user_purchase_id_result_size(
        self: &Self,
        async_: *mut XAsyncBlock,
        size: *mut usize,
    ) -> HRESULT;
    // XStoreIsLicenseValid
    pub unsafe fn x_store_is_license_valid(
        self: &Self,
        store_license_handle: XStoreLicenseHandle,
    ) -> bool;
    // XStoreProductsQueryHasMorePages
    pub unsafe fn x_store_products_query_has_more_pages(
        self: &Self,
        product_query_handle: XStoreProductQueryHandle,
    ) -> bool;
    // XStoreProductsQueryNextPageAsync
    pub unsafe fn x_store_products_query_next_page_async(
        self: &Self,
        product_query_handle: XStoreProductQueryHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreProductsQueryNextPageResult
    pub unsafe fn x_store_products_query_next_page_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT;
    // XStoreQueryAddOnLicensesAsync
    pub unsafe fn x_store_query_add_on_licenses_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryAddOnLicensesResult
    pub unsafe fn x_store_query_add_on_licenses_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        count: u32,
        add_on_licenses: *mut XStoreAddonLicense,
    ) -> HRESULT;
    // XStoreQueryAddOnLicensesResultCount
    pub unsafe fn x_store_query_add_on_licenses_result_count(
        self: &Self,
        async_: *mut XAsyncBlock,
        count: *mut u32,
    ) -> HRESULT;
    // XStoreQueryAssociatedProductsForStoreIdAsync
    pub unsafe fn x_store_query_associated_products_for_store_id_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        product_kinds: XStoreProductKind,
        max_items_to_retrieve_per_page: u32,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryAssociatedProductsForStoreIdResult
    pub unsafe fn x_store_query_associated_products_for_store_id_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT;
    // XStoreQueryConsumableBalanceRemainingAsync
    pub unsafe fn x_store_query_consumable_balance_remaining_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        store_product_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryConsumableBalanceRemainingResult
    pub unsafe fn x_store_query_consumable_balance_remaining_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        consumable_result: *mut XStoreConsumableResult,
    ) -> HRESULT;
    // XStoreQueryGameAndDlcPackageUpdatesAsync
    pub unsafe fn x_store_query_game_and_dlc_package_updates_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryGameAndDlcPackageUpdatesResult
    pub unsafe fn x_store_query_game_and_dlc_package_updates_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        count: u32,
        package_updates: *mut XStorePackageUpdate,
    ) -> HRESULT;
    // XStoreQueryGameAndDlcPackageUpdatesResultCount
    pub unsafe fn x_store_query_game_and_dlc_package_updates_result_count(
        self: &Self,
        async_: *mut XAsyncBlock,
        count: *mut u32,
    ) -> HRESULT;
    // XStoreQueryGameLicenseAsync
    pub unsafe fn x_store_query_game_license_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryGameLicenseResult
    pub unsafe fn x_store_query_game_license_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        license: *mut XStoreGameLicense,
    ) -> HRESULT;
    // XStoreQueryLicenseTokenAsync
    pub unsafe fn x_store_query_license_token_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        product_ids: *const *mut c_char,
        product_ids_count: usize,
        custom_developer_string: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryLicenseTokenResult
    pub unsafe fn x_store_query_license_token_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        size: usize,
        result: *mut c_char,
    ) -> HRESULT;
    // XStoreQueryLicenseTokenResultSize
    pub unsafe fn x_store_query_license_token_result_size(
        self: &Self,
        async_: *mut XAsyncBlock,
        size: *mut usize,
    ) -> HRESULT;
    // XStoreQueryPackageIdentifier
    pub unsafe fn x_store_query_package_identifier(
        self: &Self,
        store_id: *const c_char,
        size: usize,
        package_identifier: *mut c_char,
    ) -> HRESULT;
    // XStoreQueryPackageUpdatesAsync
    pub unsafe fn x_store_query_package_updates_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        package_identifiers: *const *mut c_char,
        package_identifiers_count: usize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreQueryPackageUpdatesResult
    pub unsafe fn x_store_query_package_updates_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        count: u32,
        package_updates: *mut XStorePackageUpdate,
    ) -> HRESULT;
    // XStoreQueryPackageUpdatesResultCount
    pub unsafe fn x_store_query_package_updates_result_count(
        self: &Self,
        async_: *mut XAsyncBlock,
        count: *mut u32,
    ) -> HRESULT;
    // XStoreRegisterGameLicenseChanged
    pub unsafe fn x_store_register_game_license_changed(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: Option<XStoreGameLicenseChangedCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT;
    // XStoreRegisterPackageLicenseLost
    pub unsafe fn x_store_register_package_license_lost(
        self: &Self,
        license_handle: XStoreLicenseHandle,
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: Option<XStorePackageLicenseLostCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT;
    // XStoreReportConsumableFulfillmentAsync
    pub unsafe fn x_store_report_consumable_fulfillment_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        store_product_id: *const c_char,
        quantity: u32,
        tracking_id: GUID,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreReportConsumableFulfillmentResult
    pub unsafe fn x_store_report_consumable_fulfillment_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        consumable_result: *mut XStoreConsumableResult,
    ) -> HRESULT;
    // XStoreShowAssociatedProductsUIAsync
    pub unsafe fn x_store_show_associated_products_u_i_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        product_kinds: XStoreProductKind,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreShowAssociatedProductsUIResult
    pub unsafe fn x_store_show_associated_products_u_i_result(
        self: &Self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreShowGiftingUIAsync
    pub unsafe fn x_store_show_gifting_u_i_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        name: *const c_char,
        extended_json_data: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreShowGiftingUIResult
    pub unsafe fn x_store_show_gifting_u_i_result(self: &Self, async_: *mut XAsyncBlock)
    -> HRESULT;
    // XStoreShowProductPageUIAsync
    pub unsafe fn x_store_show_product_page_u_i_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreShowProductPageUIResult
    pub unsafe fn x_store_show_product_page_u_i_result(
        self: &Self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreShowPurchaseUIAsync
    pub unsafe fn x_store_show_purchase_u_i_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        name: *const c_char,
        extended_json_data: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreShowPurchaseUIResult
    pub unsafe fn x_store_show_purchase_u_i_result(
        self: &Self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreShowRateAndReviewUIAsync
    pub unsafe fn x_store_show_rate_and_review_u_i_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreShowRateAndReviewUIResult
    pub unsafe fn x_store_show_rate_and_review_u_i_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        result: *mut XStoreRateAndReviewResult,
    ) -> HRESULT;
    // XStoreShowRedeemTokenUIAsync
    pub unsafe fn x_store_show_redeem_token_u_i_async(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        token: *const c_char,
        allowed_store_ids: *const *mut c_char,
        allowed_store_ids_count: usize,
        disallow_csv_redemption: bool,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreShowRedeemTokenUIResult
    pub unsafe fn x_store_show_redeem_token_u_i_result(
        self: &Self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    // XStoreUnregisterGameLicenseChanged
    pub unsafe fn x_store_unregister_game_license_changed(
        self: &Self,
        store_context_handle: XStoreContextHandle,
        token: XTaskQueueRegistrationToken,
        wait: bool,
    ) -> bool;
    // XStoreUnregisterPackageLicenseLost
    pub unsafe fn x_store_unregister_package_license_lost(
        self: &Self,
        license_handle: XStoreLicenseHandle,
        token: XTaskQueueRegistrationToken,
        wait: bool,
    ) -> bool;
}

#[interface("5c48dedf-0b67-4492-a4b5-6829b8e796e1")]
pub unsafe trait IXStoreAlias1: IXStore {}

#[interface("b09d803c-2414-4a05-82c6-66dfdc9e9a44")]
pub unsafe trait IXStoreAlias2: IXStore {}

#[interface("0dd112ac-7c24-448c-b92b-3960fb5bd30c")]
pub unsafe trait IXStoreAlias3: IXStore {}

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
pub unsafe trait IXNetworking: IUnknown {
    // XNetworkingQueryPreferredLocalUdpMultiplayerPort
    pub unsafe fn x_networking_query_preferred_local_udp_multiplayer_port(
        self: &Self,
        preferred_local_udp_multiplayer_port: *mut u16,
    ) -> HRESULT;
    // XNetworkingQueryPreferredLocalUdpMultiplayerPortAsync
    pub unsafe fn x_networking_query_preferred_local_udp_multiplayer_port_async(
        self: &Self,
        async_block: *mut XAsyncBlock,
    ) -> HRESULT;
    // XNetworkingQueryPreferredLocalUdpMultiplayerPortAsyncResult
    pub unsafe fn x_networking_query_preferred_local_udp_multiplayer_port_async_result(
        self: &Self,
        async_block: *mut XAsyncBlock,
        preferred_local_udp_multiplayer_port: *mut u16,
    ) -> HRESULT;
    // XNetworkingRegisterPreferredLocalUdpMultiplayerPortChanged
    pub unsafe fn x_networking_register_preferred_local_udp_multiplayer_port_changed(
        self: &Self,
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: Option<XNetworkingPreferredLocalUdpMultiplayerPortChangedCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT;
    // XNetworkingUnregisterPreferredLocalUdpMultiplayerPortChanged
    pub unsafe fn x_networking_unregister_preferred_local_udp_multiplayer_port_changed(
        self: &Self,
        token: XTaskQueueRegistrationToken,
        wait: bool,
    ) -> bool;
    // XNetworkingGetConnectivityHint
    pub unsafe fn x_networking_get_connectivity_hint(
        self: &Self,
        connectivity_hint: *mut XNetworkingConnectivityHint,
    ) -> HRESULT;
    // XNetworkingQuerySecurityInformationForUrlAsync
    pub unsafe fn x_networking_query_security_information_for_url_async(
        self: &Self,
        url: *const c_char,
        async_block: *mut XAsyncBlock,
    ) -> HRESULT;
    // XNetworkingQuerySecurityInformationForUrlAsyncResult
    pub unsafe fn x_networking_query_security_information_for_url_async_result(
        self: &Self,
        async_block: *mut XAsyncBlock,
        security_information_buffer_byte_count: usize,
        security_information_buffer_byte_count_used: *mut usize,
        security_information_buffer: *mut u8,
        security_information: *mut *mut XNetworkingSecurityInformation,
    ) -> HRESULT;
    // XNetworkingQuerySecurityInformationForUrlAsyncResultSize
    pub unsafe fn x_networking_query_security_information_for_url_async_result_size(
        self: &Self,
        async_block: *mut XAsyncBlock,
        security_information_buffer_byte_count: *mut usize,
    ) -> HRESULT;
    // XNetworkingQuerySecurityInformationForUrlUtf16Async
    pub unsafe fn x_networking_query_security_information_for_url_utf16_async(
        self: &Self,
        url: *const u16,
        async_block: *mut XAsyncBlock,
    ) -> HRESULT;
    // XNetworkingQuerySecurityInformationForUrlUtf16AsyncResult
    pub unsafe fn x_networking_query_security_information_for_url_utf16_async_result(
        self: &Self,
        async_block: *mut XAsyncBlock,
        security_information_buffer_byte_count: usize,
        security_information_buffer_byte_count_used: *mut usize,
        security_information_buffer: *mut u8,
        security_information: *mut *mut XNetworkingSecurityInformation,
    ) -> HRESULT;
    // XNetworkingQuerySecurityInformationForUrlUtf16AsyncResultSize
    pub unsafe fn x_networking_query_security_information_for_url_utf16_async_result_size(
        self: &Self,
        async_block: *mut XAsyncBlock,
        security_information_buffer_byte_count: *mut usize,
    ) -> HRESULT;
    // XNetworkingRegisterConnectivityHintChanged
    pub unsafe fn x_networking_register_connectivity_hint_changed(
        self: &Self,
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: Option<XNetworkingConnectivityHintChangedCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT;
    // XNetworkingUnregisterConnectivityHintChanged
    pub unsafe fn x_networking_unregister_connectivity_hint_changed(
        self: &Self,
        token: XTaskQueueRegistrationToken,
        wait: bool,
    ) -> bool;
    // XNetworkingVerifyServerCertificate
    pub unsafe fn x_networking_verify_server_certificate(
        self: &Self,
        request_handle: *mut c_void,
        security_information: *const XNetworkingSecurityInformation,
    ) -> HRESULT;
    // XNetworkingQueryConfigurationSetting
    pub unsafe fn x_networking_query_configuration_setting(
        self: &Self,
        configuration_setting: XNetworkingConfigurationSetting,
        value: *mut u64,
    ) -> HRESULT;
    // XNetworkingSetConfigurationSetting
    pub unsafe fn x_networking_set_configuration_setting(
        self: &Self,
        configuration_parameter: XNetworkingConfigurationSetting,
        value: u64,
    ) -> HRESULT;
    // XNetworkingQueryStatistics
    pub unsafe fn x_networking_query_statistics(
        self: &Self,
        statistics_type: XNetworkingStatisticsType,
        statistics_buffer: *mut XNetworkingTcpQueuedReceivedBufferUsageStatistics,
    ) -> HRESULT;
}

#[interface("37e56907-2f10-41e8-b72f-36edb185331a")]
pub unsafe trait IXNetworking2: IXNetworking {}

#[implement(IXStore, IXStoreAlias1, IXStoreAlias2)]
pub struct XStoreObject;

impl IXStore_Impl for XStoreObject_Impl {
    unsafe fn x_store_acquire_license_for_durables_async(
        &self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_acquire_license_for_durables_result(
        &self,
        async_: *mut XAsyncBlock,
        store_license_handle: *mut XStoreLicenseHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_acquire_license_for_package_async(
        &self,
        store_context_handle: XStoreContextHandle,
        package_identifier: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_acquire_license_for_package_result(
        &self,
        async_: *mut XAsyncBlock,
        store_license_handle: *mut XStoreLicenseHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_can_acquire_license_for_package_async(
        &self,
        store_context_handle: XStoreContextHandle,
        package_identifier: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_can_acquire_license_for_package_result(
        &self,
        async_: *mut XAsyncBlock,
        store_can_acquire_license: *mut XStoreCanAcquireLicenseResult,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_can_acquire_license_for_store_id_async(
        &self,
        store_context_handle: XStoreContextHandle,
        store_product_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_can_acquire_license_for_store_id_result(
        &self,
        async_: *mut XAsyncBlock,
        store_can_acquire_license: *mut XStoreCanAcquireLicenseResult,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_close_context_handle(&self, store_context_handle: XStoreContextHandle) -> () {
        todo!()
    }

    unsafe fn x_store_close_license_handle(&self, store_license_handle: XStoreLicenseHandle) -> () {
        todo!()
    }

    unsafe fn x_store_close_products_query_handle(
        &self,
        product_query_handle: XStoreProductQueryHandle,
    ) -> () {
        todo!()
    }

    unsafe fn x_store_create_context(
        &self,
        user: XUserHandle,
        store_context_handle: *mut XStoreContextHandle,
    ) -> HRESULT {
        unsafe {
            *store_context_handle = 1;
        };
        HRESULT(0)
    }

    unsafe fn x_store_download_and_install_packages_async(
        &self,
        store_context_handle: XStoreContextHandle,
        store_ids: *const *mut c_char,
        store_ids_count: usize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_and_install_packages_result_count(
        &self,
        async_: *mut XAsyncBlock,
        count: *mut u32,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_and_install_package_updates_async(
        &self,
        store_context_handle: XStoreContextHandle,
        package_identifiers: *const *mut c_char,
        package_identifiers_count: usize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_and_install_package_updates_result(
        &self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_package_updates_async(
        &self,
        store_context_handle: XStoreContextHandle,
        package_identifiers: *const *mut c_char,
        package_identifiers_count: usize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_download_package_updates_result(&self, async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_enumerate_products_query(
        &self,
        product_query_handle: XStoreProductQueryHandle,
        context: *mut c_void,
        callback: Option<XStoreProductQueryCallback>,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_collections_id_async(
        &self,
        store_context_handle: XStoreContextHandle,
        service_ticket: *const c_char,
        publisher_user_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_collections_id_result(
        &self,
        async_: *mut XAsyncBlock,
        size: usize,
        result: *mut c_char,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_collections_id_result_size(
        &self,
        async_: *mut XAsyncBlock,
        size: *mut usize,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_purchase_id_async(
        &self,
        store_context_handle: XStoreContextHandle,
        service_ticket: *const c_char,
        publisher_user_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_purchase_id_result(
        &self,
        async_: *mut XAsyncBlock,
        size: usize,
        result: *mut c_char,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_get_user_purchase_id_result_size(
        &self,
        async_: *mut XAsyncBlock,
        size: *mut usize,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_is_license_valid(&self, store_license_handle: XStoreLicenseHandle) -> bool {
        todo!()
    }

    unsafe fn x_store_products_query_has_more_pages(
        &self,
        product_query_handle: XStoreProductQueryHandle,
    ) -> bool {
        todo!()
    }

    unsafe fn x_store_products_query_next_page_async(
        &self,
        product_query_handle: XStoreProductQueryHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_products_query_next_page_result(
        &self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_add_on_licenses_async(
        &self,
        store_context_handle: XStoreContextHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_add_on_licenses_result(
        &self,
        async_: *mut XAsyncBlock,
        count: u32,
        add_on_licenses: *mut XStoreAddonLicense,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_add_on_licenses_result_count(
        &self,
        async_: *mut XAsyncBlock,
        count: *mut u32,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_associated_products_async(
        &self,
        store_context_handle: XStoreContextHandle,
        product_kinds: XStoreProductKind,
        max_items_to_retrieve_per_page: u32,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_associated_products_for_store_id_async(
        &self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        product_kinds: XStoreProductKind,
        max_items_to_retrieve_per_page: u32,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_associated_products_for_store_id_result(
        &self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_associated_products_result(
        &self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_consumable_balance_remaining_async(
        &self,
        store_context_handle: XStoreContextHandle,
        store_product_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_consumable_balance_remaining_result(
        &self,
        async_: *mut XAsyncBlock,
        consumable_result: *mut XStoreConsumableResult,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_entitled_products_async(
        &self,
        store_context_handle: XStoreContextHandle,
        product_kinds: XStoreProductKind,
        max_items_to_retrieve_per_page: u32,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_entitled_products_result(
        &self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_game_and_dlc_package_updates_async(
        &self,
        store_context_handle: XStoreContextHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_game_and_dlc_package_updates_result(
        &self,
        async_: *mut XAsyncBlock,
        count: u32,
        package_updates: *mut XStorePackageUpdate,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_game_and_dlc_package_updates_result_count(
        &self,
        async_: *mut XAsyncBlock,
        count: *mut u32,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_game_license_async(
        &self,
        store_context_handle: XStoreContextHandle,
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

        let mut payload = XStoreQueryGameLicenseAsyncResultPayload {
            license: XStoreGameLicense::default(),
        };
        match unsafe { get_result(async_, null_mut(), &mut payload) } {
            Ok(_) => {
                unsafe {
                    *license = payload.license;
                }
                S_OK
            }
            Err(hr) => return hr,
        }
    }

    unsafe fn x_store_query_license_token_async(
        &self,
        store_context_handle: XStoreContextHandle,
        product_ids: *const *mut c_char,
        product_ids_count: usize,
        custom_developer_string: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_license_token_result(
        &self,
        async_: *mut XAsyncBlock,
        size: usize,
        result: *mut c_char,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_license_token_result_size(
        &self,
        async_: *mut XAsyncBlock,
        size: *mut usize,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_package_identifier(
        &self,
        store_id: *const c_char,
        size: usize,
        package_identifier: *mut c_char,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_package_updates_async(
        &self,
        store_context_handle: XStoreContextHandle,
        package_identifiers: *const *mut c_char,
        package_identifiers_count: usize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_package_updates_result(
        &self,
        async_: *mut XAsyncBlock,
        count: u32,
        package_updates: *mut XStorePackageUpdate,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_package_updates_result_count(
        &self,
        async_: *mut XAsyncBlock,
        count: *mut u32,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_product_for_current_game_async(
        &self,
        store_context_handle: XStoreContextHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_product_for_current_game_result(
        &self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_product_for_package_async(
        &self,
        store_context_handle: XStoreContextHandle,
        product_kinds: XStoreProductKind,
        package_identifier: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_product_for_package_result(
        &self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_products_async(
        &self,
        store_context_handle: XStoreContextHandle,
        product_kinds: XStoreProductKind,
        store_ids: *const *mut c_char,
        store_ids_count: usize,
        action_filters: *const *mut c_char,
        action_filters_count: usize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_query_products_result(
        &self,
        async_: *mut XAsyncBlock,
        product_query_handle: *mut XStoreProductQueryHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_register_game_license_changed(
        &self,
        store_context_handle: XStoreContextHandle,
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: Option<XStoreGameLicenseChangedCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_register_package_license_lost(
        &self,
        license_handle: XStoreLicenseHandle,
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: Option<XStorePackageLicenseLostCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_report_consumable_fulfillment_async(
        &self,
        store_context_handle: XStoreContextHandle,
        store_product_id: *const c_char,
        quantity: u32,
        tracking_id: GUID,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_report_consumable_fulfillment_result(
        &self,
        async_: *mut XAsyncBlock,
        consumable_result: *mut XStoreConsumableResult,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_associated_products_u_i_async(
        &self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        product_kinds: XStoreProductKind,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_associated_products_u_i_result(
        &self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_gifting_u_i_async(
        &self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        name: *const c_char,
        extended_json_data: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_gifting_u_i_result(&self, async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_product_page_u_i_async(
        &self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_product_page_u_i_result(&self, async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_purchase_u_i_async(
        &self,
        store_context_handle: XStoreContextHandle,
        store_id: *const c_char,
        name: *const c_char,
        extended_json_data: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_purchase_u_i_result(&self, async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_rate_and_review_u_i_async(
        &self,
        store_context_handle: XStoreContextHandle,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_rate_and_review_u_i_result(
        &self,
        async_: *mut XAsyncBlock,
        result: *mut XStoreRateAndReviewResult,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_redeem_token_u_i_async(
        &self,
        store_context_handle: XStoreContextHandle,
        token: *const c_char,
        allowed_store_ids: *const *mut c_char,
        allowed_store_ids_count: usize,
        disallow_csv_redemption: bool,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_show_redeem_token_u_i_result(&self, async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_store_unregister_game_license_changed(
        &self,
        store_context_handle: XStoreContextHandle,
        token: XTaskQueueRegistrationToken,
        wait: bool,
    ) -> bool {
        todo!()
    }

    unsafe fn x_store_unregister_package_license_lost(
        &self,
        license_handle: XStoreLicenseHandle,
        token: XTaskQueueRegistrationToken,
        wait: bool,
    ) -> bool {
        todo!()
    }
}

impl IXStoreAlias1_Impl for XStoreObject_Impl {}
impl IXStoreAlias2_Impl for XStoreObject_Impl {}
impl IXStoreAlias3_Impl for XStoreObject_Impl {}

#[implement(IXNetworking, IXNetworking2)]
pub struct XNetworkingObject;

#[repr(u32)]
pub enum XNetworkingConfigurationSetting {
    MaxTitleTcpQueuedReceiveBufferSize = 0,
    MaxSystemTcpQueuedReceiveBufferSize = 1,
    MaxToolsTcpQueuedReceiveBufferSize = 2,
}
#[repr(u32)]
pub enum XNetworkingConnectivityCostHint {
    Unknown = 0,
    Unrestricted = 1,
    Fixed = 2,
    Variable = 3,
}
#[repr(u32)]
pub enum XNetworkingConnectivityLevelHint {
    Unknown = 0,
    None = 1,
    LocalAccess = 2,
    InternetAccess = 3,
    ConstrainedInternetAccess = 4,
}
#[repr(u32)]
pub enum XNetworkingStatisticsType {
    TitleTcpQueuedReceivedBufferUsage = 0,
    SystemTcpQueuedReceivedBufferUsage = 1,
    ToolsTcpQueuedReceivedBufferUsage = 2,
}
#[repr(u32)]
pub enum XNetworkingThumbprintType {
    Leaf = 0,
    Issuer = 1,
    Root = 2,
}

#[repr(C)]
pub struct XNetworkingConnectivityHint {
    pub connectivity_level: XNetworkingConnectivityLevelHint,
    pub connectivity_cost: XNetworkingConnectivityCostHint,
    pub iana_interface_type: u32,
    pub network_initialized: bool,
    pub approaching_data_limit: bool,
    pub over_data_limit: bool,
    pub roaming: bool,
}
#[repr(C)]
pub struct XNetworkingSecurityInformation {
    pub enabled_http_security_protocol_flags: u32,
    pub thumbprint_count: usize,
    pub thumbprints: *mut XNetworkingThumbprint,
}
#[repr(C)]
pub struct XNetworkingTcpQueuedReceivedBufferUsageStatistics {
    pub num_bytes_currently_queued: u64,
    pub peak_num_bytes_ever_queued: u64,
    pub total_num_bytes_queued: u64,
    pub num_bytes_dropped_for_exceeding_configured_max: u64,
    pub num_bytes_dropped_due_to_any_failure: u64,
}
#[repr(C)]
pub struct XNetworkingThumbprint {
    pub thumbprint_type: XNetworkingThumbprintType,
    pub thumbprint_buffer_byte_count: usize,
    pub thumbprint_buffer: *mut u8,
}

type OnChanged =
    unsafe extern "system" fn(context: *mut c_void, hint: *const XNetworkingConnectivityHint);

impl IXNetworking_Impl for XNetworkingObject_Impl {
    unsafe fn x_networking_query_preferred_local_udp_multiplayer_port(
        &self,
        preferred_local_udp_multiplayer_port: *mut u16,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_query_preferred_local_udp_multiplayer_port_async(
        &self,
        async_block: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_query_preferred_local_udp_multiplayer_port_async_result(
        &self,
        async_block: *mut XAsyncBlock,
        preferred_local_udp_multiplayer_port: *mut u16,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_register_preferred_local_udp_multiplayer_port_changed(
        &self,
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: Option<XNetworkingPreferredLocalUdpMultiplayerPortChangedCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_unregister_preferred_local_udp_multiplayer_port_changed(
        &self,
        token: XTaskQueueRegistrationToken,
        wait: bool,
    ) -> bool {
        todo!()
    }

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
                network_initialized: true,
                approaching_data_limit: false,
                over_data_limit: false,
                roaming: false,
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
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: Option<XNetworkingConnectivityHintChangedCallback>,
        token: *mut XTaskQueueRegistrationToken,
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
                        network_initialized: true,
                        approaching_data_limit: false,
                        over_data_limit: false,
                        roaming: false,
                    },
                )
            };
        }
        S_OK
    }

    unsafe fn x_networking_unregister_connectivity_hint_changed(
        &self,
        token: XTaskQueueRegistrationToken,
        wait: bool,
    ) -> bool {
        todo!()
    }

    unsafe fn x_networking_verify_server_certificate(
        &self,
        request_handle: *mut c_void,
        security_information: *const XNetworkingSecurityInformation,
    ) -> HRESULT {
        S_OK
    }

    unsafe fn x_networking_query_configuration_setting(
        &self,
        configuration_setting: XNetworkingConfigurationSetting,
        value: *mut u64,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_set_configuration_setting(
        &self,
        configuration_parameter: XNetworkingConfigurationSetting,
        value: u64,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_networking_query_statistics(
        &self,
        statistics_type: XNetworkingStatisticsType,
        statistics_buffer: *mut XNetworkingTcpQueuedReceivedBufferUsageStatistics,
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
            unsafe {
                async_.x_task_queue_create(
                    threading::XTaskQueueDispatchMode::ThreadPool,
                    threading::XTaskQueueDispatchMode::ThreadPool,
                    &mut queue,
                )
            };
            unsafe { async_.x_task_queue_set_current_process_task_queue(queue) };
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
    use std::ffi::{c_char, c_void};
    use std::ptr::null_mut;

    use crate::com::{IXStore, XStoreGameLicense, get_result, query_api_impl};
    use crate::xasync::{XAsyncBlock, get_status, run};
    use crate::{
        E_FAIL, InitializeApiImplEx2, UninitializeApiImpl, set_delegated_dll_path_for_test,
    };
    use windows_core::{GUID, HRESULT, Interface};

    fn read_c_string(bytes: &[c_char]) -> String {
        let len = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        let raw: Vec<u8> = bytes[..len].iter().map(|byte| *byte as u8).collect();
        String::from_utf8(raw).expect("license string should be valid utf-8")
    }
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

        let uninit_hr = UninitializeApiImpl();
        assert_eq!(uninit_hr, HRESULT(0));
        set_delegated_dll_path_for_test(None);
    }
}
