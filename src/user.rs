use std::{ffi::{CStr, CString, c_char}, mem, os::{raw::c_void, windows::raw::HANDLE}, sync::Arc};
use std::ptr::null_mut;
use rustls::lock::Mutex;
// use reqwest::tls::Version;
use windows_core::{HRESULT, IUnknown, Interface, implement, interface};
use xal::SignaturePolicyCache;
use xodus::{auth::do_sisu, secrets, tokens::TokenManager};

use crate::{E_FAIL, results::S_OK, threading::{XTaskQueueHandle, XTaskQueueRegistrationToken}, xasync::{self, XAsyncBlock}};

#[repr(u32)]
enum XUserAddOptions {
    None = 0x00,
    AddDefaultUserSilently = 0x01,
    AllowGuests = 0x02,
    AddDefaultUserAllowingUI = 0x04,
}
#[repr(u32)]
enum XUserAgeGroup {
    Unknown = 0,
    Child = 1,
    Teen = 2,
    Adult = 3,
}
#[repr(u32)]
enum XUserChangeEvent {
    SignedInAgain = 0,
    SigningOut = 1,
    SignedOut = 2,
    Gamertag = 3,
    GamerPicture = 4,
    Privileges = 5,
}
#[repr(u32)]
enum XUserDefaultAudioEndpointKind {
    CommunicationRender = 0,
    CommunicationCapture = 1
}
#[repr(u32)]
enum XUserGamerPictureSize {
    Small = 0,
    Medium = 1,
    Large = 2,
    ExtraLarge = 3,
}
#[repr(u32)]
enum XUserGamertagComponent {
    Classic = 0,
    Modern = 1,
    ModernSuffix = 2,
    UniqueModern = 3,
}
#[repr(u32)]
enum XUserGetMsaTokenSilentlyOptions {
    None = 0x00,
}
#[repr(u32)]
enum XUserGetTokenAndSignatureOptions {
    None = 0x00,
    ForceRefresh = 0x01,
    AllUsers = 0x02,
}
#[repr(u32)]
enum XUserPrivilege {
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
enum XUserPrivilegeDenyReason {
    None = 0,
    PurchaseRequired = 1,
    Restricted = 2,
    Banned = 3,
    Unknown = 0xFFFFFFFF
}
#[repr(u32)]
enum XUserPrivilegeOptions {
    None = 0x00,
    AllUsers = 0x01,
}
#[repr(u32)]
enum XUserState {
    SignedIn = 0,
    SigningOut = 1,
    SignedOut = 2,
}
#[repr(u32)]
enum XUserPlatformOperationResult {
    Success = 0,
    Failure = 1,
    Canceled = 2
}

#[repr(u32)]
enum XUserPlatformSpopOperationResult {
    SignInHere = 0,
    SwitchAccount = 1,
    Failure = 2,
    Canceled = 3,
}

struct APP_LOCAL_DEVICE_ID {
    value: [u8; 16],
}

#[repr(C)]
struct XUserDeviceAssociationChange {
deviceId: APP_LOCAL_DEVICE_ID,
oldUser: XUserLocalId,
newUser: XUserLocalId,
}
#[repr(C)]
struct XUserGetTokenAndSignatureData {
tokenSize: usize,
signatureSize: usize,
token: *const c_char,
signature: *const c_char,
}
#[repr(C)]
struct XUserGetTokenAndSignatureHttpHeader {
name: *const c_char,
value: *const c_char,
}
#[repr(C)]
struct XUserGetTokenAndSignatureUtf16Data {
tokenCount: usize,
signatureCount: usize,
token: *const u16,
signature: *const u16,
}
#[repr(C)]
struct XUserGetTokenAndSignatureUtf16HttpHeader {
name: *const u16,
value: *const u16,
}
#[repr(C)]
struct XUserLocalId {
value: u64,
}
#[repr(C)]
struct XUserPlatformRemoteConnectEventHandler {
show: *mut c_void,
close: *mut c_void,
context: *mut c_void,
}

pub type XUserHandle = *mut c_void;
pub type XUserSignOutDeferralHandle = *mut c_void;
pub type XUserChangeEventCallback = *mut c_void;
pub type XUserDeviceAssociationChangedCallback = *mut c_void;
pub type XUserPlatformSpopPromptEventHandlers = *mut c_void;

pub type XUserPlatformOperation = u64;

#[interface("01acd177-91f9-4763-a38e-ccbb55ce32e0")]
pub unsafe trait IXUser: IUnknown {
pub unsafe fn x_user_duplicate_handle (self: &Self, handle: XUserHandle, duplicated_handle: *mut XUserHandle) -> HRESULT;
pub unsafe fn x_user_close_handle (self: &Self, handle: XUserHandle);
pub unsafe fn x_user_compare (self: &Self, user1: XUserHandle, user2: XUserHandle) -> u32;
pub unsafe fn x_user_get_max_users (self: &Self, max_users: *mut u32) -> HRESULT;
pub unsafe fn x_user_add_async (self: &Self, options: XUserAddOptions, async_: *mut XAsyncBlock) -> HRESULT;
pub unsafe fn x_user_add_result (self: &Self, async_: *mut XAsyncBlock, new_user: *mut XUserHandle) -> HRESULT;
pub unsafe fn x_user_get_local_id (self: &Self, user: XUserHandle, user_local_id: *mut XUserLocalId) -> HRESULT;
pub unsafe fn x_user_find_user_by_local_id (self: &Self, user_local_id: XUserLocalId, handle: *mut XUserHandle) -> HRESULT;
pub unsafe fn x_user_get_id (self: &Self, user: XUserHandle, user_id: *mut u64) -> HRESULT;
pub unsafe fn x_user_find_user_by_id (self: &Self, user_id: u64, handle: *mut XUserHandle) -> HRESULT;
pub unsafe fn x_user_get_is_guest (self: &Self, user: XUserHandle, is_guest: *mut bool) -> HRESULT;
pub unsafe fn x_user_get_state (self: &Self, user: XUserHandle, state: *mut XUserState) -> HRESULT;
pub unsafe fn ___1 (self: &Self);
pub unsafe fn x_user_get_gamer_picture_async (self: &Self, user: XUserHandle, picture_size: XUserGamerPictureSize, async_: *mut XAsyncBlock) -> HRESULT;
pub unsafe fn x_user_get_gamer_picture_result_size (self: &Self, async_: *mut XAsyncBlock, buffer_size: *mut usize) -> HRESULT;
pub unsafe fn x_user_get_gamer_picture_result (self: &Self, async_: *mut XAsyncBlock, buffer_size: usize, buffer: *mut c_void, buffer_used: *mut usize) -> HRESULT;
pub unsafe fn x_user_get_age_group (self: &Self, user: XUserHandle, age_group: *mut XUserAgeGroup) -> HRESULT;
pub unsafe fn x_user_check_privilege (self: &Self, user: XUserHandle, options: XUserPrivilegeOptions, privilege: XUserPrivilege, has_privilege: *mut bool, reason: *mut XUserPrivilegeDenyReason) -> HRESULT;
pub unsafe fn x_user_resolve_privilege_with_ui_async (self: &Self, user: XUserHandle, options: XUserPrivilegeOptions, privilege: XUserPrivilege, async_: *mut XAsyncBlock) -> HRESULT;
pub unsafe fn x_user_resolve_privilege_with_ui_result (self: &Self, async_: *mut XAsyncBlock) -> HRESULT;
pub unsafe fn x_user_get_token_and_signature_async (self: &Self, user: XUserHandle, options: XUserGetTokenAndSignatureOptions, method: *const c_char, url: *const c_char, header_count: usize, headers: *const XUserGetTokenAndSignatureHttpHeader, body_size: usize, body_buffer: *const c_void, async_: *mut XAsyncBlock) -> HRESULT;
pub unsafe fn x_user_get_token_and_signature_result_size (self: &Self, async_: *mut XAsyncBlock, buffer_size: *mut usize) -> HRESULT;
pub unsafe fn x_user_get_token_and_signature_result (self: &Self, async_: *mut XAsyncBlock, buffer_size: usize, buffer: *mut c_void, ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureData, buffer_used: *mut usize) -> HRESULT;
pub unsafe fn x_user_get_token_and_signature_utf16_async (self: &Self, user: XUserHandle, options: XUserGetTokenAndSignatureOptions, method: *const u16, url: *const u16, header_count: usize, headers: *const XUserGetTokenAndSignatureUtf16HttpHeader, body_size: usize, body_buffer: *const c_void, async_: *mut XAsyncBlock) -> HRESULT;
pub unsafe fn x_user_get_token_and_signature_utf16_result_size (self: &Self, async_: *mut XAsyncBlock, buffer_size: *mut usize) -> HRESULT;
pub unsafe fn x_user_get_token_and_signature_utf16_result (self: &Self, async_: *mut XAsyncBlock, buffer_size: usize, buffer: *mut c_void, ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureUtf16Data, buffer_used: *mut usize) -> HRESULT;
pub unsafe fn x_user_resolve_issue_with_ui_async (self: &Self, user: XUserHandle, url: *const c_char, async_: *mut XAsyncBlock) -> HRESULT;
pub unsafe fn x_user_resolve_issue_with_ui_result (self: &Self, async_: *mut XAsyncBlock) -> HRESULT;
pub unsafe fn x_user_resolve_issue_with_ui_utf16_async (self: &Self, user: XUserHandle, url: *const u16, async_: *mut XAsyncBlock) -> HRESULT;
pub unsafe fn x_user_resolve_issue_with_ui_utf16_result (self: &Self, async_: *mut XAsyncBlock) -> HRESULT;
pub unsafe fn x_user_register_for_change_event (self: &Self, queue: XTaskQueueHandle, context: *mut c_void, callback: *mut XUserChangeEventCallback, token: *mut XTaskQueueRegistrationToken) -> HRESULT;
pub unsafe fn x_user_unregister_for_change_event (self: &Self, token: XTaskQueueRegistrationToken, wait: bool) -> HRESULT;
pub unsafe fn x_user_get_sign_out_deferral (self: &Self, deferral: *mut XUserSignOutDeferralHandle) -> HRESULT;
pub unsafe fn x_user_close_sign_out_deferral_handle (self: &Self, deferral: XUserSignOutDeferralHandle) -> HRESULT;
pub unsafe fn x_user_register_for_device_association_changed (self: &Self, queue: XTaskQueueHandle, context: *mut c_void, callback: *mut XUserDeviceAssociationChangedCallback, token: *mut XTaskQueueRegistrationToken) -> HRESULT;
pub unsafe fn x_user_unregister_for_device_association_changed (self: &Self, token: XTaskQueueRegistrationToken, wait: bool) -> HRESULT;
pub unsafe fn ___2 (self: &Self);
pub unsafe fn x_user_is_store_user (self: &Self, user: XUserHandle) -> HRESULT;
pub unsafe fn x_user_platform_remote_connect_set_event_handlers (self: &Self, queue: XTaskQueueHandle, handlers: *mut XUserPlatformRemoteConnectEventHandler) -> HRESULT;
pub unsafe fn x_user_platform_remote_connect_cancel_prompt (self: &Self, operation: XUserPlatformOperation) -> HRESULT;
pub unsafe fn x_user_platform_spop_prompt_set_event_handlers (self: &Self, queue: XTaskQueueHandle, handler: *mut XUserPlatformSpopPromptEventHandlers, context: *mut c_void) -> HRESULT;
pub unsafe fn x_user_platform_spop_prompt_complete (self: &Self, operation: XUserPlatformOperation, result: XUserPlatformSpopOperationResult) -> HRESULT;
}

#[interface("cef4fac0-7676-4a94-a119-4c43f9eb5b74")]
pub unsafe trait IXUser2: IUnknown {
    pub unsafe fn get_gamertag(&self) -> *const c_char;
}

#[interface("26f3c674-a2fe-44fa-b6c4-a323bc94ff53")]
pub unsafe trait IXUser3: IXUser {
    // unsafe fn __reserved_slot_3(&self) -> HRESULT;
    // unsafe fn __reserved_slot_4(&self) -> HRESULT;
    // unsafe fn __reserved_slot_5(&self) -> HRESULT;
    // unsafe fn __reserved_slot_6(&self) -> HRESULT;
    // unsafe fn __reserved_slot_7(&self) -> HRESULT;
    // unsafe fn __reserved_slot_8(&self) -> HRESULT;
    // unsafe fn __reserved_slot_9(&self) -> HRESULT;
    // unsafe fn __reserved_slot_10(&self) -> HRESULT;
    // unsafe fn __reserved_slot_11(&self) -> HRESULT;
    // unsafe fn __reserved_slot_12(&self) -> HRESULT;
    // unsafe fn __reserved_slot_13(&self) -> HRESULT;
    // unsafe fn __reserved_slot_14(&self) -> HRESULT;
    // unsafe fn __reserved_slot_15(&self) -> HRESULT;
    // unsafe fn __reserved_slot_16(&self) -> HRESULT;
    // unsafe fn __reserved_slot_17(&self) -> HRESULT;
    // unsafe fn __reserved_slot_18(&self) -> HRESULT;
    // unsafe fn __reserved_slot_19(&self) -> HRESULT;
    // unsafe fn __reserved_slot_20(&self) -> HRESULT;
    // unsafe fn __reserved_slot_21(&self) -> HRESULT;
    // unsafe fn __reserved_slot_22(&self) -> HRESULT;
    // unsafe fn __reserved_slot_23(&self) -> HRESULT;
    // unsafe fn __reserved_slot_24(&self) -> HRESULT;
    // unsafe fn __reserved_slot_25(&self) -> HRESULT;
    // unsafe fn __reserved_slot_26(&self) -> HRESULT;
    // unsafe fn __reserved_slot_27(&self) -> HRESULT;
    // unsafe fn __reserved_slot_28(&self) -> HRESULT;
    // unsafe fn __reserved_slot_29(&self) -> HRESULT;
    // unsafe fn __reserved_slot_30(&self) -> HRESULT;
    // unsafe fn __reserved_slot_31(&self) -> HRESULT;
    // unsafe fn __reserved_slot_32(&self) -> HRESULT;
    // unsafe fn __reserved_slot_33(&self) -> HRESULT;
    // unsafe fn __reserved_slot_34(&self) -> HRESULT;
    // unsafe fn __reserved_slot_35(&self) -> HRESULT;
    // unsafe fn __reserved_slot_36(&self) -> HRESULT;
    // unsafe fn __reserved_slot_37(&self) -> HRESULT;
    // unsafe fn __reserved_slot_38(&self) -> HRESULT;
    // unsafe fn __reserved_slot_39(&self) -> HRESULT;
    // unsafe fn __reserved_slot_40(&self) -> HRESULT;
    // unsafe fn __reserved_slot_41(&self) -> HRESULT;
    // unsafe fn __reserved_slot_42(&self) -> HRESULT;
    // pub unsafe fn XUserPlatformRemoteConnectSetEventHandlers(
    //     &self,
    //     queue: *mut c_void,
    //     handler: *const c_void,
    // ) -> HRESULT;

}



#[implement(IXUser, IXUser2, IXUser3)]
pub struct XUser;

#[interface("01acd177-91f9-4763-a38e-ccbb55ce32e0")]
unsafe trait IXUserHandle : IUnknown {
    unsafe fn get_xuid(&self) -> u64;
    unsafe fn get_local_id(&self) -> XUserLocalId;
    // unsafe fn get_object(&self) -> *mut XUserHandleObject;
    unsafe fn get_auth(&self,) -> Arc<tokio::sync::Mutex<XUserHandleObject_Auth>>;
}

struct XUserHandleObject_Auth {
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
    auth: Arc<tokio::sync::Mutex<XUserHandleObject_Auth>>,
}

impl IXUserHandle_Impl for XUserHandleObject_Impl {
    unsafe fn get_xuid(&self,) -> u64 {
        self.xuid
    }
    unsafe fn get_local_id(&self,) -> XUserLocalId {
        XUserLocalId { value: self.local_id.value }
    }
    
