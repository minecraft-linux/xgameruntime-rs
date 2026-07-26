use super::E_NOTIMPL;
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
const STORE_SKU_ID_SIZE: usize = 18;
const TRIAL_UNIQUE_ID_MAX_SIZE: usize = 64;

type XStoreContextHandle = u64;

use crate::xasync::{XAsyncBlock, get_result};
use crate::{results::*, xasync};

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

pub type XGameUiCallbackHandle = u64;
pub type XTaskQueueHandle = u64;

// pub unsafe fn x_game_ui_show_player_profile_card_ui_callback (callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, requesting_user: XUserHandle, target_player: u64, context: *mut c_void);
// pub unsafe fn x_game_ui_show_player_picker_ui_callback (callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, info: *mut XGameUiPlayerPickerInfo, context: *mut c_void);
// pub unsafe fn x_game_ui_show_send_game_invite_ui_callback (callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, requesting_user: XUserHandle, session_configuration_id: *const c_char, session_template_name: *const c_char, session_id: *const c_char, invitation_text: *const c_char, custom_activation_context: *const c_char, context: *mut c_void);
// pub unsafe fn x_game_ui_show_achievements_ui_callback (callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, requesting_user: XUserHandle, title_id: u32, context: *mut c_void);
// pub unsafe fn x_game_ui_show_multiplayer_activity_game_invite_ui_callback (callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, requesting_user: XUserHandle, context: *mut c_void);
// pub unsafe fn x_game_ui_show_error_dialog_ui_callback (callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, error_code: HRESULT, service_context: *const c_char, context: *mut c_void);
// pub unsafe fn x_game_ui_show_text_entry_ui_callback (callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, title_text: *const c_char, description_text: *const c_char, default_text: *const c_char, input_scope: XGameUiTextEntryInputScope, max_text_length: u32, context: *mut c_void);

#[repr(C)]
pub struct XGameUiWebAuthenticationResultData {
responseStatus: HRESULT,
responseCompletionUriSize: usize,
responseCompletionUri: *const c_char,
}
#[repr(C)]
pub struct XGameUiTextEntryExtents {
left: f32,
top: f32,
right: f32,
bottom: f32,
}
#[repr(C)]
pub struct XGameUiTextEntryOptions {
inputScope: XGameUiTextEntryInputScope,
positionHint: XGameUiTextEntryPositionHint,
visibilityflags: XGameUiTextEntryVisibilityFlags,
}
#[repr(C)]
pub struct XGameUiPlayerPickerInfo {
pub requestingUser: XUserHandle,
pub promptText: *const c_char,
pub selectFromPlayersCount: u32,
pub selectFromPlayers: *const u64,
pub preSelectedPlayersCount: u32,
pub preSelectedPlayers: *const u64,
pub minSelectionCount: u32,
pub maxSelectionCount: u32,
}

pub type XGameUiShowPlayerProfileCardUiCallback = unsafe extern "system" fn(callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, requesting_user: XUserHandle, target_player: u64, context: *mut c_void);
pub type XGameUiShowPlayerPickerUiCallback = unsafe extern "system" fn(callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, info: *mut XGameUiPlayerPickerInfo, context: *mut c_void);
pub type XGameUiShowSendGameInviteUiCallback = unsafe extern "system" fn(callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, requesting_user: XUserHandle, session_configuration_id: *const c_char, session_template_name: *const c_char, session_id: *const c_char, invitation_text: *const c_char, custom_activation_context: *const c_char, context: *mut c_void);
pub type XGameUiShowAchievementsUiCallback = unsafe extern "system" fn(callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, requesting_user: XUserHandle, title_id: u32, context: *mut c_void);
pub type XGameUiShowMultiplayerActivityGameInviteUiCallback = unsafe extern "system" fn(callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, requesting_user: XUserHandle, title_id: u32, context: *mut c_void);
pub type XGameUiShowMessageDialogUiCallback = unsafe extern "system" fn(callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, requesting_user: XUserHandle, context: *mut c_void);
pub type XGameUiShowErrorDialogUiCallback = unsafe extern "system" fn(callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, error_code: HRESULT, service_context: *const c_char, context: *mut c_void);
pub type XGameUiShowTextEntryUiCallback = unsafe extern "system" fn(callback_handle: XGameUiCallbackHandle, queue: XTaskQueueHandle, title_text: *const c_char, description_text: *const c_char, default_text: *const c_char, input_scope: XGameUiTextEntryInputScope, max_text_length: u32, context: *mut c_void);

