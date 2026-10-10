// D3D12CreateDevice @101
// D3D12GetDebugInterface @102
// D3D12CreateRootSignatureDeserializer
// D3D12CreateVersionedRootSignatureDeserializer

// D3D12EnableExperimentalFeatures
// D3D12SerializeRootSignature
// D3D12SerializeVersionedRootSignature
// D3D12GetInterface

use std::{
    os::raw::c_void,
    ptr::{null, null_mut},
    sync::atomic::AtomicPtr,
};

use rustls::lock::Mutex;
use windows::libloaderapi::{GetProcAddress, LoadLibraryA};
use windows_core::{BOOL, GUID, HRESULT, IUnknown, Interface, implement, interface};
// use windows::Graphics::DirectX::

use crate::{S_OK, results::E_NOINTERFACE};

static CHECK_FEATURE_SUPPORT: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

type ty = extern "system" fn(s: *mut c_void, f: i32, data: *mut c_void, len: u32) -> HRESULT;
extern "system" fn my_CHECK_FEATURE_SUPPORT(
    s: *mut c_void,
    feature: i32,
    feature_data: *mut c_void,
    feature_data_size: u32,
) -> HRESULT {
    println!("check_feature_support {feature} size {feature_data_size}");
    let org = CHECK_FEATURE_SUPPORT.load(std::sync::atomic::Ordering::Acquire);
    let org: ty = unsafe { std::mem::transmute_copy(&org) };

    let hr = match feature {
        // 0 => {
        //     S_OK
        // },
        7 => S_OK,
        // 4 => {
        //     S_OK
        // },
        _ => org(s, feature, feature_data, feature_data_size), //self.child.check_feature_support(feature, feature_data, feature_data_size)//HRESULT(0x80070057u32 as i32) // E_INVALIDARG
    };
    // let hr = self.child.check_feature_support(feature, feature_data, feature_data_size);
    println!("check_feature_support ({hr}) {feature} size {feature_data_size}");
    match feature {
        0 => S_OK,
        7 => {
            let feature_data =
                unsafe { &mut *(feature_data as *mut D3D12_FEATURE_DATA_SHADER_MODEL) };
            println!("{:?}", feature_data);
            feature_data.HighestShaderModel = 0x60;
            println!("{:?}", feature_data);
            S_OK
        }
        _ => hr,
    }
}

#[allow(invalid_reference_casting)]
#[unsafe(no_mangle)]
pub extern "system" fn D3D12CreateDevice(
    adapter: *mut IUnknown,
    minimum_feature_level: i32,
    iid: *const GUID,
    device: *mut *mut c_void,
) -> HRESULT {
    println!(
        "D3D12CreateDevice {} minimum_feature_level: {:x}, {:?}",
        adapter as u64,
        minimum_feature_level,
        unsafe { *iid }
    );
    // let adapter = unsafe { &*adapter };
    // if minimum_feature_level >= 0xc000 {
    //     return HRESULT(0x80070057u32 as i32); // E_INVALIDARG
    // }
    // todo!();
    let hande = unsafe {
        LoadLibraryA(windows_core::PCSTR::from_raw(
            c"D3D12.apple.dll".as_ptr() as *const u8
        ))
    };
    let addr = unsafe {
        GetProcAddress(
            hande,
            windows_core::PCSTR::from_raw(c"D3D12CreateDevice".as_ptr() as *const u8),
        )
        .unwrap()
    };
    let dest: unsafe extern "system" fn(
        _adapter: *mut IUnknown,
        minimum_feature_level: i32,
        iid: *const GUID,
        _device: *mut *mut c_void,
    ) -> HRESULT = unsafe { std::mem::transmute_copy(&addr) };
    let res = unsafe { dest(adapter, minimum_feature_level, iid, device) };
    println!(
        "D3D12CreateDevice created ({res}) {} minimum_feature_level: {:x}, {:?}",
        adapter as u64,
        minimum_feature_level,
        unsafe { *iid }
    );
    // res
    let child = unsafe { ID3D12Device14::from_raw(*device) };
    let table = unsafe { child.assume_vtable::<ID3D12Device>() };
    CHECK_FEATURE_SUPPORT.store(
        table.check_feature_support as *mut c_void,
        std::sync::atomic::Ordering::Release,
    );
    let rwtable = unsafe { &mut *(table as *const ID3D12Device_Vtbl as *mut ID3D12Device_Vtbl) };
    rwtable.check_feature_support = my_CHECK_FEATURE_SUPPORT;
    // let obj : IUnknown = Device { child }.into();
    // unsafe { obj.query(iid, device) }
    res
}

