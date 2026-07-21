//! WP14 Q17 native critical-error surface — Rev C §8.5.
//!
//! Renderer OOM handling cannot depend on another swapchain frame. The two
//! shipping desktop platforms therefore use their synchronous system alert
//! APIs directly, behind a call-site-stable boundary that adds no dependency.

pub(crate) fn show_out_of_memory_alert(title: &str, message: &str) -> Result<(), String> {
    platform::show_alert(title, message)
}

#[cfg(target_os = "macos")]
mod platform {
    use std::{
        ffi::{c_char, c_void, CString},
        os::raw::c_ulong,
        ptr,
    };

    const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const CLOSE_BUTTON: &str = "CLOSE SOLAR SIM";

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(
            allocator: *const c_void,
            text: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        fn CFRelease(value: *const c_void);
        fn CFUserNotificationDisplayAlert(
            timeout: f64,
            flags: c_ulong,
            icon_url: *const c_void,
            sound_url: *const c_void,
            localization_url: *const c_void,
            alert_header: *const c_void,
            alert_message: *const c_void,
            default_button_title: *const c_void,
            alternate_button_title: *const c_void,
            other_button_title: *const c_void,
            response_flags: *mut c_ulong,
        ) -> i32;
    }

    struct CfString(*const c_void);

    impl CfString {
        fn new(value: &str) -> Result<Self, String> {
            let value = CString::new(value)
                .map_err(|_| "native alert text contains an interior NUL".to_string())?;
            // SAFETY: CoreFoundation copies the valid NUL-terminated UTF-8
            // bytes, and a null allocator selects the system default.
            let string = unsafe {
                CFStringCreateWithCString(ptr::null(), value.as_ptr(), CF_STRING_ENCODING_UTF8)
            };
            if string.is_null() {
                Err("CoreFoundation could not allocate alert text".into())
            } else {
                Ok(Self(string))
            }
        }
    }

    impl Drop for CfString {
        fn drop(&mut self) {
            // SAFETY: `CfString::new` accepts only owned create-rule
            // references, each released exactly once here.
            unsafe { CFRelease(self.0) };
        }
    }

    pub(super) fn show_alert(title: &str, message: &str) -> Result<(), String> {
        let title = CfString::new(title)?;
        let message = CfString::new(message)?;
        let button = CfString::new(CLOSE_BUTTON)?;
        let mut response = 0;
        // SAFETY: all CFString references remain alive for the synchronous
        // call; null optional URLs/buttons are accepted by this API. Flag 0
        // is CoreFoundation's documented stop-alert severity.
        let status = unsafe {
            CFUserNotificationDisplayAlert(
                0.0,
                0,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                title.0,
                message.0,
                button.0,
                ptr::null(),
                ptr::null(),
                &mut response,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!("CoreFoundation alert failed with status {status}"))
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::{ffi::c_void, ptr};

    const MB_OK: u32 = 0x0000_0000;
    const MB_ICONERROR: u32 = 0x0000_0010;
    const MB_TASKMODAL: u32 = 0x0000_2000;
    const MB_SETFOREGROUND: u32 = 0x0001_0000;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            window: *mut c_void,
            text: *const u16,
            caption: *const u16,
            kind: u32,
        ) -> i32;
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub(super) fn show_alert(title: &str, message: &str) -> Result<(), String> {
        let title = wide(title);
        let message = wide(message);
        // SAFETY: both UTF-16 buffers are NUL-terminated and remain alive for
        // this synchronous call. A null owner plus MB_TASKMODAL creates a
        // native task-modal surface independent of the stopped swapchain.
        let result = unsafe {
            MessageBoxW(
                ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR | MB_TASKMODAL | MB_SETFOREGROUND,
            )
        };
        if result == 0 {
            Err("User32 MessageBoxW failed".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    pub(super) fn show_alert(_title: &str, _message: &str) -> Result<(), String> {
        Err("native OOM alerts are supported only on the shipping macOS and Windows targets".into())
    }
}
