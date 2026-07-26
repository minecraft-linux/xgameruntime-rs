use std::{borrow::Cow, collections::HashSet, fs::File, io::Read};


fn to_rust_type(cpp_type: &str) -> Cow<str> {
    let st =     match cpp_type {
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
        "void" => "c_void",
        "char" => "c_char",
        _ => "",
    };
    if st.is_empty() {
        let array: regex::Regex = regex::Regex::new(r"^(const\s+)?(.+)\s*\*$").unwrap();
        if let Some(caps) = array.captures(cpp_type) {
            let base_type = caps.get(2).unwrap().as_str();
            let rust_base_type = to_rust_type(base_type);
            if caps.get(1).is_some() {
                return format!("*const {}", rust_base_type).into();
            } else {
                return format!("*mut {}", rust_base_type).into();
            }
        }
    } else {
        return st.into();
    }
    return cpp_type.into();
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
                print!("#[repr({})]\nenum {} {{", to_rust_type(ty.as_str()), name.as_str());
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
                println!("#[repr(C)]\nstruct {} {{", name.as_str());
                // println!("{}", body.as_str());
                for field in bodyre.captures_iter(body.as_str()) {
                    if let (Some(field), Some(ty)) = (field.get(3), field.get(1)) {
                        println!("{}: {},", field.as_str(), to_rust_type(ty.as_str().trim()));
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

    let re = regex::Regex::new(r"(\w+)\s+(\w+)\s*\((((\n|\s|//.*\n)*([\w\*]+\s*)+,?)+)[\n\s]*\)")?;
    let bodyre = regex::Regex::new(r"[\s\n]*(([\w\*]+\s+)+)(\w+)\s*")?;
    
    let mut known = HashSet::new();

    for res in re.captures_iter(&buf) {
        if let (Some(name), Some(body)) = (res.get(2), res.get(3)) {
            if known.insert(name.as_str()) {
                // TODO normalize the name + body so it follows naming conventions
                print!("pub unsafe fn {} (self: &Self", to_snake_case(name.as_str()));
                // println!("{}", body.as_str());
                // let mut i = 0;
                for field in bodyre.captures_iter(body.as_str()) {
                    // if i > 0 {
                        print!(", ");
                    // }
                    if let (Some(field), Some(ty)) = (field.get(3), field.get(1)) {
                        print!("{}: {}", to_snake_case(field.as_str()), to_rust_type(ty.as_str().trim()));
                    }
                    //i+= 1;
                }
                println!(");");
            }
        }
        // println!("{:?}", res);
    }


    Ok(())
}