use std::{env, path::PathBuf};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"));
    let proto_dir = manifest_dir.join("vendor").join("myserver").join("proto");
    let game_proto = proto_dir.join("game.proto");
    let chat_proto = proto_dir.join("chat.proto");

    if !game_proto.exists() {
        panic!(
            "vendored MyServer game.proto not found at {}. Refresh project/vendor/myserver from MyServer.",
            game_proto.display()
        );
    }

    println!("cargo:rerun-if-changed={}", game_proto.display());
    println!("cargo:rerun-if-changed={}", chat_proto.display());

    let protoc = protoc_bin_vendored::protoc_bin_path().expect("failed to locate vendored protoc");
    unsafe {
        env::set_var("PROTOC", protoc);
    }

    prost_build::Config::new()
        .compile_protos(&[game_proto, chat_proto], &[proto_dir])
        .expect("failed to compile MyServer game/chat proto files");
}
