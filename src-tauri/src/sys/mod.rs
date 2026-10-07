//! Everything that differs between operating systems: the system proxy, elevation, the
//! clipboard, how the core process is tied to the app, where the machine id lives.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;
