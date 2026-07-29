use super::E_NOTIMPL;
use std::env::temp_dir;
use std::ffi::{CStr, c_char, c_void};
use std::mem::size_of;
use std::pin::Pin;
use std::ptr::null_mut;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use windows::minwindef::LPARAM;
use windows::windef::HWND;
use windows::winuser::{EnumWindows, MB_OK, MessageBoxW};
use windows_core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, Interface, implement, interface};
use windows_sys::core::BOOL;

const CLSID_XSTORE: GUID = GUID::from_u128(0x0dd112ac_7c24_448c_b92b_3960fb5bd30c);
const CLSID_XNETWORKING: GUID = GUID::from_u128(0x37e56907_2f10_41e8_b72f_36edb185331a);
const CLSID_XPERSISTENT_LOCAL_STORAGE: GUID =
    GUID::from_u128(0xf4faf4d4_2d04_4fce_b3e0_474a713a3e84);
const STORE_SKU_ID_SIZE: usize = 18;
const TRIAL_UNIQUE_ID_MAX_SIZE: usize = 64;

type XStoreContextHandle = u64;

use crate::xasync::{XAsyncBlock, get_result};
use crate::{E_FAIL, results::*, xasync};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XStoreGameLicense {
    pub skuStoreId: [c_char; STORE_SKU_ID_SIZE],
    pub isActive: bool,
    pub isTrialOwnedByThisUser: bool,
    pub isDiscLicense: bool,
    pub isTrial: bool,
    pub trialTimeRemainingInSeconds: u32,
    pub trialUniqueId: [c_char; TRIAL_UNIQUE_ID_MAX_SIZE],
    pub expirationDate: i64,
}

