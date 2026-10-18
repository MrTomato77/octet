# Third-party notices

Octet is MIT licensed (see LICENSE). It bundles the following.

## Interface assets (ui/vendor)

- **Phosphor Icons** 2.1.1 (font and icon outlines), MIT, Copyright (c) 2020-2021 Phosphor Icons. Full text: ui/vendor/phosphor/LICENSE.
- **morphicons** 2.0.0, MIT, Copyright (c) 2026 Guillermo. Full text: ui/vendor/morphicons/LICENSE.
- **Simple Icons** 16.35.0 (DNS provider logos), CC0-1.0. Full text: ui/vendor/logos/LICENSE.md. The logos are trademarks of their owners (Google, Cloudflare, Quad9, AdGuard, Cisco, NextDNS, Tailscale); they are shown only to identify which DNS service an address belongs to and imply no endorsement.

## Data

- **IEEE MA-L registry** (crates/octet/data/oui.txt), the public list of MAC address prefixes and the organizations they are assigned to, from https://standards-oui.ieee.org/. Names are shortened (legal suffixes dropped) by crates/octet/data/oui.py.

## Rust crates compiled into Octet.exe

Grouped by license as declared by each crate. License texts are in each crate's source on crates.io.

### (MIT OR Apache-2.0) AND Unicode-3.0

unicode-ident 1.0.27

### 0BSD OR MIT OR Apache-2.0

adler2 2.0.1

### Apache-2.0

tao 0.37.1

### Apache-2.0 / MIT

fnv 1.0.7

### Apache-2.0 AND MIT

dpi 0.1.2

### Apache-2.0 OR MIT

bit-set 0.8.0, bit-vec 0.8.0, ctor 1.0.13, equivalent 1.0.3, fastrand 2.5.0, idna_adapter 1.2.2, indexmap 1.9.3, indexmap 2.14.2, muda 0.20.0, pin-project-lite 0.2.17, rustc-hash 2.1.3, tauri 2.12.4, tauri-codegen 2.7.3, tauri-macros 2.7.3, tauri-runtime 2.12.2, tauri-runtime-wry 2.12.2, tauri-utils 2.10.2, utf8_iter 1.0.4, uuid 1.28.0, window-vibrancy 0.8.1, wry 0.57.0

### BSD-3-Clause

alloc-no-stdlib 3.0.0, alloc-stdlib 0.3.0

### BSD-3-Clause AND MIT

brotli 9.0.0

### BSD-3-Clause/MIT

brotli-decompressor 6.0.1

### CC0-1.0 OR MIT-0 OR Apache-2.0

dunce 1.0.5

### MIT

bytes 1.12.1, cargo_metadata 0.19.2, cfb 0.14.0, darling 0.24.1, darling_core 0.24.1, darling_macro 0.24.1, derive_more 2.1.1, derive_more-impl 2.1.1, dom_query 0.28.0, generic-array 0.14.7, ico 0.5.0, infer 0.22.0, mio 1.2.4, new_debug_unreachable 1.0.6, phf 0.13.1, phf_generator 0.13.1, phf_macros 0.13.1, phf_shared 0.13.1, plist 1.10.1, precomputed-hash 0.1.1, quick-xml 0.42.0, schemars 0.8.22, schemars 0.9.0, schemars 1.2.3, schemars_derive 0.8.22, simd-adler32 0.3.10, strsim 0.11.1, synstructure 0.14.0, tokio 1.53.3, tracing 0.1.44, tracing-core 0.1.36, urlpattern 0.6.0, webview2-com 0.39.1, webview2-com-macros 0.8.1, webview2-com-sys 0.39.1, winnow 1.0.4, zmij 1.0.23

### MIT OR Apache-2.0

