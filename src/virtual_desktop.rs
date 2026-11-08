use crate::error::{MsError, Result};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::core::BOOL;
use windows_sys::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::UI::Shell::VirtualDesktopManager;
use windows_sys::core::GUID;

const IID_IVIRTUAL_DESKTOP_MANAGER: GUID = GUID::from_u128(0xa5cd92ff_29be_454c_8d04_d82879fb3f1b);

#[repr(C)]
struct Vtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(*mut IVdm, *const GUID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut IVdm) -> u32,
    release: unsafe extern "system" fn(*mut IVdm) -> u32,
    // IVirtualDesktopManager
    is_window_on_current: unsafe extern "system" fn(*mut IVdm, HWND, *mut BOOL) -> i32,
    get_window_desktop_id: unsafe extern "system" fn(*mut IVdm, HWND, *mut GUID) -> i32,
    move_window_to_desktop: unsafe extern "system" fn(*mut IVdm, HWND, *const GUID) -> i32,
}

#[repr(C)]
struct IVdm { vtbl: *const Vtbl }

pub struct VirtualDesktopManager { ptr: *mut IVdm }

unsafe impl Send for VirtualDesktopManager {}
unsafe impl Sync for VirtualDesktopManager {}

impl VirtualDesktopManager {
    pub fn new() -> Result<Self> {
        unsafe {
            let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
            // S_OK or S_FALSE (already init) acceptable
            if hr < 0 && hr != 1 { // RPC_E_CHANGED_MODE = 0x80010106? just proceed
            }
            let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let hr = CoCreateInstance(
                &VirtualDesktopManager,
                std::ptr::null_mut(),
                CLSCTX_ALL,
                &IID_IVIRTUAL_DESKTOP_MANAGER,
                &mut ptr,
            );
            if hr < 0 {
                return Err(MsError::Com(format!("CoCreateInstance failed hr={hr}")));
            }
            Ok(Self { ptr: ptr as *mut IVdm })
        }
    }

    pub fn current_desktop_id(&self, hwnd: HWND) -> Result<GUID> {
        unsafe {
            let mut id = GUID::from_u128(0);
            let hr = ((*(*self.ptr).vtbl).get_window_desktop_id)(self.ptr, hwnd, &mut id);
            if hr < 0 { return Err(MsError::Com(format!("GetWindowDesktopId hr={hr}"))); }
            Ok(id)
        }
    }

    pub fn is_on_current(&self, hwnd: HWND) -> Result<bool> {
        unsafe {
            let mut b: BOOL = 0;
            let hr = ((*(*self.ptr).vtbl).is_window_on_current)(self.ptr, hwnd, &mut b);
            if hr < 0 { return Err(MsError::Com(format!("IsWindowOnCurrent hr={hr}"))); }
            Ok(b != 0)
        }
    }

    pub fn move_to_desktop(&self, hwnd: HWND, desktop_id: &GUID) -> Result<()> {
        unsafe {
            let hr = ((*(*self.ptr).vtbl).move_window_to_desktop)(self.ptr, hwnd, desktop_id);
            if hr < 0 { return Err(MsError::Com(format!("MoveWindowToDesktop hr={hr}"))); }
            Ok(())
        }
    }
}

impl Drop for VirtualDesktopManager {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                ((*(*self.ptr).vtbl).release)(self.ptr);
            }
        }
    }
}