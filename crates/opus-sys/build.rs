// SPDX-License-Identifier: MPL-2.0
use bindgen::callbacks::ParseCallbacks;
use std::path::{Path, PathBuf};

fn get_libopus_dir() -> PathBuf {
	match std::env::var("LIBOPUS_SRC").map(PathBuf::from) {
		Ok(dir) if dir.exists() => return dir,
		Ok(dir) => panic!(
			"given LIBOPUS_SRC directory ({}) does not exist!",
			dir.display()
		),
		_ => {}
	}
	match std::env::var("CARGO_MANIFEST_DIR")
		.map(PathBuf::from)
		.map(|path| path.join("libopus"))
	{
		Ok(dir) if dir.exists() => dir,
		Ok(dir) => panic!(
			"libopus source submodule ({}) doesn't exist!",
			dir.display()
		),
		_ => panic!("CARGO_MANIFEST_DIR not set!"),
	}
}

fn build_opus_with_cmake(libopus_dir: &Path) -> PathBuf {
	println!(
		"cargo:info=Building libopus from {} with cmake.",
		libopus_dir.display()
	);
	cmake::Config::new(libopus_dir)
		.define(
			"OPUS_DRED",
			std::env::var("CARGO_FEATURE_DRED")
				.map(|_| "True")
				.unwrap_or("False"),
		)
		.build()
}

fn link_opus(libopus_build_dir: &Path) {
	println!(
		"cargo:info=Linking libopus from {}",
		libopus_build_dir.display()
	);
	println!("cargo:rustc-link-lib=static=opus");
	println!(
		"cargo:rustc-link-search=native={}",
		libopus_build_dir.join("lib").display()
	);
}

fn generate_bindings() {
	const ALLOW_LINTS: &str = r#"
#![allow(
	non_camel_case_types,
	non_snake_case,
	non_upper_case_globals,
	rustdoc::broken_intra_doc_links
)]
"#;
	let out_file = std::env::var("CARGO_MANIFEST_DIR")
		.map(PathBuf::from)
		.expect("CARGO_MANIFEST_DIR not set")
		.join("src/lib.rs");
	let bindings = bindgen::Builder::default()
		.header("src/bindings.h")
		.raw_line(ALLOW_LINTS.trim())
		.generate_block(true)
		.generate_cstr(true)
		.merge_extern_blocks(true)
		.sort_semantically(true)
		.parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
		.parse_callbacks(Box::new(DoxygenCallbacks))
		.generate()
		.expect("failed to generate libopus bindings");
	bindings
		.write_to_file(out_file)
		.expect("Couldn't write bindings!");
}

fn main() {
	println!("cargo:rerun-if-changed=src/bindings.h");
	println!("cargo:rerun-if-changed=libopus/include");
	println!("cargo:rerun-if-changed=libopus/src");
	let libopus_dir = get_libopus_dir();
	let build_dir = build_opus_with_cmake(&libopus_dir);
	link_opus(&build_dir);
	generate_bindings();
}

#[derive(Debug)]
struct DoxygenCallbacks;

impl ParseCallbacks for DoxygenCallbacks {
	fn process_comment(&self, comment: &str) -> Option<String> {
		Some(doxygen_rs::transform(comment))
	}
}