pub struct XGameUiUiCallbacks {
    pub context: *mut c_void,
    pub show_player_profile_card_callback: Option<XGameUiShowPlayerProfileCardUiCallback>,
    pub show_player_picker_callback: Option<XGameUiShowPlayerPickerUiCallback>,
    pub show_send_game_invite_callback: Option<XGameUiShowSendGameInviteUiCallback>,
    pub show_achievements_callback: Option<XGameUiShowAchievementsUiCallback>,
    pub show_multiplayer_activity_game_invite_callback: Option<XGameUiShowMultiplayerActivityGameInviteUiCallback>,
    pub show_message_dialog_callback: Option<XGameUiShowMessageDialogUiCallback>,
    pub show_error_dialog_callback: Option<XGameUiShowErrorDialogUiCallback>,
    pub show_text_entry_callback: Option<XGameUiShowTextEntryUiCallback>,
}

pub type XUserHandle = u64;


enum XGameUiMessageDialogButton {
    First = 0,
    Second = 1,
    Third = 2
}
#[repr(u32)]
enum XGameUiNotificationPositionHint {
    BottomCenter = 0,
    BottomLeft = 1,
    BottomRight = 2,
    TopCenter = 3,
    TopLeft = 4,
    TopRight = 5
}
#[repr(u32)]
pub enum XGameUiTextEntryChangeTypeFlags {
    None = 0x0,
    TextChanged = 0x1,
    Dismissed = 0x2
}
#[repr(u32)]
pub enum XGameUiTextEntryInputScope {
    Default = 0,
    Url = 1,
    EmailSmtpAddress = 5,
    Number = 29,
    Password = 31,
    TelephoneNumber = 32,
    Alphanumeric = 40,
    Search = 50,
    ChatWithoutEmoji = 68
}
#[repr(u32)]
pub enum XGameUiTextEntryPositionHint {
    Bottom = 0,
    Top = 1
}
#[repr(u32)]
pub enum XGameUiTextEntryVisibilityFlags {
    Default = 0x0,
    OnlyShowCandidates = 0x1
}
#[repr(u32)]
pub enum XGameUiWebAuthenticationOptions {
    None = 0x00,
    PreferFullscreen = 0x01
}

