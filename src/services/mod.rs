#[cfg(feature = "ssr")]
pub mod projects;
#[cfg(feature = "ssr")]
pub mod title_generation;

#[cfg(feature = "ssr")]
pub use projects::*;
#[cfg(feature = "ssr")]
pub use title_generation::*;
