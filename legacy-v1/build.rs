use std::io::Result;

fn main() -> Result<()> {
    // Set PROTOC environment variable to the path of the vendored protoc
    std::env::set_var("PROTOC", protobuf_src::protoc());
    
    prost_build::compile_protos(&["src/proto/chrononode.proto"], &["src/proto/"])?;
    Ok(())
}