#[interface("eaf669df-5542-4590-99a3-8dc061f837cc")]
pub unsafe trait IXGameUi : IUnknown {
    pub unsafe fn x_game_ui_show_message_dialog_async (self: &Self, async_: *mut XAsyncBlock, title_text: *const c_char, content_text: *const c_char, first_button_text: *const c_char, second_button_text: *const c_char, third_button_text: *const c_char, default_button: XGameUiMessageDialogButton, cancel_button: XGameUiMessageDialogButton) -> HRESULT;
    pub unsafe fn x_game_ui_show_message_dialog_result (self: &Self, async_: *mut XAsyncBlock, result_button: *mut XGameUiMessageDialogButton) -> HRESULT;
    pub unsafe fn x_game_ui_show_send_game_invite_async (self: &Self, async_: *mut XAsyncBlock, requesting_user: XUserHandle, session_configuration_id: *const c_char, session_template_name: *const c_char, session_id: *const c_char, invitation_text: *const c_char, custom_activation_context: *const c_char) -> HRESULT;
    pub unsafe fn x_game_ui_show_send_game_invite_result (self: &Self, async_: *mut XAsyncBlock) -> HRESULT;
    pub unsafe fn x_game_ui_show_player_profile_card_async (self: &Self, async_: *mut XAsyncBlock, requesting_user: XUserHandle, target_player: u64) -> HRESULT;
    pub unsafe fn x_game_ui_show_player_profile_card_result (self: &Self, async_: *mut XAsyncBlock) -> HRESULT;
    pub unsafe fn x_game_ui_show_achievements_async (self: &Self, async_: *mut XAsyncBlock, requesting_user: XUserHandle, title_id: u32) -> HRESULT;
    pub unsafe fn x_game_ui_show_achievements_result (self: &Self, async_: *mut XAsyncBlock) -> HRESULT;
    pub unsafe fn x_game_ui_show_player_picker_async (self: &Self, async_: *mut XAsyncBlock, requesting_user: XUserHandle, prompt_text: *const c_char, select_from_players_count: u32, select_from_players: *const u64, pre_selected_players_count: u32, pre_selected_players: *const u64, min_selection_count: u32, max_selection_count: u32) -> HRESULT;
    pub unsafe fn x_game_ui_show_player_picker_result_count (self: &Self, async_: *mut XAsyncBlock, result_players_count: *mut u32) -> HRESULT;
    pub unsafe fn x_game_ui_show_player_picker_result (self: &Self, async_: *mut XAsyncBlock, result_players_count: u32, result_players: *mut u64, result_players_used: *mut u32) -> HRESULT;
    pub unsafe fn x_game_ui_show_error_dialog_async (self: &Self, async_: *mut XAsyncBlock, error_code: HRESULT, context: *const c_char) -> HRESULT;
    pub unsafe fn x_game_ui_show_error_dialog_result (self: &Self, async_: *mut XAsyncBlock) -> HRESULT;
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
    // pub unsafe fn x_game_ui_set_ui_callbacks (self: &Self, callbacks: *const XGameUiUiCallbacks, use_system_ui_if_available: bool);
    pub unsafe fn xgame_ui_set_ui_callbacks(
        &self,
        callbacks: *const XGameUiUiCallbacks,
        use_system_ui_if_available: bool,
    ) -> HRESULT;
    pub unsafe fn x_game_ui_set_message_dialog_ui_response (self: &Self, callback_handle: XGameUiCallbackHandle, response: XGameUiMessageDialogButton) -> HRESULT;
    pub unsafe fn x_game_ui_set_player_picker_ui_response (self: &Self, callback_handle: XGameUiCallbackHandle, player_count: u32, players: *const u64) -> HRESULT;
    pub unsafe fn x_game_ui_set_text_entry_ui_response (self: &Self, callback_handle: XGameUiCallbackHandle, response: *const c_char) -> HRESULT;
    pub unsafe fn x_game_ui_set_player_profile_card_ui_response (self: &Self, callback_handle: XGameUiCallbackHandle) -> HRESULT;
    pub unsafe fn x_game_ui_set_send_game_invite_ui_response (self: &Self, callback_handle: XGameUiCallbackHandle) -> HRESULT;
    pub unsafe fn x_game_ui_set_achievements_ui_response (self: &Self, callback_handle: XGameUiCallbackHandle) -> HRESULT;
    pub unsafe fn x_game_ui_set_multiplayer_activity_game_invite_ui_response (self: &Self, callback_handle: XGameUiCallbackHandle) -> HRESULT;
    pub unsafe fn x_game_ui_set_error_dialog_ui_response (self: &Self, callback_handle: XGameUiCallbackHandle) -> HRESULT;

}

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

pub struct XUserGetTokenAndSignatureUtf16HttpHeader {
    name: *const u16,
    value: *const u16,
}

