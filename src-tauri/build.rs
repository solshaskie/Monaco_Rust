fn main() {
    tauri_build::build();

    let protos = [
        "../proto/ipc_envelope.proto",
        "../proto/ipc_file.proto",
        "../proto/ipc_editor.proto",
        "../proto/ipc_editor_language.proto",
        "../proto/ipc_editor_host.proto",
    ];

    prost_build::Config::new()
        .compile_protos(&protos, &["../proto"])
        .expect("failed to compile Monaco proto definitions");
}
