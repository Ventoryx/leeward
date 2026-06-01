cfg_linux! {
    pub mod linux;
}

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "windows")]
#[cfg_attr(docsrs, doc(cfg(target_os = "windows")))]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("evalbox-sandbox supports only Linux and Windows.");
