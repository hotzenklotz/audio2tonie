fn main() {
    protobuf_codegen::Codegen::new()
        .protoc()
        .input("src/tonie_header/tonie_header.proto")
        .includes(&["src/tonie_header"])
        .out_dir("src/tonie_header")
        .run_from_script();
}