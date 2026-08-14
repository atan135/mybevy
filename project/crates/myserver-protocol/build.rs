use std::{env, path::PathBuf};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"));
    let project_dir = manifest_dir
        .parent()
        .and_then(|directory| directory.parent())
        .expect("myserver-protocol is nested under project/crates");
    let proto_dir = project_dir.join("vendor").join("myserver").join("proto");
    let game_proto = proto_dir.join("game.proto");
    let chat_proto = proto_dir.join("chat.proto");

    for proto in [&game_proto, &chat_proto] {
        if !proto.exists() {
            panic!(
                "vendored MyServer proto not found at {}. Refresh project/vendor/myserver from MyServer.",
                proto.display()
            );
        }
        println!("cargo:rerun-if-changed={}", proto.display());
    }

    let protoc = protoc_bin_vendored::protoc_bin_path().expect("failed to locate vendored protoc");
    unsafe {
        env::set_var("PROTOC", protoc);
    }

    prost_build::Config::new()
        .compile_protos(&[game_proto, chat_proto], &[proto_dir])
        .expect("failed to compile MyServer game/chat proto files");
}
