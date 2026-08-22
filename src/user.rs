use std::ptr::null_mut;
use std::{
    ffi::{CStr, c_char},
    os::raw::c_void,
    sync::Arc,
};
use windows_core::{HRESULT, IUnknown, Interface, implement, interface};
use xal_new::SignaturePolicyCache;
use xal_xodus as xal;
#[cfg(feature = "xuser")]
use xodus::{auth::do_sisu, secrets, tokens::TokenManager};

use crate::threading::XAsyncBlock;
use crate::{
    E_FAIL,
    results::S_OK,
    threading::{XTaskQueueHandle, XTaskQueueRegistrationToken},
    xasync,
};

#[repr(u32)]
pub enum XUserAddOptions {
    None = 0x00,
    AddDefaultUserSilently = 0x01,
    AllowGuests = 0x02,
    AddDefaultUserAllowingUI = 0x04,
}
#[repr(u32)]
pub enum XUserAgeGroup {
    Unknown = 0,
    Child = 1,
    Teen = 2,
    Adult = 3,
}
#[repr(u32)]
pub enum XUserChangeEvent {
    SignedInAgain = 0,
    SigningOut = 1,
    SignedOut = 2,
    Gamertag = 3,
    GamerPicture = 4,
    Privileges = 5,
}
#[repr(u32)]
pub enum XUserDefaultAudioEndpointKind {
    CommunicationRender = 0,
    CommunicationCapture = 1,
}
#[repr(u32)]
pub enum XUserGamerPictureSize {
    Small = 0,
    Medium = 1,
    Large = 2,
    ExtraLarge = 3,
}
#[repr(u32)]
pub enum XUserGamertagComponent {
    Classic = 0,
    Modern = 1,
    ModernSuffix = 2,
    UniqueModern = 3,
}
#[repr(u32)]
pub enum XUserGetMsaTokenSilentlyOptions {
    None = 0x00,
}
#[repr(u32)]
pub enum XUserGetTokenAndSignatureOptions {
    None = 0x00,
    ForceRefresh = 0x01,
    AllUsers = 0x02,
}
#[repr(u32)]
pub enum XUserPrivilege {
    CrossPlay = 185,
    Clubs = 188,
    Sessions = 189,
    Broadcast = 190,
    ManageProfilePrivacy = 196,
    GameDvr = 198,
    MultiplayerParties = 203,
    CloudManageSession = 207,
    CloudJoinSession = 208,
    CloudSavedGames = 209,
    SocialNetworkSharing = 220,
    UserGeneratedContent = 247,
    Communications = 252,
    Multiplayer = 254,
    AddFriends = 255,
}
#[repr(u32)]
pub enum XUserPrivilegeDenyReason {
    None = 0,
    PurchaseRequired = 1,
    Restricted = 2,
    Banned = 3,
    Unknown = 0xFFFFFFFF,
}
#[repr(u32)]
pub enum XUserPrivilegeOptions {
    None = 0x00,
    AllUsers = 0x01,
}
#[repr(u32)]
pub enum XUserState {
    SignedIn = 0,
    SigningOut = 1,
    SignedOut = 2,
}
#[repr(u32)]
pub enum XUserPlatformOperationResult {
    Success = 0,
    Failure = 1,
    Canceled = 2,
}

#[repr(u32)]
pub enum XUserPlatformSpopOperationResult {
    SignInHere = 0,
    SwitchAccount = 1,
    Failure = 2,
    Canceled = 3,
}

pub struct AppLocalDeviceId {
    pub value: [u8; 16],
}

#[repr(C)]
pub struct XUserDeviceAssociationChange {
    pub device_id: AppLocalDeviceId,
    pub old_user: XUserLocalId,
    pub new_user: XUserLocalId,
}
#[repr(C)]
pub struct XUserGetTokenAndSignatureData {
    pub token_size: usize,
    pub signature_size: usize,
    pub token: *const c_char,
    pub signature: *const c_char,
}
#[repr(C)]
pub struct XUserGetTokenAndSignatureHttpHeader {
    pub name: *const c_char,
    pub value: *const c_char,
}
#[repr(C)]
pub struct XUserGetTokenAndSignatureUtf16Data {
    pub token_count: usize,
    pub signature_count: usize,
    pub token: *const u16,
    pub signature: *const u16,
}
#[repr(C)]
pub struct XUserGetTokenAndSignatureUtf16HttpHeader {
    pub name: *const u16,
    pub value: *const u16,
}
#[repr(C)]
pub struct XUserLocalId {
    pub value: u64,
}

