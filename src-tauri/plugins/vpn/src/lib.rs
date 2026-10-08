//! The bridge to Quadra's Android code (android/): the VPN service, the core's data files,
//! the device's model. Everything is called from Rust; the webview never talks to it.

use serde::{de::DeserializeOwned, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

pub struct Vpn<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> Vpn<R> {
    /// Runs one command of the Kotlin plugin and waits for its answer.
    pub fn call<T: DeserializeOwned>(&self, command: &str, payload: impl Serialize) -> Result<T, String> {
        #[cfg(target_os = "android")]
        {
            self.handle.run_mobile_plugin(command, payload).map_err(|e| e.to_string())
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (command, payload);
            Err("this plugin only exists on Android".into())
        }
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("quadra-vpn")
        .setup(|app, _api| {
            #[cfg(target_os = "android")]
            let vpn = Vpn { handle: _api.register_android_plugin("dev.quadra.vpn", "VpnPlugin")? };
            #[cfg(not(target_os = "android"))]
            let vpn = Vpn::<R> { _runtime: std::marker::PhantomData };
            app.manage(vpn);
            Ok(())
        })
        .build()
}
