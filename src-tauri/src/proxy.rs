//! The proxy the OS would use for a URL, for the in-app updater.
//!
//! The updater's HTTP client (reqwest) only reads proxy environment variables
//! and fixed system proxies; it can't evaluate a proxy auto-config (PAC)
//! script. Company networks often only reach the internet through a proxy
//! chosen by PAC, and apps started from Finder / Explorer don't get the
//! shell's `HTTPS_PROXY`, so the update check fails there with "error sending
//! request". Asking the OS (CFNetwork / WinHTTP), which runs the PAC script,
//! gives the same answer the browser uses.

/// `http://host:port` to reach `url` through, or None for a direct connection
/// (or when the OS can't tell).
pub fn for_url(url: &str) -> Option<String> {
    imp::for_url(url)
}

/// Picks the proxy for `https` from a WinHTTP / PAC style list:
/// "host:port", "https=host:port;http=host:port", "a:1; b:2" (first wins),
/// "PROXY host:port; DIRECT".
pub fn pick(list: &str) -> Option<String> {
    let entries: Vec<&str> = list
        .split([';', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let scheme_specific = entries.iter().find_map(|e| e.strip_prefix("https="));
    let any = entries.iter().copied().find(|e| {
        !e.contains('=') && !e.eq_ignore_ascii_case("PROXY") && !e.eq_ignore_ascii_case("DIRECT")
    });
    let first = match entries.first() {
        // PAC style: a leading DIRECT means "no proxy".
        Some(e) if e.eq_ignore_ascii_case("DIRECT") => return None,
        _ => scheme_specific.or(any)?,
    };
    let host_port = first.trim_start_matches("http://");
    Some(format!("http://{host_port}"))
}

#[cfg(target_os = "macos")]
mod imp {
    use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
    use core_foundation_sys::base::{kCFAllocatorDefault, CFIndex, CFRelease, CFRetain, CFTypeRef};
    use core_foundation_sys::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
    use core_foundation_sys::error::CFErrorRef;
    use core_foundation_sys::number::{kCFNumberSInt64Type, CFNumberGetValue, CFNumberRef};
    use core_foundation_sys::runloop::{
        CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRemoveSource, CFRunLoopRunInMode,
        CFRunLoopSourceRef, CFRunLoopStop,
    };
    use core_foundation_sys::string::{
        kCFStringEncodingUTF8, CFStringCreateWithBytes, CFStringGetCString, CFStringRef,
    };
    use core_foundation_sys::url::{CFURLCreateWithString, CFURLRef};
    use std::ffi::c_void;

    #[repr(C)]
    struct CFStreamClientContext {
        version: CFIndex,
        info: *mut c_void,
        retain: *const c_void,
        release: *const c_void,
        copy_description: *const c_void,
    }

    type PacCallback = extern "C" fn(client: *mut c_void, proxies: CFArrayRef, error: CFErrorRef);

    #[link(name = "CFNetwork", kind = "framework")]
    unsafe extern "C" {
        fn CFNetworkCopySystemProxySettings() -> CFDictionaryRef;
        fn CFNetworkCopyProxiesForURL(url: CFURLRef, settings: CFDictionaryRef) -> CFArrayRef;
        fn CFNetworkExecuteProxyAutoConfigurationURL(
            pac: CFURLRef,
            target: CFURLRef,
            cb: PacCallback,
            ctx: *mut CFStreamClientContext,
        ) -> CFRunLoopSourceRef;
        static kCFProxyTypeKey: CFStringRef;
        static kCFProxyHostNameKey: CFStringRef;
        static kCFProxyPortNumberKey: CFStringRef;
        static kCFProxyAutoConfigurationURLKey: CFStringRef;
        static kCFProxyTypeNone: CFStringRef;
        static kCFProxyTypeHTTP: CFStringRef;
        static kCFProxyTypeHTTPS: CFStringRef;
        static kCFProxyTypeAutoConfigurationURL: CFStringRef;
    }

    /// Releases a +1 Core Foundation object when dropped.
    struct Owned(CFTypeRef);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0) }
            }
        }
    }

    fn cfstr(s: &str) -> Owned {
        Owned(unsafe {
            CFStringCreateWithBytes(
                kCFAllocatorDefault,
                s.as_ptr(),
                s.len() as CFIndex,
                kCFStringEncodingUTF8,
                0,
            )
        } as CFTypeRef)
    }

    fn to_string(s: CFStringRef) -> Option<String> {
        if s.is_null() {
            return None;
        }
        let mut buf = [0i8; 512];
        let ok = unsafe {
            CFStringGetCString(
                s,
                buf.as_mut_ptr(),
                buf.len() as CFIndex,
                kCFStringEncodingUTF8,
            )
        };
        (ok != 0).then(|| {
            unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        })
    }

    fn same(a: CFTypeRef, b: CFStringRef) -> bool {
        !a.is_null() && to_string(a as CFStringRef) == to_string(b)
    }

    extern "C" fn on_pac(client: *mut c_void, proxies: CFArrayRef, _error: CFErrorRef) {
        let out = client as *mut CFArrayRef;
        unsafe {
            if !proxies.is_null() {
                CFRetain(proxies as CFTypeRef);
                *out = proxies;
            }
            CFRunLoopStop(CFRunLoopGetCurrent());
        }
    }

    /// Runs the PAC script at `pac` for `target`, waiting up to 10 s.
    fn run_pac(pac: CFURLRef, target: CFURLRef) -> Option<Owned> {
        let mut result: CFArrayRef = std::ptr::null();
        let mut ctx = CFStreamClientContext {
            version: 0,
            info: &mut result as *mut CFArrayRef as *mut c_void,
            retain: std::ptr::null(),
            release: std::ptr::null(),
            copy_description: std::ptr::null(),
        };
        let mode = cfstr("FeedabilityPAC");
        unsafe {
            let source = CFNetworkExecuteProxyAutoConfigurationURL(pac, target, on_pac, &mut ctx);
            if source.is_null() {
                return None;
            }
            let rl = CFRunLoopGetCurrent();
            CFRunLoopAddSource(rl, source, mode.0 as CFStringRef);
            CFRunLoopRunInMode(mode.0 as CFStringRef, 10.0, 0);
            CFRunLoopRemoveSource(rl, source, mode.0 as CFStringRef);
            CFRelease(source as CFTypeRef);
        }
        (!result.is_null()).then(|| Owned(result as CFTypeRef))
    }

    /// The first usable entry of a CFNetwork proxy list; PAC entries are run.
    fn first_proxy(list: CFArrayRef, target: CFURLRef, depth: u8) -> Option<String> {
        let n = unsafe { CFArrayGetCount(list) };
        for i in 0..n {
            let entry = unsafe { CFArrayGetValueAtIndex(list, i) } as CFDictionaryRef;
            let get = |k: CFStringRef| unsafe { CFDictionaryGetValue(entry, k as *const c_void) };
            let kind = get(unsafe { kCFProxyTypeKey });
            if same(kind, unsafe { kCFProxyTypeNone }) {
                return None;
            }
            if same(kind, unsafe { kCFProxyTypeHTTP }) || same(kind, unsafe { kCFProxyTypeHTTPS }) {
                let host = to_string(get(unsafe { kCFProxyHostNameKey }) as CFStringRef)?;
                let port_ref = get(unsafe { kCFProxyPortNumberKey }) as CFNumberRef;
                let mut port: i64 = 8080;
                if !port_ref.is_null() {
                    unsafe {
                        CFNumberGetValue(
                            port_ref,
                            kCFNumberSInt64Type,
                            &mut port as *mut i64 as *mut c_void,
                        )
                    };
                }
                return Some(format!("http://{host}:{port}"));
            }
            if same(kind, unsafe { kCFProxyTypeAutoConfigurationURL }) && depth == 0 {
                let pac = get(unsafe { kCFProxyAutoConfigurationURLKey }) as CFURLRef;
                if pac.is_null() {
                    continue;
                }
                let resolved = run_pac(pac, target)?;
                return first_proxy(resolved.0 as CFArrayRef, target, depth + 1);
            }
        }
        None
    }

    pub fn for_url(url: &str) -> Option<String> {
        let s = cfstr(url);
        let target = Owned(unsafe {
            CFURLCreateWithString(kCFAllocatorDefault, s.0 as CFStringRef, std::ptr::null())
        } as CFTypeRef);
        if target.0.is_null() {
            return None;
        }
        let settings = Owned(unsafe { CFNetworkCopySystemProxySettings() } as CFTypeRef);
        if settings.0.is_null() {
            return None;
        }
        let list = Owned(unsafe {
            CFNetworkCopyProxiesForURL(target.0 as CFURLRef, settings.0 as CFDictionaryRef)
        } as CFTypeRef);
        if list.0.is_null() {
            return None;
        }
        first_proxy(list.0 as CFArrayRef, target.0 as CFURLRef, 0)
    }
}