pub struct XUserGetTokenAndSignatureUtf16Data {
    pub token_count: usize,
    pub signature_count: usize,
    pub token: *const u16,
    pub signature: *const u16,
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
    pub unsafe fn x_user_add_async(
        &self,
        options: u32,
        async_block: *mut c_void,
    ) -> HRESULT;
    pub unsafe fn x_user_add_result(&self, async_block: *mut c_void, user: *mut *mut c_void) -> HRESULT;
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
    pub unsafe fn xuser_get_token_and_signature_utf16_async(
        &self,
        user: *const c_void,
        options: u32,
        method: *const u16,
        url: *const u16,
        header_count: usize,
        headers: *const XUserGetTokenAndSignatureUtf16HttpHeader,
        body_size: usize,
        body_buffer: *const c_void,
        async_block: *mut c_void,
    ) -> HRESULT;
    pub unsafe fn xuser_get_token_and_signature_utf16_result_size(&self, async_block: *mut c_void, result_size: *mut usize) -> HRESULT;
    pub unsafe fn xuser_get_token_and_signature_utf16_result(&self, async_block: *mut c_void, buffer_size: usize, buffer: *mut u8, ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureUtf16Data, buffer_used: *mut usize) -> HRESULT;
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
        let hr = unsafe { get_result(async_.cast(), null_mut(), &mut payload) };
        if hr != S_OK {
            return hr;
        }

        unsafe {
            *(license as *mut XStoreGameLicense) = payload.license;
        }
        S_OK
    }
}

impl IXStoreAlias1_Impl for XStoreObject_Impl {}
impl IXStoreAlias2_Impl for XStoreObject_Impl {}
impl IXStoreAlias3_Impl for XStoreObject_Impl {}

#[implement(IXNetworking, IXNetworking2)]
pub struct XNetworkingObject;

#[repr(C)]
pub struct XNetworkingConnectivityHint {
    pub connectivityLevel: u32,
    pub connectivityCost: u32,
    pub ianaInterfaceType: u32,
    pub networkInitialized: u8,
    pub approachingDataLimit: u8,
    pub overDataLimit: u8,
    pub roaming: u8,
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
                connectivityLevel: 3,
                connectivityCost: 1,
                ianaInterfaceType: 0,
                networkInitialized: 1,
                approachingDataLimit: 0,
                overDataLimit: 0,
                roaming: 0,
            }
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
                        connectivityLevel: 3,
                        connectivityCost: 1,
                        ianaInterfaceType: 0,
                        networkInitialized: 1,
                        approachingDataLimit: 0,
                        overDataLimit: 0,
                        roaming: 0,
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
        let hr = unsafe {
            get_result(
                asyncBlock.cast(),
                null_mut(),
                securityInformationBuffer.cast::<XNetworkingSecurityInformation>(),
            )
        };
        if hr.is_ok() {
            unsafe { *securityInformation = securityInformationBuffer.cast() };
            // println!("XNetworkingQuerySecurityInformationForUrlAsyncResult: OK");
            S_OK
        } else {
            todo!(
                "XNetworkingQuerySecurityInformationForUrlAsyncResult {}",
                hr
            );
            hr
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
        let hr = unsafe {
            get_result(
                asyncBlock.cast(),
                null_mut(),
                securityInformationBuffer.cast::<XNetworkingSecurityInformation>(),
            )
        };
        if hr.is_ok() {
            unsafe { *securityInformation = securityInformationBuffer.cast() };
            S_OK
        } else {
            hr
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
        _ => crate::delegated_query_api_impl(runtime_class_id, interface_id, out),
    };
    res
}

#[cfg(test)]
mod tests {
    use std::ffi::{c_char, c_void};
    use std::ptr::{null, null_mut};

    use crate::com::{IXStore, XStoreGameLicense, get_result, query_api_impl};
    use crate::xasync::{XAsyncBlock, get_status, run};
    use crate::{
        E_FAIL, InitializeApiImplEx2, UninitializeApiImpl, set_delegated_dll_path_for_test,
    };
    use windows::Storage::Pickers::{FileOpenPicker, PickerLocationId, PickerViewMode};
    use windows::Storage::{IStorageItem, IStorageItem_Impl, StorageFile};
    use windows::shobjidl_core::{IInitializeWithWindow, IInitializeWithWindow_Impl};
    use windows::windef::HWND;
    use windows_core::{GUID, HRESULT, HSTRING, Interface, implement};

