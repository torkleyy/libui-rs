extern crate bindgen;
extern crate cmake;

use bindgen::Builder as BindgenBuilder;
use cmake::Config;

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy)]
enum Target {
    Apple,
    Msvc,
    Other,
}

impl Target {
    pub fn is_apple(self) -> bool {
        match self {
            Target::Apple => true,
            _ => false,
        }
    }

    pub fn is_msvc(self) -> bool {
        match self {
            Target::Msvc => true,
            _ => false,
        }
    }

    pub fn determine() -> Self {
        let target = env::var("TARGET").unwrap();

        if target.contains("msvc") {
            Target::Msvc
        } else if target.contains("apple") {
            Target::Apple
        } else {
            Target::Other
        }
    }
}

/// Checks if a given feature `s` is enabled.
fn with_feature(s: &str) -> bool {
    let var = format!("CARGO_FEATURE_{}", s.to_uppercase());

    env::var(&var).is_ok()
}

fn fetch_submodule() {
    if with_feature("fetch") {
        // Init or update the submodule with libui if needed
        if !Path::new("libui/.git").exists() {
            Command::new("git")
                .args(&["version"])
                .status()
                .expect("Git does not appear to be installed. Error");
            Command::new("git")
                .args(&["submodule", "update", "--init"])
                .status()
                .expect("Unable to init libui submodule. Error");
        } else {
            Command::new("git")
                .args(&["submodule", "update", "--recursive"])
                .status()
                .expect("Unable to update libui submodule. Error");
        }
    }
}

fn generate_bindings() {
    let bindings = BindgenBuilder::default()
        .header("wrapper.h")
        .opaque_type("max_align_t") // For some reason this ends up too large
        //.rustified_enum(".*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings");
}

/// Builds the native library, returning the output dir.
fn build_native(target: Target) -> PathBuf {
    let mut cfg = Config::new("libui");
    cfg.build_target("").profile("release");

    if target.is_apple() {
        cfg.cxxflag("--stdlib=libc++");
    }

    let mut dst = cfg.build();

    let mut postfix = Path::new("build").join("out");

    if target.is_msvc() {
        postfix = postfix.join("Release");
    }

    dst.push(&postfix);

    dst
}

/// Builds the native library if the `build` feature is enabled.
/// Returns the output directory if there was a fresh build.
/// Otherwise, returns `./lib`.
fn native_dst(target: Target) -> PathBuf {
    if with_feature("build") {
        build_native(target)
    } else {
        // TODO: should this be pwd/lib or CARGO_MANIFEST_DIR/lib?
        let mut dst = env::current_dir().expect("Unable to retrieve current directory location.");
        dst.push("lib");

        dst
    }
}

fn native_libname(target: Target) -> &'static str {
    if target.is_msvc() {
        "libui"
    } else {
        "ui"
    }
}

fn link_native() {
    let target = Target::determine();
    let dst = native_dst(target);
    let libname = native_libname(target);

    println!("cargo:rustc-link-search=native={}", dst.display());
    println!("cargo:rustc-link-lib={}", libname);
}

fn main() {
    fetch_submodule();
    generate_bindings();
    link_native();
}
