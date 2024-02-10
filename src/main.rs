use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use windows::{
    core::{GUID, HSTRING, PCWSTR, PWSTR},
    Win32::{
        Foundation::{ERROR_SUCCESS, HANDLE, INVALID_HANDLE_VALUE, WIN32_ERROR},
        NetworkManagement::WiFi::{
            WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory, WlanGetProfile,
            WlanGetProfileList, WlanOpenHandle, WLAN_API_VERSION, WLAN_INTERFACE_INFO,
            WLAN_PROFILE_GET_PLAINTEXT_KEY, WLAN_PROFILE_INFO_LIST,
        },
    },
};
use windows::Win32::NetworkManagement::WiFi::{WLAN_API_VERSION_2_0, WLAN_INTERFACE_INFO_LIST};

//Getting an open handle to the WLAN interface
fn open_wlan_handle(api_version: u32) -> Result<HANDLE, windows::core::Error>{
    let mut negotiated_version = 0;
    let mut wlan_handle = INVALID_HANDLE_VALUE;

    let result = unsafe { // Call the WlanOpenHandle function
        WlanOpenHandle(
            api_version,
            None,
            &mut negotiated_version,
            &mut wlan_handle,
        )
    };
    WIN32_ERROR(result).ok()?; // Convert the result to a Result type

    Ok(wlan_handle)
}

//function to enum our WLAN interfaces
fn enum_wlan_interfaces(wlan_handle: HANDLE) -> Result<*mut WLAN_INTERFACE_INFO_LIST, windows::core::Error> {

    let mut interface_ptr = std::ptr::null_mut(); // Pointer to the interface ptr
    let result = unsafe { WlanEnumInterfaces(wlan_handle, None, &mut interface_ptr) }; // Call the WlanEnumInterfaces function
    WIN32_ERROR(result).ok()?; // Convert the result to a Result type

    Ok(interface_ptr) // Return the pointer to the interface
}

//function to get the profile list of the WLAN interface
fn get_profile_list(wlan_handle: HANDLE, interface_guid: &GUID) -> Result<*const WLAN_PROFILE_INFO_LIST, windows::core::Error> {
    let mut profile_list_ptr = std::ptr::null_mut(); // Pointer to the profile list
    let result = unsafe { WlanGetProfileList(wlan_handle, interface_guid, None, &mut profile_list_ptr) }; // Call the WlanGetProfileList function
    WIN32_ERROR(result).ok()?; // Convert the result to a Result ty  pe

    Ok(profile_list_ptr) // Return the pointer to the profile list
}


// Function to parse a UTF-16 slice into an OsString
fn parse_utf16_slice(string_slice: &[u16]) -> Option<OsString>{
    let null_index = string_slice.iter().position(|c| c == 0)?; // Find the null terminator

    Some(OsString::from_wide(&string_slice[..null_index])) // Convert the slice to an OsString
}

fn main() {
    //Getting the wlan handle
    let wlan_handle = open_wlan_handle(WLAN_API_VERSION_2_0).expect("Failed to open WLAN handle");

    //Getting the wlan interface
    let interface_ptr = match enum_wlan_interfaces(wlan_handle){
        Ok(ptr) => ptr,
        Err(e) => {
            eprintln!("Failed to enum WLAN interfaces: {:?}", e);
            unsafe { WlanCloseHandle(wlan_handle, None) };
            std::process::exit(1);
        }
    };

    //Extracting the interface list
    let interface_list = unsafe{
        std::slice::from_raw_parts(
            (*interface_ptr).InterfaceInfo.as_ptr(),
            (*interface_ptr).dwNumberOfItems as usize)
    };

    for interface_info in interface_list{ // Iterate over the interface list
        let interface_description = match parse_utf16_slice(interface_info.strInterfaceDescription.as_slice()){ // Parse the interface description
            Some(name) => name,
            None => {
                eprintln!("Failed to parse interface description");
                continue;
            }
        };

        //For every interface we get the profile list
        let profile_list_ptr = match get_profile_list(wlan_handle, &interface_info.InterfaceGuid){
            Ok(ptr) => ptr,
            Err(e) => {
                eprintln!("Failed to get profile list: {:?}", e);
                continue;
            }
        };
    }
}
