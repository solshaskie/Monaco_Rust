pub mod code {
    pub mod ipc {
        include!(concat!(env!("OUT_DIR"), "/code.ipc.rs"));

        pub mod file {
            include!(concat!(env!("OUT_DIR"), "/code.ipc.file.rs"));
        }

        pub mod editor {
            include!(concat!(env!("OUT_DIR"), "/code.ipc.editor.rs"));

            pub mod host {
                include!(concat!(env!("OUT_DIR"), "/code.ipc.editor.host.rs"));
            }

            pub mod language {
                include!(concat!(env!("OUT_DIR"), "/code.ipc.editor.language.rs"));
            }
        }
    }
}