    // unsafe fn get_object(&self,) -> *mut XUserHandleObject {
    //     &mut self.this as *mut XUserHandleObject
    // }
    unsafe fn get_auth(&self,) -> Arc<tokio::sync::Mutex<XUserHandleObject_Auth>> {
        self.auth.clone()
    }
}

impl IXUser2_Impl for XUser_Impl {
    unsafe fn get_gamertag(&self,) -> *const c_char {
        c"FakeGamertag".as_ptr() as *const c_char
    }
}

impl IXUser3_Impl for XUser_Impl {
    // unsafe fn __reserved_slot_3(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_4(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_5(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_6(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_7(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_8(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_9(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_10(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_11(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_12(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_13(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_14(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_15(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_16(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_17(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_18(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_19(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_20(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_21(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_22(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_23(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_24(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_25(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_26(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_27(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_28(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_29(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_30(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_31(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_32(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_33(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_34(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_35(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_36(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_37(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_38(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_39(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_40(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_41(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn __reserved_slot_42(&self,) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn XUserPlatformRemoteConnectSetEventHandlers(&self,queue: *mut c_void,handler: *const c_void) -> HRESULT {
    //     todo!()
    // }
}

#[repr(C)]
struct XUserGetTokenAndSignatureDataWrapper {
    data: XUserGetTokenAndSignatureData,
    token: [u8; 4096],
    signature: [u8; 4096],
}

impl IXUser_Impl for XUser_Impl {
    unsafe fn x_user_duplicate_handle(&self,handle: XUserHandle,duplicated_handle: *mut XUserHandle) -> HRESULT {
        IXUserHandle::from_raw_borrowed(&handle).map(|f| {
            *duplicated_handle = f.clone().into_raw();
            S_OK
        }).unwrap_or(E_FAIL)
    }
    