#[interface("189819f1-1db6-4b57-be54-1821339b85f7")]
pub unsafe trait ID3D12Device: IUnknown {
    unsafe fn __reserved_slot_3(&self);
    unsafe fn __reserved_slot_4(&self);
    unsafe fn __reserved_slot_5(&self);
    unsafe fn __reserved_slot_6(&self);
    // unsafe fn __reserved_slot_7(&self);
    unsafe fn get_node_count(&self) -> u32;
    unsafe fn __reserved_slot_8(&self);
    unsafe fn __reserved_slot_9(&self);
    unsafe fn __reserved_slot_10(&self);
    unsafe fn __reserved_slot_11(&self);
    unsafe fn __reserved_slot_12(&self);
    // unsafe fn __reserved_slot_13(&self);
    unsafe fn check_feature_support(
        &self,
        feature: i32,
        feature_data: *mut c_void,
        feature_data_size: u32,
    ) -> HRESULT;
    unsafe fn __reserved_slot_14(&self);
    unsafe fn __reserved_slot_15(&self);
    unsafe fn __reserved_slot_16(&self);
    unsafe fn __reserved_slot_17(&self);
    unsafe fn __reserved_slot_18(&self);
    unsafe fn __reserved_slot_19(&self);
    unsafe fn __reserved_slot_20(&self);
    unsafe fn __reserved_slot_21(&self);
    unsafe fn __reserved_slot_22(&self);
    unsafe fn __reserved_slot_23(&self);
    unsafe fn __reserved_slot_24(&self);
    unsafe fn __reserved_slot_25(&self);
    unsafe fn __reserved_slot_26(&self);
    unsafe fn __reserved_slot_27(&self);
    unsafe fn __reserved_slot_28(&self);
    unsafe fn __reserved_slot_29(&self);
    unsafe fn __reserved_slot_30(&self);
    unsafe fn __reserved_slot_31(&self);
    unsafe fn __reserved_slot_32(&self);
    unsafe fn __reserved_slot_33(&self);
    unsafe fn __reserved_slot_34(&self);
    unsafe fn __reserved_slot_35(&self);
    unsafe fn __reserved_slot_36(&self);
    unsafe fn __reserved_slot_37(&self);
    unsafe fn __reserved_slot_38(&self);
    unsafe fn __reserved_slot_39(&self);
    unsafe fn __reserved_slot_40(&self);
    unsafe fn __reserved_slot_41(&self);
    unsafe fn __reserved_slot_42(&self);
}

#[interface("77acce80-638e-4e65-8895-c1f23386863e")]
pub unsafe trait ID3D12Device1: ID3D12Device {}

#[interface("30baa41e-b15b-475c-a0bb-1af5c5b64328")]
pub unsafe trait ID3D12Device2: ID3D12Device1 {}

#[interface("81dadc15-2bad-4392-93c5-101345c4aa98")]
pub unsafe trait ID3D12Device3: ID3D12Device2 {}

#[interface("e865df17-a9ee-46f9-a463-3098315aa2e5")]
pub unsafe trait ID3D12Device4: ID3D12Device3 {}

#[interface("8b4f173b-2fea-4b80-8f58-4307191ab95d")]
pub unsafe trait ID3D12Device5: ID3D12Device4 {}

#[interface("c70b221b-40e4-4a17-89af-025a0727a6dc")]
pub unsafe trait ID3D12Device6: ID3D12Device5 {}

#[interface("5c014b53-68a1-4b9b-8bd1-dd6046b9358b")]
pub unsafe trait ID3D12Device7: ID3D12Device6 {}

#[interface("9218e6bb-f944-4f7e-a75c-b1b2c7b701f3")]
pub unsafe trait ID3D12Device8: ID3D12Device7 {}

#[interface("4c80e962-f032-4f60-bc9e-ebc2cfa1d83c")]
pub unsafe trait ID3D12Device9: ID3D12Device8 {}

#[interface("517f8718-aa66-49f9-b02b-a7ab89c06031")]
pub unsafe trait ID3D12Device10: ID3D12Device9 {}