    // #[implement(IInitializeWithWindow)]
    // struct RsStorageFile;

    // impl IStorageFile2_Impl for RsStorageFile_Impl {

    // }

    // #[implement(IStorageItem)]
    // struct MyStorageItem {}

    // impl IStorageItem_Impl for MyStorageItem_Impl {
    //     fn RenameAsyncOverloadDefaultOptions(
    //         &self,
    //         desiredName: &windows_core::HSTRING,
    //     ) -> windows_core::Result<windows_future::IAsyncAction> {
    //         todo!()
    //     }

    //     fn RenameAsync(
    //         &self,
    //         desiredName: &windows_core::HSTRING,
    //         option: windows::Storage::NameCollisionOption,
    //     ) -> windows_core::Result<windows_future::IAsyncAction> {
    //         todo!()
    //     }

    //     fn DeleteAsyncOverloadDefaultOptions(
    //         &self,
    //     ) -> windows_core::Result<windows_future::IAsyncAction> {
    //         todo!()
    //     }

    //     fn DeleteAsync(
    //         &self,
    //         option: windows::Storage::StorageDeleteOption,
    //     ) -> windows_core::Result<windows_future::IAsyncAction> {
    //         todo!()
    //     }

    //     fn GetBasicPropertiesAsync(
    //         &self,
    //     ) -> windows_core::Result<
    //         windows_future::IAsyncOperation<windows::Storage::FileProperties::BasicProperties>,
    //     > {
    //         todo!()
    //     }

    //     fn Name(&self) -> windows_core::Result<windows_core::HSTRING> {
    //         todo!()
    //     }

    //     fn Path(&self) -> windows_core::Result<windows_core::HSTRING> {
    //         todo!()
    //     }

    //     fn Attributes(&self) -> windows_core::Result<windows::Storage::FileAttributes> {
    //         todo!()
    //     }

    //     fn DateCreated(&self) -> windows_core::Result<windows_time::DateTime> {
    //         todo!()
    //     }

    //     fn IsOfType(
    //         &self,
    //         r#type: windows::Storage::StorageItemTypes,
    //     ) -> windows_core::Result<bool> {
    //         todo!()
    //     }
    // }

    // #[implement(FileOpenPicker)]
    // struct MyStoragePicker  {

    // }

    // impl FileOpenPicker_Impl for MyStoragePicker_Impl {

    // }
    // #[repr(transparent)]
    // #[derive(Clone, Debug, Eq, PartialEq)]
    // pub struct RsFileOpenPicker(FileOpenPicker);

    // windows_core::imp::interface_hierarchy!(
    //     RsFileOpenPicker,
    //     FileOpenPicker,
    //     windows_core::IUnknown,
    //     windows_core::IInspectable
    // );
    // impl IInitializeWithWindow_Impl for RsFileOpenPicker {
    //     fn Initialize(&self, hwnd: windows::windef::HWND) -> windows_core::Result<()> {
    //         todo!()
    //     }
    // }

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

        // windows::Storage::Pickers::
    }

    pub async fn pick_file(hwnd: HWND) -> windows_core::Result<StorageFile> {
        let picker = FileOpenPicker::new()?;

        picker.SetViewMode(PickerViewMode::List)?;
        picker.SetSuggestedStartLocation(PickerLocationId::DocumentsLibrary)?;

        let filters = picker.FileTypeFilter()?;
        filters.Append(&HSTRING::from(".xml"))?;
        filters.Append(&HSTRING::from(".txt"))?;

        // FileOpenPicker is a WinRT object, but desktop apps need to supply
        // the owner HWND through this COM interop interface.
        let initialize: IInitializeWithWindow = picker.cast()?;

        unsafe {
            initialize.Initialize(hwnd);
        }

        let file = picker.PickSingleFileAsync()?.await?;
        Ok(file)
    }

    #[tokio::test]
    async fn test_picker() {
        let file = pick_file(HWND(null_mut())).await.unwrap();
        println!("{}", file.DisplayName().unwrap().to_string_lossy());
    }
}