anyhow 1.0.104, base64 0.23.1, bitflags 2.13.2, block-buffer 0.10.4, camino 1.2.6, cargo-platform 0.1.9, cfg-if 1.0.5, chrono 0.4.45, cookie 0.18.2, cpufeatures 0.2.17, crc32fast 1.5.2, crossbeam-channel 0.5.17, crossbeam-utils 0.8.23, crypto-common 0.1.7, defmt 1.2.0, defmt-macros 1.2.0, defmt-parser 1.1.0, deranged 0.5.9, digest 0.10.7, dirs 7.0.0, dirs-sys 0.5.0, displaydoc 0.2.7, dtoa 1.0.11, dyn-clone 1.0.20, erased-serde 0.4.10, fdeflate 0.3.7, flate2 1.1.10, form_urlencoded 1.2.2, getrandom 0.3.4, getrandom 0.4.3, glob 0.3.4, hashbrown 0.12.3, hashbrown 0.17.1, heck 0.5.0, hex 0.4.3, html5ever 0.39.0, http 1.5.0, idna 1.1.0, itoa 1.0.18, jsonptr 0.7.1, keyboard-types 0.8.3, libc 0.2.191, lock_api 0.4.14, log 0.4.34, markup5ever 0.39.0, mime 0.3.17, num-conv 0.2.2, num-traits 0.2.19, once_cell 1.21.4, parking_lot 0.12.5, parking_lot_core 0.9.12, percent-encoding 2.3.2, png 0.17.16, png 0.18.1, powerfmt 0.2.1, proc-macro2 1.0.107, quote 1.0.47, ref-cast 1.0.27, ref-cast-impl 1.0.27, regex 1.13.1, regex-automata 0.4.18, regex-syntax 0.8.11, scopeguard 1.2.0, semver 1.0.28, serde 1.0.229, serde-untagged 0.1.9, serde_core 1.0.229, serde_derive 1.0.229, serde_derive_internals 0.29.1, serde_json 1.0.154, serde_repr 0.1.21, serde_spanned 1.1.2, serde_with 3.24.0, serde_with_macros 3.24.0, serialize-to-javascript 0.1.2, serialize-to-javascript-impl 0.1.2, servo_arc 0.4.3, sha2 0.10.9, siphasher 1.0.4, smallvec 1.16.3, socket2 0.6.5, softbuffer 0.4.8, stable_deref_trait 1.2.1, string_cache 0.9.0, syn 2.0.119, syn 3.0.9, tendril 0.5.1, thiserror 2.0.21, thiserror-impl 2.0.21, time 0.3.55, time-core 0.1.9, time-macros 0.2.32, toml 1.1.8+spec-1.1.0, toml_datetime 1.1.2+spec-1.1.0, toml_parser 1.1.5+spec-1.1.0, toml_writer 1.1.3+spec-1.1.0, tray-icon 0.25.1, typeid 1.0.3, typenum 1.20.1, unicode-segmentation 1.13.3, url 2.5.8, web-time 1.1.0, web_atoms 0.2.6, windows 0.62.2, windows-collections 0.3.2, windows-core 0.62.2, windows-future 0.3.2, windows-implement 0.60.2, windows-interface 0.59.3, windows-link 0.2.1, windows-numerics 0.3.1, windows-result 0.4.1, windows-strings 0.5.1, windows-sys 0.61.2, windows-threading 0.2.1, windows-version 0.1.7

### MIT OR Apache-2.0 OR Zlib

raw-window-handle 0.6.2

### MIT OR Zlib OR Apache-2.0

miniz_oxide 0.8.9, miniz_oxide 0.9.1

### MIT/Apache-2.0

bitflags 1.3.2, bs58 0.5.1, ident_case 1.0.1, json-patch 4.2.0

### MPL-2.0

cssparser 0.37.0, cssparser-macros 0.7.1, dtoa-short 0.3.5, option-ext 0.2.0, selectors 0.38.0

### Unicode-3.0

icu_collections 2.3.0, icu_locale_core 2.3.0, icu_normalizer 2.3.0, icu_normalizer_data 2.3.0, icu_properties 2.3.0, icu_properties_data 2.3.0, icu_provider 2.3.1, litemap 0.8.3, potential_utf 0.1.6, tinystr 0.8.4, writeable 0.6.4, yoke 0.8.3, yoke-derive 0.8.4, zerofrom 0.1.8, zerofrom-derive 0.1.8, zerotrie 0.2.5, zerovec 0.11.8, zerovec-derive 0.11.6

### Unlicense OR MIT

aho-corasick 1.1.5, byteorder 1.5.0, jiff 0.2.38, jiff-core 0.1.1, jiff-tzdb 0.1.9, jiff-tzdb-platform 0.1.3, memchr 2.8.3, winapi-util 0.1.11

### Unlicense/MIT

same-file 1.0.6, walkdir 2.5.0

### Zlib

foldhash 0.2.0, zlib-rs 0.6.8

### Zlib OR Apache-2.0 OR MIT

tinyvec 1.13.3