#[cfg(windows)]
mod imp {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{GlobalFree, HGLOBAL};
    use windows::Win32::Networking::WinHttp::*;

    fn take(p: PWSTR) -> Option<String> {
        if p.is_null() {
            return None;
        }
        let s = unsafe { p.to_string() }.ok();
        let _ = unsafe { GlobalFree(Some(HGLOBAL(p.0 as _))) };
        s.filter(|s| !s.is_empty())
    }

    /// Runs auto-detection / the PAC script through WinHTTP.
    fn auto(url: &str, config_url: Option<&str>, detect: bool) -> Option<Option<String>> {
        let agent: Vec<u16> = "Feedability\0".encode_utf16().collect();
        let session = unsafe {
            WinHttpOpen(
                PCWSTR(agent.as_ptr()),
                WINHTTP_ACCESS_TYPE_NO_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            )
        };
        if session.is_null() {
            return None;
        }
        let pac: Option<Vec<u16>> = config_url.map(|u| u.encode_utf16().chain([0]).collect());
        let mut opts = WINHTTP_AUTOPROXY_OPTIONS {
            dwFlags: if pac.is_some() {
                WINHTTP_AUTOPROXY_CONFIG_URL
            } else {
                0
            } | if detect {
                WINHTTP_AUTOPROXY_AUTO_DETECT
            } else {
                0
            },
            dwAutoDetectFlags: if detect {
                WINHTTP_AUTO_DETECT_TYPE_DHCP | WINHTTP_AUTO_DETECT_TYPE_DNS_A
            } else {
                0
            },
            lpszAutoConfigUrl: pac.as_ref().map_or(PCWSTR::null(), |p| PCWSTR(p.as_ptr())),
            fAutoLogonIfChallenged: true.into(),
            ..Default::default()
        };
        let target: Vec<u16> = url.encode_utf16().chain([0]).collect();
        let mut info = WINHTTP_PROXY_INFO::default();
        let ok = unsafe {
            WinHttpGetProxyForUrl(session, PCWSTR(target.as_ptr()), &mut opts, &mut info)
        }
        .is_ok();
        let _ = unsafe { WinHttpCloseHandle(session) };
        if !ok {
            return None;
        }
        let proxy = take(info.lpszProxy);
        let _ = take(info.lpszProxyBypass);
        Some(if info.dwAccessType == WINHTTP_ACCESS_TYPE_NAMED_PROXY {
            proxy.and_then(|p| super::pick(&p))
        } else {
            None
        })
    }

