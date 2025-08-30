extern crate bindgen;

use bindgen::callbacks::{DeriveInfo, MacroParsingBehavior, ParseCallbacks, TypeKind};
use cmake::{self, Config};
use std::collections::HashSet;
use std::env;
use std::path::PathBuf;

const IGNORE_MACROS: [&str; 5] = [
    "FP_NAN",
    "FP_INFINITE",
    "FP_NORMAL",
    "FP_SUBNORMAL",
    "FP_ZERO",
];

const SERDE_TYPES: [&str; 8] = [
    // Top-level event types
    "dxf_trade_t",
    "dxf_quote_t",
    "dxf_summary",
    // "dxf_profile",
    // "dxf_order_t": skipped. anonymous union can't be supportd
    // "dxf_time_and_sale". Can't serialize raw C strings
    "dxf_candle_t",
    // "dxf_trade_eth_t",
    // "dx_spread_order",
    "dxf_greeks",
    "dx_theo_price",
    "dxf_underlying",
    "dxf_series",
    // "dxf_configuration",
];

// Types that contain function pointers or unions and should not derive PartialEq
const NO_PARTIALEQ_TYPES: [&str; 7] = [
    "__darwin_pthread_handler_rec",
    "__sbuf",
    "__sFILE",
    "_RuneLocale",
    "dxf_order_t__bindgen_ty_1",
    "dxf_order_t",
    "dxf_time_and_sale_t__bindgen_ty_1",
];

#[derive(Debug)]
struct CustomParser {
    ignore_macros: HashSet<String>,
    serde_types: HashSet<String>,
    no_partialeq_types: HashSet<String>,
}

impl ParseCallbacks for CustomParser {
    fn will_parse_macro(&self, name: &str) -> MacroParsingBehavior {
        if self.ignore_macros.contains(name) {
            MacroParsingBehavior::Ignore
        } else {
            MacroParsingBehavior::Default
        }
    }

    fn add_derives(&self, info: &DeriveInfo<'_>) -> Vec<String> {
        let mut derives = vec![];

        // Don't add PartialEq for unions or types in the blocklist
        let is_union = matches!(info.kind, TypeKind::Union);
        if !is_union && !self.no_partialeq_types.contains(info.name) {
            derives.push("PartialEq".to_string());
        }

        // Add serde derives if enabled
        let enabled = std::env::var("CARGO_FEATURE_SERDE").unwrap_or("0".to_string()) == "1";
        if enabled && self.serde_types.contains(info.name) {
            eprintln!("Adding Serialize/Deserialize");
            derives.push("Serialize".to_string());
            derives.push("Deserialize".to_string());
        }

        derives
    }
}

impl CustomParser {
    fn new() -> Self {
        let ignore_macros = IGNORE_MACROS.iter().map(|s| s.to_string()).collect();
        let serde_types = SERDE_TYPES.iter().map(|s| s.to_string()).collect();
        let no_partialeq_types = NO_PARTIALEQ_TYPES.iter().map(|s| s.to_string()).collect();
        Self {
            ignore_macros,
            serde_types,
            no_partialeq_types,
        }
    }
}

fn main() {
    let dst = Config::new("dxfeed-c-api")
        .define("DISABLE_TLS", "ON")
        .define("BUILD_STATIC_LIBS", "ON")
        .define("CMAKE_POLICY_VERSION_MINIMUM", "3.5")
        .build();

    println!("cargo:rustc-link-search=native={}", dst.display());

    // TODO: Investigate whether `cc` crate can help with this logic
    #[cfg(unix)]
    {
        println!("cargo:rustc-link-search=native={}/build", dst.display());
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            println!("cargo:rustc-link-lib=c++");
        }
        #[cfg(all(not(target_os = "macos"), not(target_os = "ios")))]
        {
            println!("cargo:rustc-link-lib=stdc++");
        }
    }

    #[cfg(windows)]
    {
        println!(
            "cargo:rustc-link-search=native={}/build/Debug",
            dst.display()
        );
        println!(
            "cargo:rustc-link-search=native={}/build/Release",
            dst.display()
        );
    }

    // println!("cargo:rustc-link-search={}", dst.display());

    let profile = std::env::var("PROFILE").unwrap();
    let suffix = if profile == "debug" { "d" } else { "" };
    println!("cargo:rustc-link-lib=static={}{}", "DXFeed", suffix);

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        .blocklist_file(r".*c?math.*")
        .blocklist_function("wcstold")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(CustomParser::new()))
        .derive_partialeq(false) // Handle PartialEq manually in CustomParser
        // .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