#[interface("5405c344-d457-444e-b4dd-2366e45aee39")]
pub unsafe trait ID3D12Device11: ID3D12Device10 {}

#[interface("5af5c532-4c91-4cd0-b541-15a405395fc5")]
pub unsafe trait ID3D12Device12: ID3D12Device11 {}

#[interface("14eecffc-4df8-40f7-a118-5c816f45695e")]
pub unsafe trait ID3D12Device13: ID3D12Device12 {}

#[interface("5f6e592d-d895-44c2-8e4a-88ad4926d323")]
pub unsafe trait ID3D12Device14: ID3D12Device13 {}

#[implement(ID3D12Device14)]
struct Device {
    child: ID3D12Device14,
}

// #[derive(Debug)]
// struct D3D12_FEATURE_DATA_ARCHITECTURE {
//     NodeIndex: u32,
//     TileBasedRenderer: BOOL,
//     UMA: BOOL,
//     CacheCoherentUMA: BOOL,
// }
#[derive(Debug)]
struct D3D12_FEATURE_DATA_SHADER_MODEL {
    HighestShaderModel: u32,
}

impl ID3D12Device_Impl for Device_Impl {
    unsafe fn __reserved_slot_3(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_4(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_5(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_6(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_8(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_9(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_10(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_11(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_12(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_14(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_15(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_16(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_17(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_18(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_19(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_20(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_21(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_22(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_23(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_24(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_25(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_26(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_27(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_28(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_29(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_30(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_31(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_32(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_33(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_34(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_35(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_36(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_37(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_38(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_39(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_40(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_41(&self) {
        todo!()
    }

    unsafe fn __reserved_slot_42(&self) {
        todo!()
    }

    unsafe fn check_feature_support(
        &self,
        feature: i32,
        feature_data: *mut c_void,
        feature_data_size: u32,
    ) -> HRESULT {
        println!("check_feature_support {feature} size {feature_data_size}");
        let hr = match feature {
            // 0 => {
            //     S_OK
            // },
            // 7 => {
            //     S_OK
            // },
            _ => self
                .child
                .check_feature_support(feature, feature_data, feature_data_size), //HRESULT(0x80070057u32 as i32) // E_INVALIDARG
        };
        // let hr = self.child.check_feature_support(feature, feature_data, feature_data_size);
        println!("check_feature_support ({hr}) {feature} size {feature_data_size}");
        match feature {
            0 => S_OK,
            7 => {
                let feature_data =
                    unsafe { &mut *(feature_data as *mut D3D12_FEATURE_DATA_SHADER_MODEL) };
                println!("{:?}", feature_data);
                feature_data.HighestShaderModel = 0x60;
                println!("{:?}", feature_data);
                S_OK
            }
            _ => hr,
        }
    }

    unsafe fn get_node_count(&self) -> u32 {
        println!("get_node_count");
        let c = self.child.get_node_count();
        // 1
        println!("get_node_count ({c})");
        c
    }
}

impl ID3D12Device1_Impl for Device_Impl {}
impl ID3D12Device2_Impl for Device_Impl {}
impl ID3D12Device3_Impl for Device_Impl {}
impl ID3D12Device4_Impl for Device_Impl {}
impl ID3D12Device5_Impl for Device_Impl {}
impl ID3D12Device6_Impl for Device_Impl {}
impl ID3D12Device7_Impl for Device_Impl {}
impl ID3D12Device8_Impl for Device_Impl {}
impl ID3D12Device9_Impl for Device_Impl {}
impl ID3D12Device10_Impl for Device_Impl {}
impl ID3D12Device11_Impl for Device_Impl {}
impl ID3D12Device12_Impl for Device_Impl {}
impl ID3D12Device13_Impl for Device_Impl {}
impl ID3D12Device14_Impl for Device_Impl {}

#[interface("82bc481c-6b9b-4030-aedb-7ee3d1df1e63")]
pub unsafe trait ID3D12DeviceRemovedExtendedDataSettings: IUnknown {
    pub unsafe fn set_auto_breadcrumbs_enablement(&self, enablement: i32);
    pub unsafe fn set_page_fault_enablement(&self, enablement: i32);
    pub unsafe fn set_watson_dump_enablement(&self, enablement: i32);
}

#[interface("DBD5AE51-3317-4F0A-ADF9-1D7CEDCAAE0B")]
pub unsafe trait ID3D12DeviceRemovedExtendedDataSettings1:
    ID3D12DeviceRemovedExtendedDataSettings
{
    pub unsafe fn set_breadcrumb_context_enablement(&self, enablement: i32);
}

#[interface("61552388-01ab-4008-a436-83db189566ea")]
pub unsafe trait ID3D12DeviceRemovedExtendedDataSettings2:
    ID3D12DeviceRemovedExtendedDataSettings1
{
    pub unsafe fn use_markers_only_auto_breadcrumbs(&self, markers_only: bool);
}

#[implement(ID3D12DeviceRemovedExtendedDataSettings2)]
pub struct D3D12DeviceRemovedExtendedDataSettings;

impl ID3D12DeviceRemovedExtendedDataSettings_Impl for D3D12DeviceRemovedExtendedDataSettings_Impl {
    unsafe fn set_auto_breadcrumbs_enablement(&self, enablement: i32) {
        println!("set_auto_breadcrumbs_enablement");
    }

    unsafe fn set_page_fault_enablement(&self, enablement: i32) {
        println!("set_page_fault_enablement");
    }

    unsafe fn set_watson_dump_enablement(&self, enablement: i32) {
        println!("set_watson_dump_enablement");
    }
}

impl ID3D12DeviceRemovedExtendedDataSettings1_Impl for D3D12DeviceRemovedExtendedDataSettings_Impl {
    unsafe fn set_breadcrumb_context_enablement(&self, enablement: i32) {
        println!("set_breadcrumb_context_enablement");
    }
}

impl ID3D12DeviceRemovedExtendedDataSettings2_Impl for D3D12DeviceRemovedExtendedDataSettings_Impl {
    unsafe fn use_markers_only_auto_breadcrumbs(&self, markers_only: bool) {
        println!("use_markers_only_auto_breadcrumbs");
    }
}
// [
//     uuid(82bc481c-6b9b-4030-aedb-7ee3d1df1e63),
//     object,
//     local,
//     pointer_default(unique)
// ]
// interface ID3D12DeviceRemovedExtendedDataSettings : IUnknown
// {
//     void SetAutoBreadcrumbsEnablement(D3D12_DRED_ENABLEMENT enablement);
//     void SetPageFaultEnablement(D3D12_DRED_ENABLEMENT enablement);
//     void SetWatsonDumpEnablement(D3D12_DRED_ENABLEMENT enablement);
// }

#[unsafe(no_mangle)]
pub extern "system" fn D3D12GetDebugInterface(
    iid: *const GUID,
    debug: *mut *mut c_void,
) -> HRESULT {
    println!("D3D12GetDebugInterface {:?}", unsafe { *iid });
    // E_NOINTERFACE
    let obj: IUnknown = D3D12DeviceRemovedExtendedDataSettings {}.into();
    unsafe { obj.query(iid, debug) }
    // todo!();
}

#[unsafe(no_mangle)]
pub extern "system" fn D3D12CreateRootSignatureDeserializer(
    rcslid: *const GUID,
    iid: *const GUID,
    debug: *mut *mut c_void,
) -> HRESULT {
    todo!();
}

#[unsafe(no_mangle)]
pub extern "system" fn D3D12CreateVersionedRootSignatureDeserializer(
    rcslid: *const GUID,
    iid: *const GUID,
    debug: *mut *mut c_void,
) -> HRESULT {
    todo!();
}

#[unsafe(no_mangle)]
pub extern "system" fn D3D12EnableExperimentalFeatures(
    feature_count: u32,
    iids: *const GUID,
    configurations: *mut c_void,
    configurations_sizes: *mut u32,
) -> HRESULT {
    // Seems to be ok for Minecraft Legends
    E_NOINTERFACE
}

#[unsafe(no_mangle)]
pub extern "system" fn D3D12SerializeRootSignature(
    rcslid: *const GUID,
    iid: *const GUID,
    debug: *mut *mut c_void,
) -> HRESULT {
    todo!();
}

#[unsafe(no_mangle)]
pub extern "system" fn D3D12SerializeVersionedRootSignature(
    rcslid: *const GUID,
    iid: *const GUID,
    debug: *mut *mut c_void,
) -> HRESULT {
    todo!();
}

#[unsafe(no_mangle)]
pub extern "system" fn D3D12GetInterface(
    rcslid: *const GUID,
    iid: *const GUID,
    debug: *mut *mut c_void,
) -> HRESULT {
    todo!();
}