    pub fn for_url(url: &str) -> Option<String> {
        let mut cfg = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
        unsafe { WinHttpGetIEProxyConfigForCurrentUser(&mut cfg) }.ok()?;
        let config_url = take(cfg.lpszAutoConfigUrl);
        let fixed = take(cfg.lpszProxy);
        let _ = take(cfg.lpszProxyBypass);
        if config_url.is_some() || cfg.fAutoDetect.as_bool() {
            if let Some(result) = auto(url, config_url.as_deref(), cfg.fAutoDetect.as_bool()) {
                return result;
            }
        }
        fixed.and_then(|p| super::pick(&p))
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod imp {
    pub fn for_url(_url: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_from_lists() {
        assert_eq!(
            pick("10.0.0.1:8080").as_deref(),
            Some("http://10.0.0.1:8080")
        );
        assert_eq!(pick("http=a:1;https=b:2").as_deref(), Some("http://b:2"));
        assert_eq!(pick("a:1; b:2").as_deref(), Some("http://a:1"));
        assert_eq!(pick("PROXY a:1; PROXY b:2").as_deref(), Some("http://a:1"));
        assert_eq!(pick("DIRECT"), None);
        assert_eq!(pick(""), None);
    }

    /// On this machine, whatever the OS says must not panic; when a proxy is
    /// configured it should come back as a URL the updater accepts.
    #[test]
    fn asks_the_os() {
        if let Some(p) = for_url("https://github.com/") {
            assert!(
                p.starts_with("http://") && p.rsplit(':').next().unwrap().parse::<u16>().is_ok(),
                "{p}"
            );
        }
    }
}
