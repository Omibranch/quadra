# Third-party software and data

Quadra's own code is under the MIT licence (see `LICENSE`). The installers also carry the
following, each under its own terms.

| What | Where it comes from | Licence |
|---|---|---|
| **Xray-core** (`xray` / `xray.exe`), which makes every connection | <https://github.com/XTLS/Xray-core>, release named in `tools/xray.version`, downloaded unmodified and checksum-verified by `tools/fetch-xray.mjs` | MPL-2.0; its `LICENSE` file is installed next to the binary |
| **Wintun** (`wintun.dll`, Windows only), the tunnel driver used by the all-traffic mode | Shipped inside the Xray-core Windows archive; <https://www.wintun.net> | Prebuilt binaries licence, installed as `LICENSE-wintun.txt` |
| **geoip.dat**, **geosite.dat**, the address and domain lists used for routing rules and for placing servers on the map | Shipped inside the Xray-core archives | See the Xray-core project |
| **Natural Earth** country outlines (`tools/ne_110m_countries.geojson`), from which the dot map is generated | <https://www.naturalearthdata.com> | Public domain |
| **Tauri**, **Svelte**, **Vite** and the Rust crates listed in `src-tauri/Cargo.toml` | crates.io, npm | MIT / Apache-2.0 and similar, see each package |

The startup cube, the icons, the flags and the map rendering were made for this project.
