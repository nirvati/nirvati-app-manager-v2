// Minify any JS code in src/tera/polyfills/*.js to $OUT_DIR/polyfills/*.js
// To reduce binary size

use esbuild_rs::{transform_direct, Loader, TransformOptionsBuilder};
use std::fs;
use std::sync::mpsc::channel;
use std::{path::Path, sync::Mutex};

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    println!("cargo:rerun-if-changed=src/tera/polyfills");
    std::fs::create_dir_all(out_dir.join("polyfills")).unwrap();
    for entry in fs::read_dir("src/tera/polyfills").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name().unwrap().to_str().unwrap();
            let ext = path.extension().unwrap().to_str().unwrap();
            if ext == "js" || (ext == "ts" && !file_name.ends_with(".d.ts")) {
                // Read the file to a u8 vector
                let src = fs::read(&path).unwrap();
                let mut options = TransformOptionsBuilder::new();
                options.loader = if ext == "js" { Loader::JS } else { Loader::TS };
                options.minify_identifiers = true;
                options.minify_syntax = true;
                options.minify_whitespace = true;
                let options = options.build();
                let (tx, rx) = channel();
                let tx = Mutex::new(tx);
                transform_direct(src.into(), options, move |result| {
                    for err in result.errors.as_slice() {
                        panic!("{:#?}", err.to_string());
                    }
                    tx.lock().unwrap().send(result.code.to_string()).unwrap();
                });
                let transformed = rx.recv().unwrap();
                fs::write(
                    out_dir.join("polyfills").join(
                        path.with_extension("js")
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap(),
                    ),
                    transformed,
                )
                .unwrap();
            }
        }
    }
}
