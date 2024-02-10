use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use windows::{
    core::{GUID, HSTRING, PCWSTR, PWSTR},
    Data::Xml::Dom::{XmlDocument, XmlElement},
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
    let null_index = string_slice.iter().position(|c| c == &0)?; // Find the null terminator in the slice

    Some(OsString::from_wide(&string_slice[..null_index])) // Convert the slice to an OsString
}

// Function to load XML data from windows into an XmlDocument
fn load_xml_data(xml: &OsString) -> Result<XmlDocument, windows::core::Error>{ //
    let xml_document = XmlDocument::new()?;
    xml_document.LoadXml(&HSTRING::from(xml))?; // Load the XML data into the XmlDocument
    Ok(xml_document)
}

// Parsing the XML tree
fn traverse_xml_tree(xml: &XmlElement, node_path: &[&str]) -> Option<String> { // Function to traverse the XML tree
    let mut subtree_list = xml.childNodes().ok?; // Get the list of child nodes
    let last_node_name = node_path.last()?; // Get the last node name

    'node_traverse: for node in node_path{ // Iterate over the node path
        let node_name = OsString::from_wide(&node.encode_utf16().collect::<Vec<u16>>()); // Convert the node name to a wide string

        for subtree_value in &subtree_list{ // Iterate over the subtree list
            let element_name = match subtree_value.NodeName(){ // Get the name of the element
                Ok(name) => name,
                Err(_) => continue,
            };

            if element_name.to_os_string() == node_name{ // Check if the element name matches the node name
                if element_name.to_os_string().to_string_lossy.to_string() == last_node_name.to_string(){ // Check if the element name matches the last node name
                    return Some(subtree_value.InnerText().ok()?.to_string()); // Return the inner text of the element
                }

                subtree_list = subtree_value.childNodes().ok()?;
                continue 'node_traverse;
            }
        }
    }
    None
}

//Curving out the data in XML profile
fn get_profile_xml( // Function to get the profile XML
    handle: HANDLE,
    interface_guid: &GUID,
    profile_name: &OsString,
) -> Result<OsString, windows::core::Error>{
    let mut profile_xml_data = PWSTR::null(); // Pointer to the profile XML
    let mut profile_xml_flags = WLAN_PROFILE_GET_PLAINTEXT_KEY; // Flags for the profile XML
    let result = unsafe { // Call the WlanGetProfile function
        WlanGetProfile(handle, interface_guid, PCWSTR(HSTRING::from(profile_name).as_ptr(),
        ), None, &mut profile_xml_data, Some(&mut profile_xml_flags), None)
    };

    WIN32_ERROR(result).ok()?; // Convert the result to a Result type

    let xml_string = match unsafe { profile_xml_data.to_hstring() }{ // Convert the profile XML to an HSTRING
        Ok(data) => data,
        Err(_) => {
            unsafe { WlanFreeMemory(profile_xml_data.as_ptr().cast()) }; // Free the memory
            return Err(windows::core::Error::from(ERROR_SUCCESS)); // Return an error
        }
    };

    Ok(xml_string.to_os_string()); // Return the profile XML as an OsString
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

    //Iterating over the interface list
    for interface_info in interface_list{
        let interface_description = match parse_utf16_slice(interface_info.strInterfaceDescription.as_slice()){ // Parse the interface description
            Some(name) => name,
            None => {
                eprintln!("Failed to parse interface description");
                continue;
            }
        };

        //For every interface we get the profile list
        let wlan_profile_ptr = match get_profile_list(wlan_handle, &interface_info.InterfaceGuid){
            Ok(ptr) => ptr,
            Err(e) => {
                eprintln!("Failed to get profile list: {:?}", e);
                continue;
            }
        };

        //Extracting the profile list
        let wlan_profile_list = unsafe{
            std::slice::from_raw_parts(
                (*wlan_profile_ptr).ProfileInfo.as_ptr(),
                (*wlan_profile_ptr).dwNumberOfItems as usize)
        };

        //Iterating over the profile list
        for profile in wlan_profile_list{
            let profile_name = match parse_utf16_slice(profile.strProfileName.as_slice()){ // Parse the profile name
                Some(name) => name,
                None => {
                    eprintln!("Failed to parse profile name");
                    continue;
                }
            };

            //Windows isn't going to store your Wi-Fi passwords in a protected or encrypted manner, it will store them in plain text(XML) format
        }
    }
}
