use std::{env, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"));
    let proto_dir = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(|path| path.join("MyServer").join("packages").join("proto"))
        .expect("failed to resolve workspace parent");
    let game_proto = proto_dir.join("game.proto");

    if !game_proto.exists() {
        panic!(
            "MyServer game.proto not found at {}. Expected server repo at C:\\project\\MyServer.",
            game_proto.display()
        );
    }

    println!("cargo:rerun-if-changed={}", game_proto.display());

    let protoc = protoc_bin_vendored::protoc_bin_path().expect("failed to locate vendored protoc");
    unsafe {
        env::set_var("PROTOC", protoc);
    }

    prost_build::Config::new()
        .compile_protos(&[game_proto], &[proto_dir])
        .expect("failed to compile MyServer game.proto");
}