    unsafe fn x_user_close_handle(&self,handle: XUserHandle) {
        IXUserHandle::from_raw(handle);
    }
    
    unsafe fn x_user_compare(&self,user1: XUserHandle,user2: XUserHandle) -> u32 {
        let a = IXUserHandle::from_raw_borrowed(&user1);
        let b = IXUserHandle::from_raw_borrowed(&user2);
        let (Some(a), Some(b)) = (a, b) else {
            return 1;
        };
        a.get_xuid().cmp(&b.get_xuid()) as u32
    }
    
    unsafe fn x_user_get_max_users(&self,max_users: *mut u32) -> HRESULT {
        *max_users = 4;
        S_OK
    }
    
    unsafe fn x_user_add_async(&self,options: XUserAddOptions,async_: *mut XAsyncBlock) -> HRESULT {
        println!("x_user_add_async called");
        unsafe { xasync::run(async_ as *mut XAsyncBlock, {
            async {
                let user = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .spawn(async {
                        rustls::crypto::ring::default_provider()
                            .install_default()
                            .unwrap();
                        let client = reqwest::Client::builder()
                            // .use_native_tls()
                                // .min_tls_version(Version::TLS_1_2)
                                //  .max_tls_version(Version::TLS_1_2)
                                 .http1_only()
                            .connect_timeout(std::time::Duration::from_secs(5))
                            .timeout(std::time::Duration::from_secs(10))
                            .build().unwrap();                        // let manager = xodus::auth::Manager::new();
                        // let client_id = "your_client_id".to_string();
                        // let title_id = "your_title_id".to_string();
                        // do_sisu(&client, manager, client_id, title_id).await?;
                            env_logger::Builder::new()
                            .filter_level(log::LevelFilter::Debug)
                            .init();
                            std::env::set_var("HOME", std::env::var_os("USERPROFILE").unwrap());
                            println!("{}", std::env::var_os("HOME").unwrap().to_string_lossy());
                            secrets::init_secrets().expect("Unable to initialize credentials");
                            let tokens = TokenManager::with_keychain_and_memory();
                            let (c, resp, device) = do_sisu(&client, &tokens, "00000000441DF337", 0x663E2626)
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
                                .json::<xal::response::TitleEndpointsResponse>()
                                .await
                                .unwrap();
                            println!("{:?}", r);

                        let mut policy = SignaturePolicyCache::new(r);

                        let xid = resp
                            .authorization_token
                            .display_claims
                            .as_ref()
                            .map(|d| d.xui[0]["xid"].clone())
                            .unwrap();


                        let handle =XUserHandleObject {
                            xuid: xid.parse::<u64>().unwrap(),
                            local_id: XUserLocalId { value: 987654321 },
                            auth: Arc::new(tokio::sync::Mutex::new(XUserHandleObject_Auth {
                                authenticator: c,
                                auth: resp,
                                policy,
                                device_token: device,
                                def_policy: SignaturePolicyCache::default(),
                            }))
                        };
                        let h: IXUserHandle = handle.into();
                        
                        Ok::<_, HRESULT>(h.into_raw() as u64)
                    }).await.unwrap()?;
                Ok::<_, HRESULT>(user as *mut c_void)
            }
        }) }
    }
    
