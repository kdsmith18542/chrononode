use std::path::Path;

fn main() {
    let proto_dir = Path::new("../../proto");
    let proto_file = proto_dir.join("chrononode.proto");
    if proto_file.exists() {
        // Use vendored protoc binary (cross-platform, includes Windows)
        let protoc =
            protoc_bin_vendored::protoc_bin_path().expect("Failed to find vendored protoc");
        std::env::set_var("PROTOC", protoc);
        prost_build::compile_protos(&[&proto_file], &[proto_dir])
            .expect("Failed to compile protobuf");
    }
}
