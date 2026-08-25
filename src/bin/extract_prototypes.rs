use std::{borrow::Cow, collections::{HashMap, HashSet}, fs::File, io::Read};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GDKMappingEntry {
    function: String,
    header: String,
    clsid: String,
    iid: String,
    vtable_offset: Option<i64>,
    vtable_index: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GDKMapping {
   pub mappings: Vec<GDKMappingEntry>
}

fn to_rust_type_ex(cpp_type: &str, is_inner: bool) -> Cow<str> {
    let st = match cpp_type {
        "uint8_t" | "BYTE" => "u8",
        "uint16_t" | "UINT16" => "u16",
        "int16_t" | "INT16" | "WORD" => "i16",
        "uint32_t" | "UINT32" => "u32",
        "int32_t" | "INT32" | "DWORD" => "i32",
        "uint64_t" | "ULONG64" | "ULONGLONG" => "u64",
        "int64_t" | "INT64" => "i64",
        "float" => "f32",
        "double" => "f64",
        "time_t" => "libc::time_t",
        "size_t" => "usize",
        "wchar_t" => "u16",
        "void" => if is_inner { "c_void" } else { "()" },
        "char" => "c_char",
        "STDAPI" => "()",
        "STDAPI_(bool)" => "BOOL",
        "bool" => "BOOL",
        _ => "",
    };
    if st.is_empty() {
        let array: regex::Regex = regex::Regex::new(r"^(const\s+)?(.+)\s*\*$").unwrap();
        if let Some(caps) = array.captures(cpp_type) {
            let base_type = caps.get(2).unwrap().as_str();
            let rust_base_type = to_rust_type_ex(base_type, true);
            if caps.get(1).is_some() {
                return format!("*const {}", rust_base_type).into();
            } else if rust_base_type.ends_with("Callback") {
                return format!("Option<{}>", rust_base_type).into();
            } else {
                return format!("*mut {}", rust_base_type).into();
            }
        }
        if cpp_type.starts_with("struct ") {
            let struct_name = cpp_type["struct ".len()..].trim();
            return struct_name.into();
        }
        if cpp_type.starts_with("const ") {
            let struct_name = cpp_type["const ".len()..].trim();
            return struct_name.into();
        }
    } else {
        return st.into();
    }
    return cpp_type.into();
}

fn to_rust_type(cpp_type: &str) -> Cow<str> {
    to_rust_type_ex(cpp_type, false)
}

// #[repr(u32)]
// enum StatusCode {
//     Ok = 200,
//     NotFound = 404,
//     InternalError = 500,
// }

fn to_snake_case(s: &str) -> String {
    let mut snake_case = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i != 0 {
                snake_case.push('_');
            }
            snake_case.push(c.to_ascii_lowercase());
        } else {
            snake_case.push(c);
        }
    }
    match snake_case.as_str() {
        "async" => "async_".to_owned(),
        "fn" => "fn_".to_owned(),
        "dyn" => "dyn_".to_owned(),
        "type" => "type_".to_owned(),
        "self" => "self_".to_owned(),
        other => other.to_string(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut f = File::open("gaming-gdk-gdk-2604.md")?;
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;
    let re = regex::Regex::new(r"enum class (\w+) *: *(\w+)[\s\n]*\{(([\n\s]*\w+\s*(=\s*\w+)?,?)+)[\n\s]*\}")?;
    
    let mut known = HashSet::new();
    // enum based on defines in .md
    known.insert("HCTraceLevel");

    for res in re.captures_iter(&buf) {
        if let (Some(name), Some(ty), Some(body)) = (res.get(1), res.get(2), res.get(3)) {
            if known.insert(name.as_str()) {
                // TODO normalize the name + body so it follows naming conventions
                print!("#[repr({})]\npub enum {} {{", to_rust_type(ty.as_str()), name.as_str());
                println!("{}", body.as_str());
                println!("}}");
            }
        }
        // println!("{:?}", res);
    }

    // typedef struct XUserGetTokenAndSignatureUtf16Data {
    //     size_t tokenCount;
    //     size_t signatureCount;
    //     const wchar_t* token;
    //     const wchar_t* signature;
    // } XUserGetTokenAndSignatureUtf16Data

    // typedef struct XblServiceConfigurationStatistic {
    //     char serviceConfigurationId[XBL_SCID_LENGTH];
    //     XblStatistic* statistics;
    //     uint32_t statisticsCount;
    // } XblServiceConfigurationStatistic

    let re = regex::Regex::new(r"[\s\n]struct (\w+) *[\s\n]*\{(((\n|\s|//.*\n)*([\w\*]+\s*)+;)+)[\n\s]*\}")?;
    let bodyre = regex::Regex::new(r"[\s\n]*(([\w\*]+\s+)+)(\w+)\s*;")?;
    
    let mut known = HashSet::new();

    for res in re.captures_iter(&buf) {
        if let (Some(name), Some(body)) = (res.get(1), res.get(2)) {
            if known.insert(name.as_str()) {
                // TODO normalize the name + body so it follows naming conventions
                println!("#[repr(C)]\npub struct {} {{", name.as_str());
                // println!("{}", body.as_str());
                for field in bodyre.captures_iter(body.as_str()) {
                    if let (Some(field), Some(ty)) = (field.get(3), field.get(1)) {
                        println!("    pub {}: {},", to_snake_case(field.as_str()), to_rust_type(ty.as_str().trim()));
                    }
                }
                println!("}}");
            }
        }
        // println!("{:?}", res);
    }

// HRESULT XUserResolvePrivilegeWithUiAsync(
//         XUserHandle user,
//         XUserPrivilegeOptions options,
//         XUserPrivilege privilege,
//         XAsyncBlock* async
// )

// Syntax

// C++

// STDAPI  XUserPlatformSpopPromptSetEventHandlers(
//     _In_opt_ XTaskQueueHandle queue,
//     _In_ XUserPlatformSpopPromptEventHandler* handler,
//     _In_opt_ void* context
// ) noexcept;

// STDAPI_(bool) XGameProtocolUnregisterForActivation(
// _In_ XTaskQueueRegistrationToken token,
// _In_ bool wait
// ) noexcept;

// Parameters
    // callbacks ok
    // let re = regex::Regex::new(r"C\+\+\s*\n\s*\n\s*(\w+)\s+(\w+)\s+(\w+)\s*\((((\n|\s|//.*\n)*([\w\*]+\s*)+,?)+)[\n\s]*\)")?;
    let re = regex::Regex::new(r"C\+\+\s*\n\s*\n\s*(?:typedef\s*)?(\w+|STDAPI_\(bool\))\s+(?:(\w+)\s+)?(\w+)\s*\((((\n|\s|//.*\n)*([\w\*]+\s*)+,?)*)?[\n\s]*\)\s*(noexcept\s*)?;?\n*\n\s*\n\s*(Parameters|Return value)")?;
    let bodyre = regex::Regex::new(r"[\s\n]*(([\w\*]+\s+)+)(\w+)\s*")?;
    let opt_re: regex::Regex = regex::Regex::new(r"Option<(\w+)>")?;
    
    let mut known = HashSet::new();
    let mut known_types = HashSet::new();

    let mapping: GDKMapping = serde_json::from_reader(File::open("gdk_mapping.json").unwrap()).unwrap();

    let mut prototypes: HashMap<String, String> = HashMap::new();
    let mut maybe_callbacks: HashMap<String, String> = HashMap::new();
    let mut callbacks: HashMap<String, String> = HashMap::new();

    for res in re.captures_iter(&buf) {
        if let (Some(ret), Some(name), Some(body)) = (res.get(1), res.get(3), res.get(4)) {
            if known.insert(name.as_str()) {
                let idx = if let Some(extra) = res.get(2) && extra.as_str() == "CALLBACK" { 1 } else { 2 };
                for ty in 0..idx {
                    let mut method = String::with_capacity(512);
                    method.push_str(&format!("// {}\n", name.as_str()));
                    let mut i = 0;
                    if ty == 0 {
                        method.push_str(&format!("pub type {} = unsafe extern \"system\" fn(", name.as_str()));
                    } else {
                        method.push_str(&format!("unsafe fn {}(self: &Self", to_snake_case(name.as_str())));
                        i+=1;
                    }
                    for field in bodyre.captures_iter(body.as_str()) {
                        if i > 0 {
                            method.push_str(&format!(", "));
                        }
                        if let (Some(field), Some(ty)) = (field.get(3), field.get(1)) {
                            // let ty = regex::Regex::new(r"(^|\s)\s*_\w+_\s*\s(\s|$)").unwrap().replace_all(ty.as_str().trim(), "");
                            let ty = regex::Regex::new(r"_In_opt_z_|_Out_opt_|_Out_Opt_|_In_z_|_In_opt_|_In_|_Out_").unwrap().replace_all(ty.as_str().trim(), "");
                            // known_types
                            let rty = to_rust_type(ty.as_ref().trim());
                            if let Some(cap) = opt_re.captures(&rty) {
                                known_types.insert(cap.get(1).unwrap().as_str().to_string());
                            }
                            // prefix _ avoids problems with unused linter
                            method.push_str(&format!("_{}: {}", to_snake_case(field.as_str()), rty));
                        }
                        i+= 1;
                    }
                    method.push_str(&format!(") -> {};\n", to_rust_type(ret.as_str().trim())));
                    if ty == 1 {
                        prototypes.insert(name.as_str().to_string(), method);
                    } else {
                        if idx == 1 { &mut callbacks } else { &mut maybe_callbacks }.insert(name.as_str().to_string(), method);
                    }
                }
            }
        }
        // println!("{:?}", res);
    }

    let mut methods_by_clsid_by_iid = HashMap::new();
    for m in &mapping.mappings {
        let Some(vtable_index) = m.vtable_index else {
            continue;
        };
        if vtable_index < 3  {
            continue;
        }
        let methods_by_iid = methods_by_clsid_by_iid.entry(&m.clsid).or_insert_with(||HashMap::new());
        let methods = methods_by_iid.entry(&m.iid).or_insert_with(||Vec::new());
        if methods.len() < (vtable_index + 1) as usize {
            methods.resize((vtable_index + 1) as usize, String::new());
        }
        let f = &m.function;
        if let Some(prot) = &prototypes.get(f) {
            methods[vtable_index as usize] = prot.to_string();
            prototypes.remove_entry(f);
            maybe_callbacks.remove_entry(f);
        } else if methods[vtable_index as usize].is_empty() {
            methods[vtable_index as usize] = format!("// {}\npub unsafe fn __reserved_slot_{}(&self);\n", f, vtable_index);
        }
    }
    for (clsid, methods_by_iid) in methods_by_clsid_by_iid {
        println!("// Class {}", clsid);
        for(iid, methods) in methods_by_iid {
            println!("// IID {}", iid);
            println!("#[interface(\"{}\")]", iid.strip_prefix("_GUID_").unwrap().replace("_", "-"));
            println!("pub unsafe trait {} : IUnknown {{", iid);
            
            for i in 3..methods.len() {
                let val = &methods[i];
                if val.is_empty() {
                    println!("unsafe fn __reserved_slot_{}(&self);", i)
                } else {
                    print!("{}", val);
                }
            }
            println!("}}");
        }
    }
    println!("// Callback Types");
    for (_, val) in &callbacks {
        print!("{}", val);
    }

    println!("// Maybe Callback Types");
    for (_, val) in &maybe_callbacks {
        print!("{}", val);
    }

    println!("// Unresolved Prototypes");
    for (_, val) in prototypes {
        print!("{}", val);
    }

    Ok(())
}