    unsafe fn x_user_add_result(&self,async_: *mut XAsyncBlock,new_user: *mut XUserHandle) -> HRESULT {
        println!("x_user_add_result called");
        xasync::get_result(async_ as *mut XAsyncBlock, null_mut(), new_user).unwrap();
        // *new_user = h.into_raw();
        S_OK
    }
    
    unsafe fn x_user_get_local_id(&self,user: XUserHandle,user_local_id: *mut XUserLocalId) -> HRESULT {
        IXUserHandle::from_raw_borrowed(&user).map(|f| {
            *user_local_id = f.get_local_id();
            S_OK
        }).unwrap_or(E_FAIL)
    }
    
    unsafe fn x_user_find_user_by_local_id(&self,user_local_id: XUserLocalId,handle: *mut XUserHandle) -> HRESULT {
        E_FAIL
    }
    
    unsafe fn x_user_get_id(&self,user: XUserHandle,user_id: *mut u64) -> HRESULT {
        IXUserHandle::from_raw_borrowed(&user).map(|f| {
            *user_id = f.get_xuid();
            S_OK
        }).unwrap_or(E_FAIL)
    }
    
    unsafe fn x_user_find_user_by_id(&self,user_id: u64,handle: *mut XUserHandle) -> HRESULT {
        E_FAIL
    }
    