impl Default for XStoreGameLicense {
    fn default() -> Self {
        Self {
            skuStoreId: [0; STORE_SKU_ID_SIZE],
            isActive: false,
            isTrialOwnedByThisUser: false,
            isDiscLicense: false,
            isTrial: false,
            trialTimeRemainingInSeconds: 0,
            trialUniqueId: [0; TRIAL_UNIQUE_ID_MAX_SIZE],
            expirationDate: 0,
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
        isActive: true,
        isTrialOwnedByThisUser: true,
        isDiscLicense: false,
        isTrial: true,
        trialTimeRemainingInSeconds: 3600,
        expirationDate: 4_102_444_800,
        ..XStoreGameLicense::default()
    };
    write_c_string(&mut license.skuStoreId, b"TRIAL-SKU-001");
    write_c_string(&mut license.trialUniqueId, b"trial-license");
    license
}

#[repr(C)]
struct XStoreQueryGameLicenseAsyncResultPayload {
    license: XStoreGameLicense,
}

#[interface("8836fe87-edb9-4fe3-8dad-05f0d2cd5b40")]
pub unsafe trait IXFeature: IUnknown {
    unsafe fn XGameRuntimeIsFeatureAvailable(&self, feature: u32) -> bool;
}

#[implement(IXFeature)]
pub struct XFeature;

impl IXFeature_Impl for XFeature_Impl {
    unsafe fn XGameRuntimeIsFeatureAvailable(&self, feature: u32) -> bool {
        return true || feature != 10;
    }
}

#[repr(C)]
struct XPersistentLocalStorageSpaceInfo {
    availableFreeBytes: u64,
    totalFreeBytes: u64,
    usedBytes: u64,
    totalBytes: u64,
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
                availableFreeBytes: 1024 * 1024 * 1024,
                totalFreeBytes: 1024 * 1024 * 1024,
                usedBytes: 512 * 1024 * 1024,
                totalBytes: 2 * 1024 * 1024 * 1024,
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

#[interface("2d42fea5-e71d-4b76-97cd-c50afbb3ae5d")]
pub unsafe trait IXStore: IUnknown {
    unsafe fn XStoreCreateContext(&self, user: u64, storeContextHandle: *mut u64) -> HRESULT;
    unsafe fn XStoreCloseContextHandle(&self, storeContextHandle: u64) -> ();
    unsafe fn XStoreQueryAssociatedProductsAsync(
        &self,
        storeContextHandle: u64,
        productKinds: u64,
        maxItemsToRetrievePerPage: u32,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryAssociatedProductsResult(
        &self,
        async_: *mut c_void,
        productQueryHandle: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryProductsAsync(
        &self,
        storeContextHandle: u64,
        productKinds: u64,
        storeIds: *mut *mut c_char,
        storeIdsCount: u64,
        actionFilters: *mut *mut c_char,
        actionFiltersCount: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryProductsResult(
        &self,
        async_: *mut c_void,
        productQueryHandle: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryEntitledProductsAsync(
        &self,
        storeContextHandle: u64,
        productKinds: u64,
        maxItemsToRetrievePerPage: u32,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryEntitledProductsResult(
        &self,
        async_: *mut c_void,
        productQueryHandle: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryProductForCurrentGameAsync(
        &self,
        storeContextHandle: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryProductForCurrentGameResult(
        &self,
        async_: *mut c_void,
        productQueryHandle: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryProductForPackageAsync(
        &self,
        storeContextHandle: u64,
        productKinds: u64,
        packageIdentifier: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryProductForPackageResult(
        &self,
        async_: *mut c_void,
        productQueryHandle: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreEnumerateProductsQuery(
        &self,
        productQueryHandle: u64,
        context: *mut c_void,
        callback: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreProductsQueryHasMorePages(&self, productQueryHandle: u64) -> BOOL;
    unsafe fn XStoreProductsQueryNextPageAsync(
        &self,
        productQueryHandle: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreProductsQueryNextPageResult(
        &self,
        async_: *mut c_void,
        productQueryHandle: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreCloseProductsQueryHandle(&self, productQueryHandle: u64) -> ();
    unsafe fn XStoreAcquireLicenseForPackageAsync(
        &self,
        storeContextHandle: u64,
        packageIdentifier: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreAcquireLicenseForPackageResult(
        &self,
        async_: *mut c_void,
        storeLicenseHandle: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreIsLicenseValid(&self, storeLicenseHandle: u64) -> BOOL;
    unsafe fn XStoreCloseLicenseHandle(&self, storeLicenseHandle: u64) -> ();
    unsafe fn XStoreCanAcquireLicenseForStoreIdAsync(
        &self,
        storeContextHandle: u64,
        storeProductId: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreCanAcquireLicenseForStoreIdResult(
        &self,
        async_: *mut c_void,
        storeCanAcquireLicense: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreCanAcquireLicenseForPackageAsync(
        &self,
        storeContextHandle: u64,
        packageIdentifier: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreCanAcquireLicenseForPackageResult(
        &self,
        async_: *mut c_void,
        storeCanAcquireLicense: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryGameLicenseAsync(
        &self,
        storeContextHandle: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryGameLicenseResult(
        &self,
        async_: *mut c_void,
        license: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryAddOnLicensesAsync(
        &self,
        storeContextHandle: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryAddOnLicensesResultCount(
        &self,
        async_: *mut c_void,
        count: *mut u32,
    ) -> HRESULT;
    unsafe fn XStoreQueryAddOnLicensesResult(
        &self,
        async_: *mut c_void,
        count: u32,
        addOnLicenses: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryConsumableBalanceRemainingAsync(
        &self,
        storeContextHandle: u64,
        storeProductId: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryConsumableBalanceRemainingResult(
        &self,
        async_: *mut c_void,
        consumableResult: *mut c_void,
    ) -> HRESULT;
    unsafe fn __ReservedSlot35(&self) -> HRESULT;
    unsafe fn XStoreReportConsumableFulfillmentResult(
        &self,
        async_: *mut c_void,
        consumableResult: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreGetUserCollectionsIdAsync(
        &self,
        storeContextHandle: u64,
        serviceTicket: *mut c_char,
        publisherUserId: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreGetUserCollectionsIdResultSize(
        &self,
        async_: *mut c_void,
        size: *mut usize,
    ) -> HRESULT;
    unsafe fn XStoreGetUserCollectionsIdResult(
        &self,
        async_: *mut c_void,
        size: u64,
        result: *mut c_char,
    ) -> HRESULT;
    unsafe fn XStoreGetUserPurchaseIdAsync(
        &self,
        storeContextHandle: u64,
        serviceTicket: *mut c_char,
        publisherUserId: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreGetUserPurchaseIdResultSize(
        &self,
        async_: *mut c_void,
        size: *mut usize,
    ) -> HRESULT;
    unsafe fn XStoreGetUserPurchaseIdResult(
        &self,
        async_: *mut c_void,
        size: u64,
        result: *mut c_char,
    ) -> HRESULT;
    unsafe fn XStoreQueryLicenseTokenAsync(
        &self,
        storeContextHandle: u64,
        productIds: *mut *mut c_char,
        productIdsCount: u64,
        customDeveloperString: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryLicenseTokenResultSize(
        &self,
        async_: *mut c_void,
        size: *mut usize,
    ) -> HRESULT;
    unsafe fn XStoreQueryLicenseTokenResult(
        &self,
        async_: *mut c_void,
        size: u64,
        result: *mut c_char,
    ) -> HRESULT;
    unsafe fn __ReservedSlot46(&self) -> HRESULT;
    unsafe fn __ReservedSlot47(&self) -> HRESULT;
    unsafe fn __ReservedSlot48(&self) -> HRESULT;
    unsafe fn XStoreShowPurchaseUIAsync(
        &self,
        storeContextHandle: u64,
        storeId: *mut c_char,
        name: *mut c_char,
        extendedJsonData: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreShowPurchaseUIResult(&self, async_: *mut c_void) -> HRESULT;
    unsafe fn XStoreShowRateAndReviewUIAsync(
        &self,
        storeContextHandle: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreShowRateAndReviewUIResult(
        &self,
        async_: *mut c_void,
        result: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreShowRedeemTokenUIAsync(
        &self,
        storeContextHandle: u64,
        token: *mut c_char,
        allowedStoreIds: *mut *mut c_char,
        allowedStoreIdsCount: u64,
        disallowCsvRedemption: BOOL,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreShowRedeemTokenUIResult(&self, async_: *mut c_void) -> HRESULT;
    unsafe fn XStoreQueryGameAndDlcPackageUpdatesAsync(
        &self,
        storeContextHandle: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryGameAndDlcPackageUpdatesResultCount(
        &self,
        async_: *mut c_void,
        count: *mut u32,
    ) -> HRESULT;
    unsafe fn XStoreQueryGameAndDlcPackageUpdatesResult(
        &self,
        async_: *mut c_void,
        count: u32,
        packageUpdates: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreDownloadPackageUpdatesAsync(
        &self,
        storeContextHandle: u64,
        packageIdentifiers: *mut *mut c_char,
        packageIdentifiersCount: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreDownloadPackageUpdatesResult(&self, async_: *mut c_void) -> HRESULT;
    unsafe fn XStoreDownloadAndInstallPackageUpdatesAsync(
        &self,
        storeContextHandle: u64,
        packageIdentifiers: *mut *mut c_char,
        packageIdentifiersCount: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreDownloadAndInstallPackageUpdatesResult(&self, async_: *mut c_void) -> HRESULT;
    unsafe fn XStoreDownloadAndInstallPackagesAsync(
        &self,
        storeContextHandle: u64,
        storeIds: *mut *mut c_char,
        storeIdsCount: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreDownloadAndInstallPackagesResultCount(
        &self,
        async_: *mut c_void,
        count: *mut u32,
    ) -> HRESULT;
    unsafe fn XStoreDownloadAndInstallPackagesResult(
        &self,
        async_: *mut c_void,
        count: u32,
        packageIdentifiers: c_char,
    ) -> HRESULT;
    unsafe fn XStoreQueryPackageIdentifier(
        &self,
        storeId: *mut c_char,
        size: u64,
        packageIdentifier: *mut c_char,
    ) -> HRESULT;
    unsafe fn XStoreRegisterGameLicenseChanged(
        &self,
        storeContextHandle: u64,
        queue: u64,
        context: *mut c_void,
        callback: *mut c_void,
        token: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreUnregisterGameLicenseChanged(
        &self,
        storeContextHandle: u64,
        token: u64,
        wait: BOOL,
    ) -> BOOL;
    unsafe fn XStoreRegisterPackageLicenseLost(
        &self,
        licenseHandle: u64,
        queue: u64,
        context: *mut c_void,
        callback: *mut c_void,
        token: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreUnregisterPackageLicenseLost(
        &self,
        licenseHandle: u64,
        token: u64,
        wait: BOOL,
    ) -> BOOL;
    unsafe fn __ReservedSlot70(&self) -> HRESULT;
    unsafe fn XStoreAcquireLicenseForDurablesAsync(
        &self,
        storeContextHandle: u64,
        storeId: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreAcquireLicenseForDurablesResult(
        &self,
        async_: *mut c_void,
        storeLicenseHandle: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreShowAssociatedProductsUIAsync(
        &self,
        storeContextHandle: u64,
        storeId: *mut c_char,
        productKinds: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreShowAssociatedProductsUIResult(&self, async_: *mut c_void) -> HRESULT;
    unsafe fn XStoreShowProductPageUIAsync(
        &self,
        storeContextHandle: u64,
        storeId: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreShowProductPageUIResult(&self, async_: *mut c_void) -> HRESULT;
    unsafe fn XStoreQueryAssociatedProductsForStoreIdAsync(
        &self,
        storeContextHandle: u64,
        storeProductId: *mut c_char,
        productKinds: u64,
        maxItemsToRetrievePerPage: u32,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryAssociatedProductsForStoreIdResult(
        &self,
        async_: *mut c_void,
        productQueryHandle: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryPackageUpdatesAsync(
        &self,
        storeContextHandle: u64,
        packageIdentifiers: *mut *mut c_char,
        packageIdentifiersCount: u64,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreQueryPackageUpdatesResultCount(
        &self,
        async_: *mut c_void,
        count: *mut u32,
    ) -> HRESULT;
    unsafe fn XStoreQueryPackageUpdatesResult(
        &self,
        async_: *mut c_void,
        count: u32,
        packageUpdates: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreShowGiftingUIAsync(
        &self,
        storeContextHandle: u64,
        storeId: *mut c_char,
        name: *mut c_char,
        extendedJsonData: *mut c_char,
        async_: *mut c_void,
    ) -> HRESULT;
    unsafe fn XStoreShowGiftingUIResult(&self, async_: *mut c_void) -> HRESULT;
}

#[interface("5c48dedf-0b67-4492-a4b5-6829b8e796e1")]
pub unsafe trait IXStoreAlias1: IXStore {}

#[interface("b09d803c-2414-4a05-82c6-66dfdc9e9a44")]
pub unsafe trait IXStoreAlias2: IXStore {}

#[interface("0dd112ac-7c24-448c-b92b-3960fb5bd30c")]
pub unsafe trait IXStoreAlias3: IXStore {}

type XUserPlatformRemoteConnectShowPromptEventHandler = unsafe extern "system" fn(
    context: *const c_void,
    userIdentifier: u32,
    operation: u32,
    url: *const c_char,
    code: *const c_char,
    qrCodeSize: usize,
    qrCode: *const c_char,
);
type XUserPlatformRemoteConnectClosePromptEventHandler = unsafe extern "system" fn();

#[repr(C)]
pub struct XUserPlatformRemoteConnectEventHandlers {
    pub show: Option<XUserPlatformRemoteConnectShowPromptEventHandler>,
    pub close: Option<XUserPlatformRemoteConnectClosePromptEventHandler>,
    pub context: *mut c_void,
}

#[interface("26f3c674-a2fe-44fa-b6c4-a323bc94ff53")]
pub unsafe trait IXUserPlatform: IUnknown {
    // unsafe fn __reserved_slot_0(&self) -> HRESULT;
    // unsafe fn __reserved_slot_1(&self) -> HRESULT;
    // unsafe fn __reserved_slot_2(&self) -> HRESULT;
    unsafe fn __reserved_slot_3(&self) -> HRESULT;
    unsafe fn __reserved_slot_4(&self) -> HRESULT;
    unsafe fn __reserved_slot_5(&self) -> HRESULT;
    unsafe fn __reserved_slot_6(&self) -> HRESULT;
    unsafe fn __reserved_slot_7(&self) -> HRESULT;
    unsafe fn __reserved_slot_8(&self) -> HRESULT;
    unsafe fn __reserved_slot_9(&self) -> HRESULT;
    unsafe fn __reserved_slot_10(&self) -> HRESULT;
    unsafe fn __reserved_slot_11(&self) -> HRESULT;
    unsafe fn __reserved_slot_12(&self) -> HRESULT;
    unsafe fn __reserved_slot_13(&self) -> HRESULT;
    unsafe fn __reserved_slot_14(&self) -> HRESULT;
    unsafe fn __reserved_slot_15(&self) -> HRESULT;
    unsafe fn __reserved_slot_16(&self) -> HRESULT;
    unsafe fn __reserved_slot_17(&self) -> HRESULT;
    unsafe fn __reserved_slot_18(&self) -> HRESULT;
    unsafe fn __reserved_slot_19(&self) -> HRESULT;
    unsafe fn __reserved_slot_20(&self) -> HRESULT;
    unsafe fn __reserved_slot_21(&self) -> HRESULT;
    unsafe fn __reserved_slot_22(&self) -> HRESULT;
    unsafe fn __reserved_slot_23(&self) -> HRESULT;
    unsafe fn __reserved_slot_24(&self) -> HRESULT;
    unsafe fn __reserved_slot_25(&self) -> HRESULT;
    unsafe fn __reserved_slot_26(&self) -> HRESULT;
    unsafe fn __reserved_slot_27(&self) -> HRESULT;
    unsafe fn __reserved_slot_28(&self) -> HRESULT;
    unsafe fn __reserved_slot_29(&self) -> HRESULT;
    unsafe fn __reserved_slot_30(&self) -> HRESULT;
    unsafe fn __reserved_slot_31(&self) -> HRESULT;
    unsafe fn __reserved_slot_32(&self) -> HRESULT;
    unsafe fn __reserved_slot_33(&self) -> HRESULT;
    unsafe fn __reserved_slot_34(&self) -> HRESULT;
    unsafe fn __reserved_slot_35(&self) -> HRESULT;
    unsafe fn __reserved_slot_36(&self) -> HRESULT;
    unsafe fn __reserved_slot_37(&self) -> HRESULT;
    unsafe fn __reserved_slot_38(&self) -> HRESULT;
    unsafe fn __reserved_slot_39(&self) -> HRESULT;
    unsafe fn __reserved_slot_40(&self) -> HRESULT;
    unsafe fn __reserved_slot_41(&self) -> HRESULT;
    unsafe fn __reserved_slot_42(&self) -> HRESULT;
    pub unsafe fn XUserPlatformRemoteConnectSetEventHandlers(
        &self,
        queue: *mut c_void,
        handler: *const XUserPlatformRemoteConnectEventHandlers,
    ) -> HRESULT;
}

#[interface("bf2346b2-39af-4658-b5ea-44713c7e83b3")]
pub unsafe trait IXNetworking: IUnknown {
    unsafe fn XNetworkingQueryPreferredLocalUdpMultiplayerPort(
        &self,
        preferredLocalUdpMultiplayerPort: *mut u16,
    ) -> HRESULT;
    unsafe fn XNetworkingQueryPreferredLocalUdpMultiplayerPortAsync(
        &self,
        asyncBlock: *mut c_void,
    ) -> HRESULT;
    unsafe fn XNetworkingQueryPreferredLocalUdpMultiplayerPortAsyncResult(
        &self,
        asyncBlock: *mut c_void,
        preferredLocalUdpMultiplayerPort: *mut u16,
    ) -> HRESULT;
    unsafe fn XNetworkingRegisterPreferredLocalUdpMultiplayerPortChanged(
        &self,
        queue: u64,
        context: *mut c_void,
        callback: *mut c_void,
        token: *mut c_void,
    ) -> HRESULT;
    unsafe fn XNetworkingUnregisterPreferredLocalUdpMultiplayerPortChanged(
        &self,
        token: u64,
        wait: BOOL,
    ) -> BOOL;
    unsafe fn XNetworkingQuerySecurityInformationForUrlAsync(
        &self,
        url: *mut c_char,
        asyncBlock: *mut c_void,
    ) -> HRESULT;
    unsafe fn XNetworkingQuerySecurityInformationForUrlAsyncResultSize(
        &self,
        asyncBlock: *mut c_void,
        securityInformationBufferByteCount: *mut usize,
    ) -> HRESULT;
    unsafe fn XNetworkingQuerySecurityInformationForUrlAsyncResult(
        &self,
        asyncBlock: *mut c_void,
        securityInformationBufferByteCount: u64,
        securityInformationBufferByteCountUsed: *mut usize,
        securityInformationBuffer: *mut u8,
        securityInformation: *mut *mut c_void,
    ) -> HRESULT;
    unsafe fn XNetworkingQuerySecurityInformationForUrlUtf16Async(
        &self,
        url: *mut u16,
        asyncBlock: *mut c_void,
    ) -> HRESULT;
    unsafe fn XNetworkingQuerySecurityInformationForUrlUtf16AsyncResultSize(
        &self,
        asyncBlock: *mut c_void,
        securityInformationBufferByteCount: *mut usize,
    ) -> HRESULT;
    unsafe fn XNetworkingQuerySecurityInformationForUrlUtf16AsyncResult(
        &self,
        asyncBlock: *mut c_void,
        securityInformationBufferByteCount: u64,
        securityInformationBufferByteCountUsed: *mut usize,
        securityInformationBuffer: *mut u8,
        securityInformation: *mut *mut c_void,
    ) -> HRESULT;
    unsafe fn XNetworkingVerifyServerCertificate(
        &self,
        requestHandle: *mut c_void,
        securityInformation: *mut c_void,
    ) -> HRESULT;
    unsafe fn XNetworkingGetConnectivityHint(
        &self,
        connectivityHint: *mut XNetworkingConnectivityHint,
    ) -> HRESULT;
    unsafe fn XNetworkingRegisterConnectivityHintChanged(
        &self,
        queue: *mut c_void,
        context: *mut c_void,
        callback: Option<OnChanged>,
        token: *mut c_void,
    ) -> HRESULT;
    unsafe fn XNetworkingUnregisterConnectivityHintChanged(&self, token: u64, wait: BOOL) -> BOOL;
    unsafe fn XNetworkingQueryConfigurationSetting(
        &self,
        configurationSetting: u64,
        value: *mut u64,
    ) -> HRESULT;
    unsafe fn XNetworkingSetConfigurationSetting(
        &self,
        configurationSetting: u64,
        value: u64,
    ) -> HRESULT;
    unsafe fn XNetworkingQueryStatistics(
        &self,
        statisticsType: u64,
        statisticsBuffer: *mut c_void,
    ) -> HRESULT;
}

#[interface("37e56907-2f10-41e8-b72f-36edb185331a")]
pub unsafe trait IXNetworking2: IXNetworking {}

macro_rules! hresult_stub {
    ($(unsafe fn $name:ident (&self $(, $arg:ident : $ty:ty)*) -> HRESULT;)*) => {
        $(unsafe fn $name(&self $(, $arg: $ty)*) -> HRESULT { $(let _ = $arg;)* E_NOTIMPL })*
    };
}

macro_rules! hresult_stub_panic {
    ($(unsafe fn $name:ident (&self $(, $arg:ident : $ty:ty)*) -> HRESULT;)*) => {
        $(unsafe fn $name(&self $(, $arg: $ty)*) -> HRESULT { $(let _ = $arg;)* todo!("$name"); E_NOTIMPL })*
    };
}

macro_rules! bool_stub {
    ($(unsafe fn $name:ident (&self $(, $arg:ident : $ty:ty)*) -> BOOL;)*) => {
        $(unsafe fn $name(&self $(, $arg: $ty)*) -> BOOL { $(let _ = $arg;)* false.into() })*
    };
}

macro_rules! void_stub {
    ($(unsafe fn $name:ident (&self $(, $arg:ident : $ty:ty)*) -> ();)*) => {
        $(unsafe fn $name(&self $(, $arg: $ty)*) -> () { $(let _ = $arg;)* })*
    };
}

#[implement(IXStore, IXStoreAlias1, IXStoreAlias2)]
pub struct XStoreObject;

impl IXStore_Impl for XStoreObject_Impl {
    hresult_stub! {
        unsafe fn XStoreQueryAssociatedProductsAsync(&self, storeContextHandle: u64, productKinds: u64, maxItemsToRetrievePerPage: u32, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryAssociatedProductsResult(&self, async_: *mut c_void, productQueryHandle: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryProductsAsync(&self, storeContextHandle: u64, productKinds: u64, storeIds: *mut *mut c_char, storeIdsCount: u64, actionFilters: *mut *mut c_char, actionFiltersCount: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryProductsResult(&self, async_: *mut c_void, productQueryHandle: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryEntitledProductsAsync(&self, storeContextHandle: u64, productKinds: u64, maxItemsToRetrievePerPage: u32, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryEntitledProductsResult(&self, async_: *mut c_void, productQueryHandle: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryProductForCurrentGameAsync(&self, storeContextHandle: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryProductForCurrentGameResult(&self, async_: *mut c_void, productQueryHandle: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryProductForPackageAsync(&self, storeContextHandle: u64, productKinds: u64, packageIdentifier: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryProductForPackageResult(&self, async_: *mut c_void, productQueryHandle: *mut c_void) -> HRESULT;
        unsafe fn XStoreEnumerateProductsQuery(&self, productQueryHandle: u64, context: *mut c_void, callback: *mut c_void) -> HRESULT;
        unsafe fn XStoreProductsQueryNextPageAsync(&self, productQueryHandle: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreProductsQueryNextPageResult(&self, async_: *mut c_void, productQueryHandle: *mut c_void) -> HRESULT;
        unsafe fn XStoreAcquireLicenseForPackageAsync(&self, storeContextHandle: u64, packageIdentifier: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreAcquireLicenseForPackageResult(&self, async_: *mut c_void, storeLicenseHandle: *mut c_void) -> HRESULT;
        unsafe fn XStoreCanAcquireLicenseForStoreIdAsync(&self, storeContextHandle: u64, storeProductId: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreCanAcquireLicenseForStoreIdResult(&self, async_: *mut c_void, storeCanAcquireLicense: *mut c_void) -> HRESULT;
        unsafe fn XStoreCanAcquireLicenseForPackageAsync(&self, storeContextHandle: u64, packageIdentifier: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreCanAcquireLicenseForPackageResult(&self, async_: *mut c_void, storeCanAcquireLicense: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryAddOnLicensesAsync(&self, storeContextHandle: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryAddOnLicensesResultCount(&self, async_: *mut c_void, count: *mut u32) -> HRESULT;
        unsafe fn XStoreQueryAddOnLicensesResult(&self, async_: *mut c_void, count: u32, addOnLicenses: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryConsumableBalanceRemainingAsync(&self, storeContextHandle: u64, storeProductId: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryConsumableBalanceRemainingResult(&self, async_: *mut c_void, consumableResult: *mut c_void) -> HRESULT;
        unsafe fn __ReservedSlot35(&self) -> HRESULT;
        unsafe fn XStoreReportConsumableFulfillmentResult(&self, async_: *mut c_void, consumableResult: *mut c_void) -> HRESULT;
        unsafe fn XStoreGetUserCollectionsIdAsync(&self, storeContextHandle: u64, serviceTicket: *mut c_char, publisherUserId: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreGetUserCollectionsIdResultSize(&self, async_: *mut c_void, size: *mut usize) -> HRESULT;
        unsafe fn XStoreGetUserCollectionsIdResult(&self, async_: *mut c_void, size: u64, result: *mut c_char) -> HRESULT;
        unsafe fn XStoreGetUserPurchaseIdAsync(&self, storeContextHandle: u64, serviceTicket: *mut c_char, publisherUserId: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreGetUserPurchaseIdResultSize(&self, async_: *mut c_void, size: *mut usize) -> HRESULT;
        unsafe fn XStoreGetUserPurchaseIdResult(&self, async_: *mut c_void, size: u64, result: *mut c_char) -> HRESULT;
        unsafe fn XStoreQueryLicenseTokenAsync(&self, storeContextHandle: u64, productIds: *mut *mut c_char, productIdsCount: u64, customDeveloperString: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryLicenseTokenResultSize(&self, async_: *mut c_void, size: *mut usize) -> HRESULT;
        unsafe fn XStoreQueryLicenseTokenResult(&self, async_: *mut c_void, size: u64, result: *mut c_char) -> HRESULT;
        unsafe fn __ReservedSlot46(&self) -> HRESULT;
        unsafe fn __ReservedSlot47(&self) -> HRESULT;
        unsafe fn __ReservedSlot48(&self) -> HRESULT;
        unsafe fn XStoreShowPurchaseUIAsync(&self, storeContextHandle: u64, storeId: *mut c_char, name: *mut c_char, extendedJsonData: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowPurchaseUIResult(&self, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowRateAndReviewUIAsync(&self, storeContextHandle: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowRateAndReviewUIResult(&self, async_: *mut c_void, result: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowRedeemTokenUIAsync(&self, storeContextHandle: u64, token: *mut c_char, allowedStoreIds: *mut *mut c_char, allowedStoreIdsCount: u64, disallowCsvRedemption: BOOL, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowRedeemTokenUIResult(&self, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryGameAndDlcPackageUpdatesAsync(&self, storeContextHandle: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryGameAndDlcPackageUpdatesResultCount(&self, async_: *mut c_void, count: *mut u32) -> HRESULT;
        unsafe fn XStoreQueryGameAndDlcPackageUpdatesResult(&self, async_: *mut c_void, count: u32, packageUpdates: *mut c_void) -> HRESULT;
        unsafe fn XStoreDownloadPackageUpdatesAsync(&self, storeContextHandle: u64, packageIdentifiers: *mut *mut c_char, packageIdentifiersCount: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreDownloadPackageUpdatesResult(&self, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreDownloadAndInstallPackageUpdatesAsync(&self, storeContextHandle: u64, packageIdentifiers: *mut *mut c_char, packageIdentifiersCount: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreDownloadAndInstallPackageUpdatesResult(&self, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreDownloadAndInstallPackagesAsync(&self, storeContextHandle: u64, storeIds: *mut *mut c_char, storeIdsCount: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreDownloadAndInstallPackagesResultCount(&self, async_: *mut c_void, count: *mut u32) -> HRESULT;
        unsafe fn XStoreDownloadAndInstallPackagesResult(&self, async_: *mut c_void, count: u32, packageIdentifiers: c_char) -> HRESULT;
        unsafe fn XStoreQueryPackageIdentifier(&self, storeId: *mut c_char, size: u64, packageIdentifier: *mut c_char) -> HRESULT;
        unsafe fn XStoreRegisterGameLicenseChanged(&self, storeContextHandle: u64, queue: u64, context: *mut c_void, callback: *mut c_void, token: *mut c_void) -> HRESULT;
        unsafe fn XStoreRegisterPackageLicenseLost(&self, licenseHandle: u64, queue: u64, context: *mut c_void, callback: *mut c_void, token: *mut c_void) -> HRESULT;
        unsafe fn __ReservedSlot70(&self) -> HRESULT;
        unsafe fn XStoreAcquireLicenseForDurablesAsync(&self, storeContextHandle: u64, storeId: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreAcquireLicenseForDurablesResult(&self, async_: *mut c_void, storeLicenseHandle: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowAssociatedProductsUIAsync(&self, storeContextHandle: u64, storeId: *mut c_char, productKinds: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowAssociatedProductsUIResult(&self, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowProductPageUIAsync(&self, storeContextHandle: u64, storeId: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowProductPageUIResult(&self, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryAssociatedProductsForStoreIdAsync(&self, storeContextHandle: u64, storeProductId: *mut c_char, productKinds: u64, maxItemsToRetrievePerPage: u32, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryAssociatedProductsForStoreIdResult(&self, async_: *mut c_void, productQueryHandle: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryPackageUpdatesAsync(&self, storeContextHandle: u64, packageIdentifiers: *mut *mut c_char, packageIdentifiersCount: u64, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreQueryPackageUpdatesResultCount(&self, async_: *mut c_void, count: *mut u32) -> HRESULT;
        unsafe fn XStoreQueryPackageUpdatesResult(&self, async_: *mut c_void, count: u32, packageUpdates: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowGiftingUIAsync(&self, storeContextHandle: u64, storeId: *mut c_char, name: *mut c_char, extendedJsonData: *mut c_char, async_: *mut c_void) -> HRESULT;
        unsafe fn XStoreShowGiftingUIResult(&self, async_: *mut c_void) -> HRESULT;
    }
    bool_stub! {
        unsafe fn XStoreProductsQueryHasMorePages(&self, productQueryHandle: u64) -> BOOL;
        unsafe fn XStoreIsLicenseValid(&self, storeLicenseHandle: u64) -> BOOL;
        unsafe fn XStoreUnregisterGameLicenseChanged(&self, storeContextHandle: u64, token: u64, wait: BOOL) -> BOOL;
        unsafe fn XStoreUnregisterPackageLicenseLost(&self, licenseHandle: u64, token: u64, wait: BOOL) -> BOOL;
    }
    void_stub! {
        unsafe fn XStoreCloseContextHandle(&self, storeContextHandle: u64) -> ();
        unsafe fn XStoreCloseProductsQueryHandle(&self, productQueryHandle: u64) -> ();
        unsafe fn XStoreCloseLicenseHandle(&self, storeLicenseHandle: u64) -> ();
    }

    unsafe fn XStoreCreateContext(&self, _user: u64, storeContextHandle: *mut u64) -> HRESULT {
        unsafe {
            *storeContextHandle = 1;
        };
        HRESULT(0)
    }

    unsafe fn XStoreQueryGameLicenseAsync(
        &self,
        storeContextHandle: u64,
        async_: *mut c_void,
    ) -> HRESULT {
        if storeContextHandle == 0 {
            return E_POINTER;
        }
        unsafe {
            xasync::run_sync(async_.cast(), move || {
                // println!("storeContextHandle: {storeContextHandle}");
                return Ok(XStoreGameLicense::default());
            })
        }
    }

    unsafe fn XStoreQueryGameLicenseResult(
        &self,
        async_: *mut c_void,
        license: *mut c_void,
    ) -> HRESULT {
        // println!("XStoreQueryGameLicenseResult");
        if async_.is_null() || license.is_null() {
            return E_POINTER;
        }

        let mut payload = XStoreQueryGameLicenseAsyncResultPayload {
            license: XStoreGameLicense::default(),
        };
        match unsafe { get_result(async_.cast(), null_mut(), &mut payload) } {
            Ok(_) => {
                unsafe {
                    *(license as *mut XStoreGameLicense) = payload.license;
                }
                S_OK
            }
            Err(hr) => return hr,
        }
    }
}

impl IXStoreAlias1_Impl for XStoreObject_Impl {}
impl IXStoreAlias2_Impl for XStoreObject_Impl {}
impl IXStoreAlias3_Impl for XStoreObject_Impl {}

#[implement(IXNetworking, IXNetworking2)]
pub struct XNetworkingObject;

#[repr(u32)]
enum XNetworkingConnectivityCostHint {
    Unknown = 0,
    Unrestricted = 1,
    Fixed = 2,
    Variable = 3,
}
#[repr(u32)]
enum XNetworkingConnectivityLevelHint {
    Unknown = 0,
    None = 1,
    LocalAccess = 2,
    InternetAccess = 3,
    ConstrainedInternetAccess = 4,
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
    enabledHttpSecurityProtocolFlags: u32,
    thumbprintCount: usize,
    thumbprints: *const c_void,
}

type OnChanged =
    unsafe extern "system" fn(context: *mut c_void, hint: *const XNetworkingConnectivityHint);

impl IXNetworking_Impl for XNetworkingObject_Impl {
    hresult_stub_panic! {
        unsafe fn XNetworkingQueryPreferredLocalUdpMultiplayerPort(&self, preferredLocalUdpMultiplayerPort: *mut u16) -> HRESULT;
        unsafe fn XNetworkingQueryPreferredLocalUdpMultiplayerPortAsync(&self, asyncBlock: *mut c_void) -> HRESULT;
        unsafe fn XNetworkingQueryPreferredLocalUdpMultiplayerPortAsyncResult(&self, asyncBlock: *mut c_void, preferredLocalUdpMultiplayerPort: *mut u16) -> HRESULT;
        unsafe fn XNetworkingRegisterPreferredLocalUdpMultiplayerPortChanged(&self, queue: u64, context: *mut c_void, callback: *mut c_void, token: *mut c_void) -> HRESULT;
        unsafe fn XNetworkingQueryConfigurationSetting(&self, configurationSetting: u64, value: *mut u64) -> HRESULT;
        unsafe fn XNetworkingSetConfigurationSetting(&self, configurationSetting: u64, value: u64) -> HRESULT;
        unsafe fn XNetworkingQueryStatistics(&self, statisticsType: u64, statisticsBuffer: *mut c_void) -> HRESULT;
    }
    bool_stub! {
        unsafe fn XNetworkingUnregisterPreferredLocalUdpMultiplayerPortChanged(&self, token: u64, wait: BOOL) -> BOOL;
        unsafe fn XNetworkingUnregisterConnectivityHintChanged(&self, token: u64, wait: BOOL) -> BOOL;
    }

    unsafe fn XNetworkingGetConnectivityHint(
        &self,
        connectivityHint: *mut XNetworkingConnectivityHint,
    ) -> HRESULT {
        if connectivityHint.is_null() {
            return E_POINTER;
        }
        unsafe {
            *connectivityHint = XNetworkingConnectivityHint {
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

    unsafe fn XNetworkingVerifyServerCertificate(
        &self,
        requestHandle: *mut c_void,
        securityInformation: *mut c_void,
    ) -> HRESULT {
        S_OK
    }

    unsafe fn XNetworkingRegisterConnectivityHintChanged(
        &self,
        queue: *mut c_void,
        context: *mut c_void,
        callback: Option<OnChanged>,
        token: *mut c_void,
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

    unsafe fn XNetworkingQuerySecurityInformationForUrlAsync(
        &self,
        url: *mut c_char,
        asyncBlock: *mut c_void,
    ) -> HRESULT {
        let url = unsafe { CStr::from_ptr(url) };
        // println!("XNetworkingQuerySecurityInformationForUrlAsync {}", url.to_string_lossy());
        unsafe {
            xasync::run_sync(asyncBlock.cast(), move || {
                Ok(XNetworkingSecurityInformation {
                    enabledHttpSecurityProtocolFlags: 0x00000080
                        | 0x00000200
                        | 0x00000800
                        | 0x00002000,
                    thumbprintCount: 0,
                    thumbprints: null_mut(),
                })
            })
        }
    }

    unsafe fn XNetworkingQuerySecurityInformationForUrlAsyncResultSize(
        &self,
        asyncBlock: *mut c_void,
        securityInformationBufferByteCount: *mut usize,
    ) -> HRESULT {
        let r = unsafe { xasync::get_result_size(asyncBlock.cast()) };
        match r {
            Ok(size) => unsafe {
                *securityInformationBufferByteCount = size;
                // println!("XNetworkingQuerySecurityInformationForUrlAsyncResultSize: OK");
                S_OK
            },
            Err(hr) => hr,
        }
    }

    unsafe fn XNetworkingQuerySecurityInformationForUrlAsyncResult(
        &self,
        asyncBlock: *mut c_void,
        securityInformationBufferByteCount: u64,
        securityInformationBufferByteCountUsed: *mut usize,
        securityInformationBuffer: *mut u8,
        securityInformation: *mut *mut c_void,
    ) -> HRESULT {
        if securityInformationBufferByteCount < size_of::<XNetworkingSecurityInformation>() as u64 {
            return E_FAIL;
        }
        if !securityInformationBufferByteCountUsed.is_null() {
            unsafe { *securityInformationBufferByteCountUsed = 0 };
        }
        match unsafe {
            get_result(
                asyncBlock.cast(),
                null_mut(),
                securityInformationBuffer.cast::<XNetworkingSecurityInformation>(),
            )
        } {
            Ok(_) => {
                if !securityInformationBufferByteCountUsed.is_null() {
                    unsafe {
                        *securityInformationBufferByteCountUsed =
                            size_of::<XNetworkingSecurityInformation>()
                    };
                }
                unsafe { *securityInformation = securityInformationBuffer.cast() };
                S_OK
            }
            Err(hr) => hr,
        }
    }

    unsafe fn XNetworkingQuerySecurityInformationForUrlUtf16Async(
        &self,
        url: *mut u16,
        asyncBlock: *mut c_void,
    ) -> HRESULT {
        unsafe {
            xasync::run_sync(asyncBlock.cast(), move || {
                Ok(XNetworkingSecurityInformation {
                    enabledHttpSecurityProtocolFlags: 0x00000080
                        | 0x00000200
                        | 0x00000800
                        | 0x00002000,
                    thumbprintCount: 0,
                    thumbprints: null_mut(),
                })
            })
        }
    }

    unsafe fn XNetworkingQuerySecurityInformationForUrlUtf16AsyncResultSize(
        &self,
        asyncBlock: *mut c_void,
        securityInformationBufferByteCount: *mut usize,
    ) -> HRESULT {
        let r = unsafe { xasync::get_result_size(asyncBlock.cast()) };
        match r {
            Ok(size) => unsafe {
                *securityInformationBufferByteCount = size;
                S_OK
            },
            Err(hr) => hr,
        }
    }

    unsafe fn XNetworkingQuerySecurityInformationForUrlUtf16AsyncResult(
        &self,
        asyncBlock: *mut c_void,
        securityInformationBufferByteCount: u64,
        securityInformationBufferByteCountUsed: *mut usize,
        securityInformationBuffer: *mut u8,
        securityInformation: *mut *mut c_void,
    ) -> HRESULT {
        if securityInformationBufferByteCount < size_of::<XNetworkingSecurityInformation>() as u64 {
            return E_FAIL;
        }
        if !securityInformationBufferByteCountUsed.is_null() {
            unsafe { *securityInformationBufferByteCountUsed = 0 };
        }
        match unsafe {
            get_result(
                asyncBlock.cast(),
                null_mut(),
                securityInformationBuffer.cast::<XNetworkingSecurityInformation>(),
            )
        } {
            Ok(_) => {
                if !securityInformationBufferByteCountUsed.is_null() {
                    unsafe {
                        *securityInformationBufferByteCountUsed =
                            size_of::<XNetworkingSecurityInformation>()
                    };
                }
                unsafe { *securityInformation = securityInformationBuffer.cast() };
                S_OK
            }
            Err(hr) => hr,
        }
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
        _ => crate::delegated_query_api_impl(runtime_class_id, interface_id, out),
    };
    res
}

#[cfg(test)]
mod tests {
    use std::ffi::{c_char, c_void};
    use std::ptr::null;

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
            let hr = store.XStoreCreateContext(0, &mut store_ctx);
            assert_eq!(hr, HRESULT(0));
            let hr = store.XStoreQueryGameLicenseAsync(store_ctx, std::ptr::null_mut());
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
        let hr = unsafe { store.XStoreCreateContext(0, &mut store_ctx) };
        assert_eq!(hr, HRESULT(0));

        let mut async_block = XAsyncBlock {
            queue: std::ptr::null_mut(),
            context: std::ptr::null_mut(),
            callback: None,
            internal: [0; std::mem::size_of::<*mut c_void>() * 4],
        };
        let hr = unsafe {
            store.XStoreQueryGameLicenseAsync(
                store_ctx,
                (&mut async_block as *mut XAsyncBlock).cast(),
            )
        };
        assert_eq!(hr, HRESULT(0));

        let status_hr = unsafe { get_status(&mut async_block, true) };
        assert_eq!(status_hr, HRESULT(0));

        let mut license = XStoreGameLicense::default();
        let result_hr = unsafe {
            store.XStoreQueryGameLicenseResult(
                (&mut async_block as *mut XAsyncBlock).cast(),
                (&mut license as *mut XStoreGameLicense).cast(),
            )
        };
        assert_eq!(result_hr, HRESULT(0));
        // assert_eq!(read_c_string(&license.skuStoreId), "TRIAL-SKU-001");
        assert!(license.isActive);
        assert!(!license.isTrialOwnedByThisUser);
        assert!(!license.isTrial);
        assert!(!license.isDiscLicense);
        assert_eq!(license.trialTimeRemainingInSeconds, 0);
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

        let status_hr = unsafe { get_status(&mut async_block, true) };
        assert_eq!(status_hr, HRESULT(0));

        let mut payload: Payload = Payload {
            v: 0,
            v2: 0,
            v3: GUID::zeroed(),
        };
        let hr = unsafe { get_result(&mut async_block, null(), &mut payload) };
        assert_eq!(hr, HRESULT(0));

        println!("res {:?}", payload);

        let uninit_hr = UninitializeApiImpl();
        assert_eq!(uninit_hr, HRESULT(0));
        set_delegated_dll_path_for_test(None);
    }
}
