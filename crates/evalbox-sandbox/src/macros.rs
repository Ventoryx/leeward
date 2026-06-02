/// Wraps items with `#[cfg(target_os = "linux")]` and docs.rs annotation.
///
/// Items annotated with this macro will only be compiled on Linux and will
/// show the platform gate in generated documentation on docs.rs.
macro_rules! cfg_linux {
    ($($item:item)*) => {
        $(
            #[cfg(target_os = "linux")]
            #[cfg_attr(docsrs, doc(cfg(target_os = "linux")))]
            $item
        )*
    }
}
