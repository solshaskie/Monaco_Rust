#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);

    let Ok(mut parser) = monaco_tauri::syntax::parser::SyntaxParser::for_rust() else {
        return;
    };

    if let Ok(parsed) = parser.parse(&source) {
        let _ = monaco_tauri::syntax::tokens::tokenize_tree(&parsed.tree, &source);
    }
});
