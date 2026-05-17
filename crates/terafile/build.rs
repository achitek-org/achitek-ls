fn main() {
    compile_tree_sitter_tera();
}

fn compile_tree_sitter_tera() {
    let manifest_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set"),
    );
    let src_dir = manifest_dir.join("vendor/tree-sitter-tera/src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(&src_dir);

    #[cfg(target_env = "msvc")]
    c_config.flag("-utf-8");

    let parser_path = src_dir.join("parser.c");
    c_config.file(&parser_path);
    println!("cargo:rerun-if-changed={}", parser_path.display());

    let scanner_path = src_dir.join("scanner.c");
    c_config.file(&scanner_path);
    println!("cargo:rerun-if-changed={}", scanner_path.display());

    for header in ["alloc.h", "array.h", "parser.h"] {
        println!(
            "cargo:rerun-if-changed={}",
            src_dir.join("tree_sitter").join(header).display()
        );
    }

    c_config.compile("tree-sitter-tera");
}