    unsafe fn x_user_get_is_guest(&self,user: XUserHandle,is_guest: *mut bool) -> HRESULT {
        unsafe { *is_guest = false; };
        S_OK
    }
    
    unsafe fn x_user_get_state(&self,user: XUserHandle,state: *mut XUserState) -> HRESULT {
        unsafe { *state = XUserState::SignedIn; };
        S_OK
    }
    
    unsafe fn ___1(&self,) {
        todo!()
    }
    
    unsafe fn x_user_get_gamer_picture_async(&self,user: XUserHandle,picture_size: XUserGamerPictureSize,async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_get_gamer_picture_result_size(&self,async_: *mut XAsyncBlock,buffer_size: *mut usize) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_get_gamer_picture_result(&self,async_: *mut XAsyncBlock,buffer_size: usize,buffer: *mut c_void,buffer_used: *mut usize) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_get_age_group(&self,user: XUserHandle,age_group: *mut XUserAgeGroup) -> HRESULT {
        unsafe { *age_group = XUserAgeGroup::Adult; };
        S_OK
    }
    
    unsafe fn x_user_check_privilege(&self,user: XUserHandle,options: XUserPrivilegeOptions,privilege: XUserPrivilege,has_privilege: *mut bool,reason: *mut XUserPrivilegeDenyReason) -> HRESULT {
        unsafe { *has_privilege = true; };
        unsafe { *reason = XUserPrivilegeDenyReason::None; };
        S_OK
    }
    
    unsafe fn x_user_resolve_privilege_with_ui_async(&self,user: XUserHandle,options: XUserPrivilegeOptions,privilege: XUserPrivilege,async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_resolve_privilege_with_ui_result(&self,async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }

    // struct FakeToken
    
    unsafe fn x_user_get_token_and_signature_async(&self,user: XUserHandle,options: XUserGetTokenAndSignatureOptions,method: *const c_char,url: *const c_char,header_count: usize,headers: *const XUserGetTokenAndSignatureHttpHeader,body_size: usize,body_buffer: *const c_void,async_: *mut XAsyncBlock) -> HRESULT {
        let user = unsafe { IXUserHandle::from_raw_borrowed(&user) };
        let user = unsafe { user.unwrap().get_auth() };
        let url = unsafe { CStr::from_ptr(url) }.to_string_lossy().to_string();
        println!("x_user_get_token_and_signature_async called with url: {}", url);
        unsafe { xasync::run(async_ as *mut XAsyncBlock, {
            async move {
                let token = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap()
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
                    }
                };
                let token = user.authenticator.get_xsts_token(Some(&device_token), Some(&title_token), Some(&user_token), &relying_party).await.unwrap();
                println!("token: {}", token.authorization_header_value());
                    token.authorization_header_value()
                }).await.unwrap();

                println!("token(2): {}", token);

                let mut buffer = [0; 4096];

                buffer[..token.len()].copy_from_slice(token.as_bytes());
                buffer[token.len()] = 0; // null terminate

                println!("token(3): {}", token);

                Ok::<_, HRESULT>(XUserGetTokenAndSignatureDataWrapper{
                    data: XUserGetTokenAndSignatureData {
                        tokenSize: token.len() + 1, // include null terminator
                        signatureSize: 0,
                        token: std::ptr::null() as *const c_char,
                        signature: std::ptr::null() as *const c_char,
                    },
                    signature: [0; 4096],
                    token: buffer,
                })
            }
        })}
    }
    
    unsafe fn x_user_get_token_and_signature_result_size(&self,async_: *mut XAsyncBlock,buffer_size: *mut usize) -> HRESULT {
        *buffer_size = std::mem::size_of::<XUserGetTokenAndSignatureDataWrapper>();
        S_OK
    }
    
    unsafe fn x_user_get_token_and_signature_result(&self,async_: *mut XAsyncBlock,buffer_size: usize,buffer: *mut c_void,ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureData,buffer_used: *mut usize) -> HRESULT {
        println!("x_user_get_token_and_signature_result a");
        let data = XUserGetTokenAndSignatureData {
            tokenSize: 6,
            signatureSize: 0,
            token: c"token".as_ptr() as *const c_char,
            signature: std::ptr::null() as *const c_char,
        };
        // xasync::get_result(async_ as *mut XAsyncBlock, null_mut(), buffer).unwrap();
        let pbuf = buffer.cast::<XUserGetTokenAndSignatureDataWrapper>();
        xasync::get_result(async_ as *mut XAsyncBlock, null_mut(), pbuf).unwrap();
        // let data = unsafe { std::ptr::read(buffer as *const XUserGetTokenAndSignatureDataWrapper) };
        // unsafe { std::ptr::write(buffer as *mut XUserGetTokenAndSignatureData, data) };
        // TODO
        println!("x_user_get_token_and_signature_result b");
        println!("x_user_get_token_and_signature_result b {}", (*pbuf).data.tokenSize);
        // println!("token a {}", (*pbuf).token[0]);
        // println!("token b {}", (*pbuf).token[1]);
        let parts = &(&*pbuf).token[..(&*pbuf).data.tokenSize-1];

        println!("token b {}", std::str::from_utf8(parts).unwrap());


        println!("token c {}", (&*pbuf).token[(&*pbuf).data.tokenSize-1]);

        unsafe {
            (*pbuf).data.token = (*pbuf).token.as_ptr() as *const c_char;
            // (*pbuf).data.token = c"token".as_ptr() as *const c_char;
            // (*pbuf).data.tokenSize = 5;
            // (*pbuf).data.signatureSize = 0;
            // (*pbuf).data.signature = std::ptr::null() as *const c_char;
            // (*pbuf).data.signature = (*pbuf).signature.as_ptr() as *const c_char;
        }
        // println!("token b2 {}", String::from_utf8_lossy(std::slice::from_raw_parts((*pbuf).data.token as *const u8, (*pbuf).data.tokenSize)));

        // println!("token c2 {}", (&*pbuf).data.token[(&*pbuf).data.tokenSize-1]);
        println!("x_user_get_token_and_signature_result c {}", CStr::from_ptr((*pbuf).data.token).to_string_lossy());

        println!("x_user_get_token_and_signature_result c");
        unsafe { *ptr_to_buffer = buffer as *mut XUserGetTokenAndSignatureData };
        if !buffer_used.is_null() {
            unsafe { *buffer_used = std::mem::size_of::<XUserGetTokenAndSignatureDataWrapper>() };
        }
        println!("x_user_get_token_and_signature_result d");
        // unsafe { std::ptr::write(buffer as *mut XUserGetTokenAndSignatureData, data) };

        S_OK
    }
    
    unsafe fn x_user_get_token_and_signature_utf16_async(&self,user: XUserHandle,options: XUserGetTokenAndSignatureOptions,method: *const u16,url: *const u16,header_count: usize,headers: *const XUserGetTokenAndSignatureUtf16HttpHeader,body_size: usize,body_buffer: *const c_void,async_: *mut XAsyncBlock) -> HRESULT {
        let url = windows_strings::PCWSTR::from_raw(url);
        println!("x_user_get_token_and_signature_utf16_async called with url: {}", url.to_string().unwrap());
        xasync::run(async_ as *mut XAsyncBlock, {
            async {
                Ok::<_, HRESULT>(())
            }
        })
    }
    
    unsafe fn x_user_get_token_and_signature_utf16_result_size(&self,async_: *mut XAsyncBlock,buffer_size: *mut usize) -> HRESULT {
        *buffer_size = std::mem::size_of::<XUserGetTokenAndSignatureUtf16Data>();
        S_OK
    }
    
    unsafe fn x_user_get_token_and_signature_utf16_result(&self,async_: *mut XAsyncBlock,buffer_size: usize,buffer: *mut c_void,ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureUtf16Data,buffer_used: *mut usize) -> HRESULT {
        let data = XUserGetTokenAndSignatureUtf16Data {
            tokenCount: 6,
            signatureCount: 0,
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
    
    unsafe fn x_user_resolve_issue_with_ui_async(&self,user: XUserHandle,url: *const c_char,async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_resolve_issue_with_ui_result(&self,async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_resolve_issue_with_ui_utf16_async(&self,user: XUserHandle,url: *const u16,async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_resolve_issue_with_ui_utf16_result(&self,async_: *mut XAsyncBlock) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_register_for_change_event(&self,queue: XTaskQueueHandle,context: *mut c_void,callback: *mut XUserChangeEventCallback,token: *mut XTaskQueueRegistrationToken) -> HRESULT {
        println!("x_user_register_for_change_event called");
        S_OK
    }
    
    unsafe fn x_user_unregister_for_change_event(&self,token: XTaskQueueRegistrationToken,wait: bool) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_get_sign_out_deferral(&self,deferral: *mut XUserSignOutDeferralHandle) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_close_sign_out_deferral_handle(&self,deferral: XUserSignOutDeferralHandle) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_register_for_device_association_changed(&self,queue: XTaskQueueHandle,context: *mut c_void,callback: *mut XUserDeviceAssociationChangedCallback,token: *mut XTaskQueueRegistrationToken) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_unregister_for_device_association_changed(&self,token: XTaskQueueRegistrationToken,wait: bool) -> HRESULT {
        todo!()
    }
    
    unsafe fn ___2(&self,) {
        todo!()
    }
    
    unsafe fn x_user_is_store_user(&self,user: XUserHandle) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_platform_remote_connect_set_event_handlers(&self,queue: XTaskQueueHandle,handlers: *mut XUserPlatformRemoteConnectEventHandler) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_platform_remote_connect_cancel_prompt(&self,operation: XUserPlatformOperation) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_platform_spop_prompt_set_event_handlers(&self,queue: XTaskQueueHandle,handler: *mut XUserPlatformSpopPromptEventHandlers,context: *mut c_void) -> HRESULT {
        todo!()
    }
    
    unsafe fn x_user_platform_spop_prompt_complete(&self,operation: XUserPlatformOperation,result: XUserPlatformSpopOperationResult) -> HRESULT {
        todo!()
    }
    // unsafe fn x_user_duplicate_handle(&self,handle: XUserHandle,duplicated_handle: *mut XUserHandle) -> HRESULT {
    //     todo!()
    // }

    // unsafe fn x_user_close_handle(&self,handle: XUserHandle) {
    //     todo!()
    // }

    // unsafe fn x_user_compare(&self,user1: XUserHandle,user2: XUserHandle) {
    //     todo!()
    // }

    // unsafe fn x_user_get_max_users(&self,max_users: *mut u32) {
    //     todo!()
    // }

    // unsafe fn x_user_add_async(&self,options: XUserAddOptions,async_: *mut XAsyncBlock) {
    //     todo!()
    // }

    // unsafe fn x_user_add_result(&self,async_: *mut XAsyncBlock,new_user: *mut XUserHandle) {
    //     todo!()
    // }

    // unsafe fn x_user_get_local_id(&self,user: XUserHandle,user_local_id: *mut XUserLocalId) {
    //     todo!()
    // }

    // unsafe fn x_user_find_user_by_local_id(&self,user_local_id: XUserLocalId,handle: *mut XUserHandle) {
    //     todo!()
    // }

    // unsafe fn x_user_get_id(&self,user: XUserHandle,user_id: *mut u64) {
    //     todo!()
    // }

    // unsafe fn x_user_find_user_by_id(&self,) {
    //     todo!()
    // }

    // unsafe fn x_user_get_is_guest(&self,user: XUserHandle,is_guest: *mut bool) {
    //     todo!()
    // }

    // unsafe fn x_user_get_state(&self,user: XUserHandle,state: *mut XUserState) {
    //     todo!()
    // }

    // unsafe fn ___1(&self,) {
    //     todo!()
    // }

    // unsafe fn x_user_get_gamer_picture_async(&self,user: XUserHandle,picture_size: XUserGamerPictureSize,async_: *mut XAsyncBlock) {
    //     todo!()
    // }

    // unsafe fn x_user_get_gamer_picture_result_size(&self,async_: *mut XAsyncBlock,buffer_size: *mut usize) {
    //     todo!()
    // }

    // unsafe fn x_user_get_gamer_picture_result(&self,async_: *mut XAsyncBlock,buffer_size: usize,buffer: *mut c_void,buffer_used: *mut usize) {
    //     todo!()
    // }

    // unsafe fn x_user_get_age_group(&self,user: XUserHandle,age_group: *mut XUserAgeGroup) {
    //     todo!()
    // }

    // unsafe fn x_user_check_privilege(&self,user: XUserHandle,options: XUserPrivilegeOptions,privilege: XUserPrivilege,has_privilege: *mut bool,reason: *mut XUserPrivilegeDenyReason) {
    //     todo!()
    // }

    // unsafe fn x_user_resolve_privilege_with_ui_async(&self,user: XUserHandle,options: XUserPrivilegeOptions,privilege: XUserPrivilege,async_: *mut XAsyncBlock) {
    //     todo!()
    // }

    // unsafe fn x_user_resolve_privilege_with_ui_result(&self,async_: *mut XAsyncBlock) {
    //     todo!()
    // }

    // unsafe fn x_user_get_token_and_signature_async(&self,user: XUserHandle,options: XUserGetTokenAndSignatureOptions,method: *const c_char,url: *const c_char,header_count: usize,headers: *const XUserGetTokenAndSignatureHttpHeader,body_size: usize,body_buffer: *const c_void,async_: *mut XAsyncBlock) -> HRESULT {
    //     let method: String = CStr::from_ptr(method).to_string_lossy().into_owned();
    //     unsafe { xasync::run(async_ as *mut XAsyncBlock, {
    //         async {
    //             Ok::<_, HRESULT>(())
    //         }
    //     }) }
    // }

    // unsafe fn x_user_get_token_and_signature_result_size(&self,async_: *mut XAsyncBlock,buffer_size: *mut usize) {
    //     todo!()
    // }

    // unsafe fn x_user_get_token_and_signature_result(&self,async_: *mut XAsyncBlock,buffer_size: usize,buffer: *mut c_void,ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureData,buffer_used: *mut usize) {

    // }

    // unsafe fn x_user_get_token_and_signature_utf16_async(&self,user: XUserHandle,options: XUserGetTokenAndSignatureOptions,method: *const u16,url: *const u16,header_count: usize,headers: *const XUserGetTokenAndSignatureUtf16HttpHeader,body_size: usize,body_buffer: *const c_void,async_: *mut XAsyncBlock) {
    //     todo!()
    // }

    // unsafe fn x_user_get_token_and_signature_utf16_result_size(&self,async_: *mut XAsyncBlock,buffer_size: *mut usize) {
    //     todo!()
    // }

    // unsafe fn x_user_get_token_and_signature_utf16_result(&self,async_: *mut XAsyncBlock,buffer_size: usize,buffer: *mut c_void,ptr_to_buffer: *mut *mut XUserGetTokenAndSignatureUtf16Data,buffer_used: *mut usize) {
    //     todo!()
    // }

    // unsafe fn x_user_resolve_issue_with_ui_async(&self,user: XUserHandle,url: *const c_char,async_: *mut XAsyncBlock) {
    //     todo!()
    // }

    // unsafe fn x_user_resolve_issue_with_ui_result(&self,async_: *mut XAsyncBlock) {
    //     todo!()
    // }

    // unsafe fn x_user_resolve_issue_with_ui_utf16_async(&self,user: XUserHandle,url: *const u16,async_: *mut XAsyncBlock) {
    //     todo!()
    // }

    // unsafe fn x_user_resolve_issue_with_ui_utf16_result(&self,async_: *mut XAsyncBlock) {
    //     todo!()
    // }

    // unsafe fn x_user_register_for_change_event(&self,queue: XTaskQueueHandle,context: *mut c_void,callback: *mut XUserChangeEventCallback,token: *mut XTaskQueueRegistrationToken) {
    //     todo!()
    // }

    // unsafe fn x_user_unregister_for_change_event(&self,token: XTaskQueueRegistrationToken,wait: bool) {
    //     todo!()
    // }

    // unsafe fn x_user_get_sign_out_deferral(&self,deferral: *mut XUserSignOutDeferralHandle) {
    //     todo!()
    // }

    // unsafe fn x_user_close_sign_out_deferral_handle(&self,deferral: XUserSignOutDeferralHandle) {
    //     todo!()
    // }

    // unsafe fn x_user_register_for_device_association_changed(&self,queue: XTaskQueueHandle,context: *mut c_void,callback: *mut XUserDeviceAssociationChangedCallback,token: *mut XTaskQueueRegistrationToken) {
    //     todo!()
    // }

    // unsafe fn x_user_unregister_for_device_association_changed(&self,token: XTaskQueueRegistrationToken,wait: bool) {
    //     todo!()
    // }

    // unsafe fn ___2(&self,) {
    //     todo!()
    // }

    // unsafe fn x_user_is_store_user(&self,user: XUserHandle) {
    //     todo!()
    // }

    // unsafe fn x_user_platform_remote_connect_set_event_handlers(&self,queue: XTaskQueueHandle,handlers: *mut XUserPlatformRemoteConnectEventHandler) {
    //     todo!()
    // }

    // unsafe fn x_user_platform_remote_connect_cancel_prompt(&self,operation: XUserPlatformOperation) {
    //     todo!()
    // }

    // unsafe fn x_user_platform_spop_prompt_set_event_handlers(&self,queue: XTaskQueueHandle,handler: *mut XUserPlatformSpopPromptEventHandlers,context: *mut c_void) {
    //     todo!()
    // }

    // unsafe fn x_user_platform_spop_prompt_complete(&self,operation: XUserPlatformOperation,result: XUserPlatformSpopOperationResult) {
    //     todo!()
    // }
}