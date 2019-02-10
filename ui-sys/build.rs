extern crate bindgen;
extern crate cmake;
extern crate git2;
extern crate pkg_config;

use bindgen::Builder as BindgenBuilder;
use cmake::Config as CmakeConfig;
use pkg_config::probe_library;

use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
enum Target {
    Apple,
    Msvc,
    Linux,
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
        } else if target.contains("linux") {
            Target::Linux
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
        let repo = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let repo = match git2::Repository::open(&repo) {
            Ok(repo) => repo,
            Err(e) => {
                println!("cargo:warning={}{}", "Failed to open ui-sys repo: ", e);
                return;
            }
        };

        let mut submodule = match repo.find_submodule("libui") {
            Ok(s) => s,
            Err(e) => {
                println!("cargo:warning={}{}", "Failed to open libui submodule: ", e);
                return;
            }
        };

        if let Err(e) = submodule.update(true, None) {
            println!("cargo:warning={}{}", "Failed to update libui submodule: ", e);
            return;
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
fn build_native(target: Target, build_static: bool) -> PathBuf {
    let mut cfg = CmakeConfig::new("libui");
    cfg.build_target("").profile("release");

    if build_static {
        cfg.define("BUILD_SHARED_LIBS", "OFF");
    }

    if target.is_apple() {
        cfg.cxxflag("--stdlib=libc++");

        // FIXME: workaround for https://github.com/andlabs/libui/issues/439
        //cfg.cxxflag("-Wno-c++11-narrowing");
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
fn native_dst(target: Target, build_static: bool) -> PathBuf {
    if with_feature("build") {
        build_native(target, build_static)
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

fn pkg_config(target: Target) {
    match target {
        Target::Linux => {
            probe_library("gtk+-3.0").expect("Failed to probe gtk3");
        }
        _ => unimplemented!(),
    }
}

fn link_native() {
    let build_static = true;

    let target = Target::determine();
    let dst = native_dst(target, build_static);
    let libname = native_libname(target);

    println!("cargo:rustc-link-search=native={}", dst.display());
    if build_static {
        pkg_config(target);
        println!("cargo:rustc-link-lib=static={}", libname);
    } else {
        println!("cargo:rustc-link-lib={}", libname);
    }
}

fn main() {
    fetch_submodule();
    generate_bindings();
    link_native();
}
