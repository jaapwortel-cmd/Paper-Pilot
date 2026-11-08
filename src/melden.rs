// Paper Pilot - iets laten zeggen door de schermlezer, zonder eigen stem.
//
// Een UI Automation-melding (UiaRaiseNotificationEvent, Windows 10 1709 en nieuwer): NVDA, JAWS en Verteller lezen die
// voor, zonder dat de focus verandert. Gebruikt bij het scannen met de camera ("schuif de brief naar links").

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Accessibility::*;

pub fn zeg(hwnd: HWND, tekst: &str) {
    if hwnd.is_null() || tekst.trim().is_empty() {
        return;
    }
    unsafe {
        let mut provider: *mut core::ffi::c_void = std::ptr::null_mut();
        if UiaHostProviderFromHwnd(hwnd, &mut provider) < 0 || provider.is_null() {
            return;
        }
        let w: Vec<u16> = tekst.encode_utf16().chain(std::iter::once(0)).collect();
        let id: Vec<u16> = "PaperPilot".encode_utf16().chain(std::iter::once(0)).collect();
        let b = windows_sys::Win32::Foundation::SysAllocString(w.as_ptr());
        let a = windows_sys::Win32::Foundation::SysAllocString(id.as_ptr());
        let _ = UiaRaiseNotificationEvent(provider, NotificationKind_Other, NotificationProcessing_ImportantMostRecent, b, a);
        windows_sys::Win32::Foundation::SysFreeString(b);
        windows_sys::Win32::Foundation::SysFreeString(a);
        // IUnknown::Release (derde functie in de tabel).
        let tabel = *(provider as *const *const usize);
        let release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32 = std::mem::transmute(*tabel.add(2));
        release(provider);
    }
}