pub type XUserPlatformRemoteConnectShowPromptEventHandler = unsafe extern "system" fn(
    context: *const c_void,
    user_identifier: u32,
    operation: u32,
    url: *const c_char,
    code: *const c_char,
    qr_code_size: usize,
    qr_code: *const c_char,
);
pub type XUserPlatformRemoteConnectClosePromptEventHandler = unsafe extern "system" fn();

#[repr(C)]
pub struct XUserPlatformRemoteConnectEventHandler {
    pub show: Option<XUserPlatformRemoteConnectShowPromptEventHandler>,
    pub close: Option<XUserPlatformRemoteConnectClosePromptEventHandler>,
    pub context: *mut c_void,
}

pub type XUserHandle = *mut c_void;
pub type XUserSignOutDeferralHandle = *mut c_void;
pub type XUserChangeEventCallback = *mut c_void;
pub type XUserDeviceAssociationChangedCallback = *mut c_void;
pub type XUserPlatformSpopPromptEventHandlers = *mut c_void;

pub type XUserPlatformOperation = u64;

#[interface("01acd177-91f9-4763-a38e-ccbb55ce32e0")]
pub unsafe trait IXUser: IUnknown {
    pub unsafe fn x_user_duplicate_handle(
        self: &Self,
        handle: XUserHandle,
        duplicated_handle: *mut XUserHandle,
    ) -> HRESULT;
    pub unsafe fn x_user_close_handle(self: &Self, handle: XUserHandle);
    pub unsafe fn x_user_compare(self: &Self, user1: XUserHandle, user2: XUserHandle) -> u32;
    pub unsafe fn x_user_get_max_users(self: &Self, max_users: *mut u32) -> HRESULT;
    pub unsafe fn x_user_add_async(
        self: &Self,
        options: XUserAddOptions,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    pub unsafe fn x_user_add_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        new_user: *mut XUserHandle,
    ) -> HRESULT;
    pub unsafe fn x_user_get_local_id(
        self: &Self,
        user: XUserHandle,
        user_local_id: *mut XUserLocalId,
    ) -> HRESULT;
    pub unsafe fn x_user_find_user_by_local_id(
        self: &Self,
        user_local_id: XUserLocalId,
        handle: *mut XUserHandle,
    ) -> HRESULT;
    pub unsafe fn x_user_get_id(self: &Self, user: XUserHandle, user_id: *mut u64) -> HRESULT;
    pub unsafe fn x_user_find_user_by_id(
        self: &Self,
        user_id: u64,
        handle: *mut XUserHandle,
    ) -> HRESULT;
    pub unsafe fn x_user_get_is_guest(
        self: &Self,
        user: XUserHandle,
        is_guest: *mut bool,
    ) -> HRESULT;
    pub unsafe fn x_user_get_state(
        self: &Self,
        user: XUserHandle,
        state: *mut XUserState,
    ) -> HRESULT;
    pub unsafe fn ___1(self: &Self);
    pub unsafe fn x_user_get_gamer_picture_async(
        self: &Self,
        user: XUserHandle,
        picture_size: XUserGamerPictureSize,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    pub unsafe fn x_user_get_gamer_picture_result_size(
        self: &Self,
        async_: *mut XAsyncBlock,
        buffer_size: *mut usize,
    ) -> HRESULT;
    pub unsafe fn x_user_get_gamer_picture_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        buffer_size: usize,
        buffer: *mut c_void,
        buffer_used: *mut usize,
    ) -> HRESULT;
    pub unsafe fn x_user_get_age_group(
        self: &Self,
        user: XUserHandle,
        age_group: *mut XUserAgeGroup,
    ) -> HRESULT;
    pub unsafe fn x_user_check_privilege(
        self: &Self,
        user: XUserHandle,
        options: XUserPrivilegeOptions,
        privilege: XUserPrivilege,
        has_privilege: *mut bool,
        reason: *mut XUserPrivilegeDenyReason,
    ) -> HRESULT;
    pub unsafe fn x_user_resolve_privilege_with_ui_async(
        self: &Self,
        user: XUserHandle,
        options: XUserPrivilegeOptions,
        privilege: XUserPrivilege,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    pub unsafe fn x_user_resolve_privilege_with_ui_result(
        self: &Self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    pub unsafe fn x_user_get_token_and_signature_async(
        self: &Self,
        user: XUserHandle,
        options: XUserGetTokenAndSignatureOptions,
        method: *const c_char,
        url: *const c_char,
        header_count: usize,
        headers: *const XUserGetTokenAndSignatureHttpHeader,
        body_size: usize,
        body_buffer: *const c_void,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    pub unsafe fn x_user_get_token_and_signature_result_size(
        self: &Self,
        async_: *mut XAsyncBlock,
        buffer_size: *mut usize,
    ) -> HRESULT;
    pub unsafe fn x_user_get_token_and_signature_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        buffer_size: usize,
        buffer: *mut c_void,
        ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureData,
        buffer_used: *mut usize,
    ) -> HRESULT;
    pub unsafe fn x_user_get_token_and_signature_utf16_async(
        self: &Self,
        user: XUserHandle,
        options: XUserGetTokenAndSignatureOptions,
        method: *const u16,
        url: *const u16,
        header_count: usize,
        headers: *const XUserGetTokenAndSignatureUtf16HttpHeader,
        body_size: usize,
        body_buffer: *const c_void,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    pub unsafe fn x_user_get_token_and_signature_utf16_result_size(
        self: &Self,
        async_: *mut XAsyncBlock,
        buffer_size: *mut usize,
    ) -> HRESULT;
    pub unsafe fn x_user_get_token_and_signature_utf16_result(
        self: &Self,
        async_: *mut XAsyncBlock,
        buffer_size: usize,
        buffer: *mut c_void,
        ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureUtf16Data,
        buffer_used: *mut usize,
    ) -> HRESULT;
    pub unsafe fn x_user_resolve_issue_with_ui_async(
        self: &Self,
        user: XUserHandle,
        url: *const c_char,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    pub unsafe fn x_user_resolve_issue_with_ui_result(
        self: &Self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    pub unsafe fn x_user_resolve_issue_with_ui_utf16_async(
        self: &Self,
        user: XUserHandle,
        url: *const u16,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    pub unsafe fn x_user_resolve_issue_with_ui_utf16_result(
        self: &Self,
        async_: *mut XAsyncBlock,
    ) -> HRESULT;
    pub unsafe fn x_user_register_for_change_event(
        self: &Self,
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: *mut XUserChangeEventCallback,
        token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT;
    pub unsafe fn x_user_unregister_for_change_event(
        self: &Self,
        token: XTaskQueueRegistrationToken,
        wait: bool,
    ) -> HRESULT;
    pub unsafe fn x_user_get_sign_out_deferral(
        self: &Self,
        deferral: *mut XUserSignOutDeferralHandle,
    ) -> HRESULT;
    pub unsafe fn x_user_close_sign_out_deferral_handle(
        self: &Self,
        deferral: XUserSignOutDeferralHandle,
    ) -> HRESULT;
    pub unsafe fn x_user_register_for_device_association_changed(
        self: &Self,
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: *mut XUserDeviceAssociationChangedCallback,
        token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT;
    pub unsafe fn x_user_unregister_for_device_association_changed(
        self: &Self,
        token: XTaskQueueRegistrationToken,
        wait: bool,
    ) -> HRESULT;
    pub unsafe fn ___2(self: &Self);
    pub unsafe fn ___3(self: &Self);
    pub unsafe fn ___4(self: &Self);
    pub unsafe fn x_user_is_store_user(self: &Self, user: XUserHandle) -> HRESULT;
    pub unsafe fn x_user_platform_remote_connect_set_event_handlers(
        self: &Self,
        queue: XTaskQueueHandle,
        handlers: *mut XUserPlatformRemoteConnectEventHandler,
    ) -> HRESULT;
    pub unsafe fn x_user_platform_remote_connect_cancel_prompt(
        self: &Self,
        operation: XUserPlatformOperation,
    ) -> HRESULT;
    pub unsafe fn x_user_platform_spop_prompt_set_event_handlers(
        self: &Self,
        queue: XTaskQueueHandle,
        handler: *mut XUserPlatformSpopPromptEventHandlers,
        context: *mut c_void,
    ) -> HRESULT;
    pub unsafe fn x_user_platform_spop_prompt_complete(
        self: &Self,
        operation: XUserPlatformOperation,
        result: XUserPlatformSpopOperationResult,
    ) -> HRESULT;
}

#[interface("cef4fac0-7676-4a94-a119-4c43f9eb5b74")]
pub unsafe trait IXUser2: IUnknown {
    pub unsafe fn get_gamertag(&self) -> *const c_char;
}

#[interface("26f3c674-a2fe-44fa-b6c4-a323bc94ff53")]
pub unsafe trait IXUser3: IXUser {}

#[implement(IXUser, IXUser2, IXUser3)]
pub struct XUser {
    pub runtime: tokio::runtime::Runtime,
}

#[interface("01acd177-91f9-4763-a38e-ccbb55ce32e0")]
unsafe trait IXUserHandle: IUnknown {
    unsafe fn get_xuid(&self) -> u64;
    unsafe fn get_local_id(&self) -> XUserLocalId;
    unsafe fn get_auth(&self) -> *const Arc<tokio::sync::Mutex<XuserHandleObjectAuth>>;
    unsafe fn get_runtime(&self) -> *const tokio::runtime::Handle;
}

struct XuserHandleObjectAuth {
    authenticator: xal::XalAuthenticator,
    auth: xal::response::SisuRPSAuthorizationResponse,
    policy: SignaturePolicyCache,
    def_policy: SignaturePolicyCache,
    device_token: xal::response::DeviceToken,
}

#[implement(IXUserHandle)]
struct XUserHandleObject {
    xuid: u64,
    local_id: XUserLocalId,
    auth: Arc<tokio::sync::Mutex<XuserHandleObjectAuth>>,
    runtime: tokio::runtime::Handle,
}

impl IXUserHandle_Impl for XUserHandleObject_Impl {
    unsafe fn get_xuid(&self) -> u64 {
        self.xuid
    }
    unsafe fn get_local_id(&self) -> XUserLocalId {
        XUserLocalId {
            value: self.local_id.value,
        }
    }

    // unsafe fn get_object(&self,) -> *mut XUserHandleObject {
    //     &mut self.this as *mut XUserHandleObject
    // }
    unsafe fn get_auth(&self) -> *const Arc<tokio::sync::Mutex<XuserHandleObjectAuth>> {
        &self.auth
    }

    unsafe fn get_runtime(&self) -> *const tokio::runtime::Handle {
        &self.runtime
    }
}

impl IXUser2_Impl for XUser_Impl {
    unsafe fn get_gamertag(&self) -> *const c_char {
        c"FakeGamertag".as_ptr() as *const c_char
    }
}

impl IXUser3_Impl for XUser_Impl {}

#[repr(C)]
struct XUserGetTokenAndSignatureDataWrapper {
    data: XUserGetTokenAndSignatureData,
    token: [u8; 4096],
    signature: [u8; 4096],
}

impl IXUser_Impl for XUser_Impl {
    unsafe fn x_user_duplicate_handle(
        &self,
        handle: XUserHandle,
        duplicated_handle: *mut XUserHandle,
    ) -> HRESULT {
        unsafe {
            IXUserHandle::from_raw_borrowed(&handle)
                .map(|f| {
                    *duplicated_handle = f.clone().into_raw();
                    S_OK
                })
                .unwrap_or(E_FAIL)
        }
    }

    unsafe fn x_user_close_handle(&self, handle: XUserHandle) {
        unsafe { IXUserHandle::from_raw(handle) };
    }

    unsafe fn x_user_compare(&self, user1: XUserHandle, user2: XUserHandle) -> u32 {
        let a = unsafe { IXUserHandle::from_raw_borrowed(&user1) };
        let b = unsafe { IXUserHandle::from_raw_borrowed(&user2) };
        let (Some(a), Some(b)) = (a, b) else {
            return 1;
        };
        (unsafe { a.get_xuid().cmp(&b.get_xuid()) }) as u32
    }

    unsafe fn x_user_get_max_users(&self, max_users: *mut u32) -> HRESULT {
        unsafe { *max_users = 4 };
        S_OK
    }

    unsafe fn x_user_add_async(
        &self,
        _options: XUserAddOptions,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        println!("x_user_add_async called");
        #[cfg(feature = "xuser")]
        let handle = self.runtime.handle().clone();
        #[cfg(feature = "xuser")]
        let handle2 = self.runtime.handle().clone();
        unsafe {
            xasync::run(async_ as *mut XAsyncBlock, {
                async move {
                    #[cfg(feature = "xuser")]
                    {
                        let user = handle
                            .spawn(async move {
                                let client = reqwest::Client::builder()
                                    // .use_native_tls()
                                    // .min_tls_version(Version::TLS_1_2)
                                    //  .max_tls_version(Version::TLS_1_2)
                                    .use_rustls_tls()
                                    .http1_only()
                                    .connection_verbose(true)
                                    .pool_max_idle_per_host(0)
                                    .connect_timeout(std::time::Duration::from_secs(5))
                                    .timeout(std::time::Duration::from_secs(10))
                                    .build()
                                    .unwrap(); // let manager = xodus::auth::Manager::new();
                                // let client_id = "your_client_id".to_string();
                                // let title_id = "your_title_id".to_string();
                                // do_sisu(&client, manager, client_id, title_id).await?;
                                std::env::set_var("HOME", std::env::var_os("USERPROFILE").unwrap());
                                println!("{}", std::env::var_os("HOME").unwrap().to_string_lossy());
                                secrets::init_secrets().expect("Unable to initialize credentials");
                                let tokens = TokenManager::with_keychain_and_memory();
                                let (c, resp, device) =
                                    do_sisu(&client, &tokens, "00000000441DF337", 0x663E2626)
                                        .await
                                        .unwrap();

                                println!("title {}", resp.title_token.token);
                                println!("user {}", resp.user_token.token);
                                println!("webpage {}", resp.web_page);

                                let r = client
                                    .get("https://title.mgt.xboxlive.com/titles/current/endpoints")
                                    .header("x-xbl-contract-version", "2")
                                    .header(
                                        "Authorization",
                                        resp.authorization_token.authorization_header_value(),
                                    )
                                    .send()
                                    .await
                                    .unwrap()
                                    .json::<xal_new::response::TitleEndpointsResponse>()
                                    .await
                                    .unwrap();
                                println!("{:?}", r);

                                let policy = SignaturePolicyCache::new(r);

                                let r = client
                                    .get("https://title.mgt.xboxlive.com/titles/default/endpoints?type=1")
                                    .send()
                                    .await
                                    .unwrap()
                                    .json::<xal_new::response::TitleEndpointsResponse>()
                                    .await
                                    .unwrap();
                                println!("{:?}", r);

                                let def_policy = SignaturePolicyCache::new(r);

                                let xid = resp
                                    .authorization_token
                                    .display_claims
                                    .as_ref()
                                    .map(|d| d.xui[0]["xid"].clone())
                                    .unwrap();

                                let handle = XUserHandleObject {
                                    xuid: xid.parse::<u64>().unwrap(),
                                    local_id: XUserLocalId { value: 987654321 },
                                    auth: Arc::new(tokio::sync::Mutex::new(XuserHandleObjectAuth {
                                        authenticator: c,
                                        auth: resp,
                                        policy,
                                        device_token: device,
                                        def_policy: def_policy,
                                    })),
                                    runtime: handle2,
                                };
                                let h: IXUserHandle = handle.into();

                                Ok::<_, HRESULT>(h.into_raw() as u64)
                            })
                            .await
                            .unwrap()?;
                        return Ok::<_, HRESULT>(user as *mut c_void);
                    }
                    #[cfg(not(feature = "xuser"))]
                    {
                        return Err::<*mut c_void, _>(E_FAIL);
                    }
                }
            })
        }
    }

    unsafe fn x_user_add_result(
        &self,
        async_: *mut XAsyncBlock,
        new_user: *mut XUserHandle,
    ) -> HRESULT {
        println!("x_user_add_result called");
        unsafe { xasync::get_result(async_ as *mut XAsyncBlock, null_mut(), new_user).unwrap() };
        // *new_user = h.into_raw();
        S_OK
    }

    unsafe fn x_user_get_local_id(
        &self,
        user: XUserHandle,
        user_local_id: *mut XUserLocalId,
    ) -> HRESULT {
        unsafe {
            IXUserHandle::from_raw_borrowed(&user)
                .map(|f| {
                    *user_local_id = f.get_local_id();
                    S_OK
                })
                .unwrap_or(E_FAIL)
        }
    }

    unsafe fn x_user_find_user_by_local_id(
        &self,
        _user_local_id: XUserLocalId,
        _handle: *mut XUserHandle,
    ) -> HRESULT {
        E_FAIL
    }

    unsafe fn x_user_get_id(&self, user: XUserHandle, user_id: *mut u64) -> HRESULT {
        unsafe {
            IXUserHandle::from_raw_borrowed(&user)
                .map(|f| {
                    *user_id = f.get_xuid();
                    S_OK
                })
                .unwrap_or(E_FAIL)
        }
    }

    unsafe fn x_user_find_user_by_id(&self, _user_id: u64, _handle: *mut XUserHandle) -> HRESULT {
        E_FAIL
    }

    unsafe fn x_user_get_is_guest(&self, _user: XUserHandle, is_guest: *mut bool) -> HRESULT {
        unsafe {
            *is_guest = false;
        };
        S_OK
    }

    unsafe fn x_user_get_state(&self, _user: XUserHandle, state: *mut XUserState) -> HRESULT {
        unsafe {
            *state = XUserState::SignedIn;
        };
        S_OK
    }

    unsafe fn ___1(&self) {
        todo!()
    }

    unsafe fn x_user_get_gamer_picture_async(
        &self,
        _user: XUserHandle,
        _picture_size: XUserGamerPictureSize,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_get_gamer_picture_result_size(
        &self,
        _async_: *mut XAsyncBlock,
        _buffer_size: *mut usize,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_get_gamer_picture_result(
        &self,
        _async_: *mut XAsyncBlock,
        _buffer_size: usize,
        _buffer: *mut c_void,
        _buffer_used: *mut usize,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_get_age_group(
        &self,
        _user: XUserHandle,
        age_group: *mut XUserAgeGroup,
    ) -> HRESULT {
        unsafe {
            *age_group = XUserAgeGroup::Adult;
        };
        S_OK
    }

    unsafe fn x_user_check_privilege(
        &self,
        _user: XUserHandle,
        _options: XUserPrivilegeOptions,
        _privilege: XUserPrivilege,
        has_privilege: *mut bool,
        reason: *mut XUserPrivilegeDenyReason,
    ) -> HRESULT {
        unsafe {
            *has_privilege = true;
        };
        unsafe {
            *reason = XUserPrivilegeDenyReason::None;
        };
        S_OK
    }

    unsafe fn x_user_resolve_privilege_with_ui_async(
        &self,
        _user: XUserHandle,
        _options: XUserPrivilegeOptions,
        _privilege: XUserPrivilege,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_resolve_privilege_with_ui_result(&self, _async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    // struct FakeToken

    unsafe fn x_user_get_token_and_signature_async(
        &self,
        user: XUserHandle,
        _options: XUserGetTokenAndSignatureOptions,
        _method: *const c_char,
        url: *const c_char,
        _header_count: usize,
        _headers: *const XUserGetTokenAndSignatureHttpHeader,
        _body_size: usize,
        _body_buffer: *const c_void,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        let user = unsafe { IXUserHandle::from_raw_borrowed(&user) };
        let handle = user.map(|f| unsafe { (*f.get_runtime()).clone() }).unwrap();
        let user = unsafe { (*user.unwrap().get_auth()).clone() };
        let url = unsafe { CStr::from_ptr(url) }.to_string_lossy().to_string();
        println!(
            "x_user_get_token_and_signature_async called with url: {}",
            url
        );
        unsafe {
            xasync::run(async_ as *mut XAsyncBlock, {
                async move {
                    let token = handle
                        .spawn(async move {
                            // let (device_token, title_token, user_token, pol) = {
                            let mut user = user.lock().await;
                            let device_token = user.device_token.clone();
                            let title_token = user.auth.title_token.clone();
                            let user_token = user.auth.user_token.clone();
                            let mut pol = user.policy.clone();
                            //     (device_token, title_token, user_token, pol)
                            // };

                            let ra = pol.find_relying_party_for_url(&url).await;
                            let rb = user.def_policy.find_relying_party_for_url(&url).await;
                            let relying_party = match ra {
                                Ok(Some(rp)) => rp,
                                _ => match rb {
                                    Ok(Some(rp)) => rp,
                                    _ => {
                                        panic!("No relying party found for url: {}", url);
                                    }
                                },
                            };
                            let token = user
                                .authenticator
                                .get_xsts_token(
                                    Some(&device_token),
                                    Some(&title_token),
                                    Some(&user_token),
                                    &relying_party,
                                )
                                .await
                                .unwrap();
                            println!("token: {}", token.authorization_header_value());
                            token.authorization_header_value()
                        })
                        .await
                        .unwrap();

                    println!("token(2): {}", token);

                    let mut buffer = [0; 4096];

                    buffer[..token.len()].copy_from_slice(token.as_bytes());
                    buffer[token.len()] = 0; // null terminate

                    println!("token(3): {}", token);

                    Ok::<_, HRESULT>(XUserGetTokenAndSignatureDataWrapper {
                        data: XUserGetTokenAndSignatureData {
                            token_size: token.len() + 1, // include null terminator
                            signature_size: 0,
                            token: std::ptr::null() as *const c_char,
                            signature: std::ptr::null() as *const c_char,
                        },
                        signature: [0; 4096],
                        token: buffer,
                    })
                }
            })
        }
    }

    unsafe fn x_user_get_token_and_signature_result_size(
        &self,
        _async_: *mut XAsyncBlock,
        buffer_size: *mut usize,
    ) -> HRESULT {
        unsafe { *buffer_size = std::mem::size_of::<XUserGetTokenAndSignatureDataWrapper>() };
        S_OK
    }

    unsafe fn x_user_get_token_and_signature_result(
        &self,
        async_: *mut XAsyncBlock,
        buffer_size: usize,
        buffer: *mut c_void,
        ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureData,
        buffer_used: *mut usize,
    ) -> HRESULT {
        println!("x_user_get_token_and_signature_result a");
        if buffer_size < std::mem::size_of::<XUserGetTokenAndSignatureDataWrapper>() {
            return E_FAIL;
        }
        let pbuf = buffer.cast::<XUserGetTokenAndSignatureDataWrapper>();
        unsafe { xasync::get_result(async_ as *mut XAsyncBlock, null_mut(), pbuf).unwrap() };
        println!("x_user_get_token_and_signature_result b");
        println!(
            "x_user_get_token_and_signature_result b {}",
            unsafe { &*pbuf }.data.token_size
        );
        let parts = &unsafe { &*pbuf }.token[..unsafe { &*pbuf }.data.token_size - 1];

        println!("token b {}", std::str::from_utf8(parts).unwrap());

        println!(
            "token c {}",
            unsafe { &*pbuf }.token[unsafe { &*pbuf }.data.token_size - 1]
        );

        unsafe {
            (*pbuf).data.token = (*pbuf).token.as_ptr() as *const c_char;
            // (*pbuf).data.signature = (*pbuf).signature.as_ptr() as *const c_char;
        }
        println!(
            "x_user_get_token_and_signature_result c {}",
            unsafe { CStr::from_ptr((*pbuf).data.token) }.to_string_lossy()
        );

        println!("x_user_get_token_and_signature_result c");
        unsafe { *ptr_to_buffer = buffer as *mut XUserGetTokenAndSignatureData };
        if !buffer_used.is_null() {
            unsafe { *buffer_used = std::mem::size_of::<XUserGetTokenAndSignatureDataWrapper>() };
        }
        println!("x_user_get_token_and_signature_result d");
        // unsafe { std::ptr::write(buffer as *mut XUserGetTokenAndSignatureData, data) };

        S_OK
    }

    unsafe fn x_user_get_token_and_signature_utf16_async(
        &self,
        _user: XUserHandle,
        _options: XUserGetTokenAndSignatureOptions,
        _method: *const u16,
        url: *const u16,
        _header_count: usize,
        _headers: *const XUserGetTokenAndSignatureUtf16HttpHeader,
        _body_size: usize,
        _body_buffer: *const c_void,
        async_: *mut XAsyncBlock,
    ) -> HRESULT {
        let url = windows_strings::PCWSTR::from_raw(url);
        println!(
            "x_user_get_token_and_signature_utf16_async called with url: {}",
            unsafe { url.to_string() }.unwrap()
        );
        unsafe {
            xasync::run(async_ as *mut XAsyncBlock, {
                async { Ok::<_, HRESULT>(()) }
            })
        }
    }

    unsafe fn x_user_get_token_and_signature_utf16_result_size(
        &self,
        _async_: *mut XAsyncBlock,
        buffer_size: *mut usize,
    ) -> HRESULT {
        unsafe { *buffer_size = std::mem::size_of::<XUserGetTokenAndSignatureUtf16Data>() };
        S_OK
    }

    unsafe fn x_user_get_token_and_signature_utf16_result(
        &self,
        _async_: *mut XAsyncBlock,
        _buffer_size: usize,
        buffer: *mut c_void,
        ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureUtf16Data,
        buffer_used: *mut usize,
    ) -> HRESULT {
        let data = XUserGetTokenAndSignatureUtf16Data {
            token_count: 6,
            signature_count: 0,
            token: windows::core::w!("token").as_ptr() as *const u16,
            signature: std::ptr::null() as *const u16,
        };
        unsafe { std::ptr::write(buffer as *mut XUserGetTokenAndSignatureUtf16Data, data) };
        unsafe { *ptr_to_buffer = buffer as *mut XUserGetTokenAndSignatureUtf16Data };
        if !buffer_used.is_null() {
            unsafe { *buffer_used = std::mem::size_of::<XUserGetTokenAndSignatureUtf16Data>() };
        }
        S_OK
    }

    unsafe fn x_user_resolve_issue_with_ui_async(
        &self,
        _user: XUserHandle,
        _url: *const c_char,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_resolve_issue_with_ui_result(&self, _async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_resolve_issue_with_ui_utf16_async(
        &self,
        _user: XUserHandle,
        _url: *const u16,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_resolve_issue_with_ui_utf16_result(
        &self,
        _async_: *mut XAsyncBlock,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_register_for_change_event(
        &self,
        _queue: XTaskQueueHandle,
        _context: *mut c_void,
        _callback: *mut XUserChangeEventCallback,
        _token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT {
        println!("x_user_register_for_change_event called");
        S_OK
    }

    unsafe fn x_user_unregister_for_change_event(
        &self,
        _token: XTaskQueueRegistrationToken,
        _wait: bool,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_get_sign_out_deferral(
        &self,
        _deferral: *mut XUserSignOutDeferralHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_close_sign_out_deferral_handle(
        &self,
        _deferral: XUserSignOutDeferralHandle,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_register_for_device_association_changed(
        &self,
        _queue: XTaskQueueHandle,
        _context: *mut c_void,
        _callback: *mut XUserDeviceAssociationChangedCallback,
        _token: *mut XTaskQueueRegistrationToken,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_unregister_for_device_association_changed(
        &self,
        _token: XTaskQueueRegistrationToken,
        _wait: bool,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn ___2(&self) {
        todo!()
    }

    unsafe fn ___3(&self) {
        todo!()
    }

    unsafe fn ___4(&self) {
        todo!()
    }

    unsafe fn x_user_is_store_user(&self, _user: XUserHandle) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_platform_remote_connect_set_event_handlers(
        &self,
        _queue: XTaskQueueHandle,
        _handlers: *mut XUserPlatformRemoteConnectEventHandler,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_platform_remote_connect_cancel_prompt(
        &self,
        _operation: XUserPlatformOperation,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_platform_spop_prompt_set_event_handlers(
        &self,
        _queue: XTaskQueueHandle,
        _handler: *mut XUserPlatformSpopPromptEventHandlers,
        _context: *mut c_void,
    ) -> HRESULT {
        todo!()
    }

    unsafe fn x_user_platform_spop_prompt_complete(
        &self,
        _operation: XUserPlatformOperation,
        _result: XUserPlatformSpopOperationResult,
    ) -> HRESULT {
        todo!()
    }
}
