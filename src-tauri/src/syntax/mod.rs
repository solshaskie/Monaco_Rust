pub mod completion;
pub mod diagnostics;
pub mod folding;
pub mod hover;
pub mod parser;
pub mod semantic_tokens;
pub mod symbols;
pub mod tokens;

pub use completion::{collect_completions, prefix_at_position, CompletionItem};
pub use diagnostics::{collect_syntax_diagnostics, DiagnosticSeverity, SyntaxDiagnostic};
pub use folding::{extract_folding_ranges, FoldingRange};
pub use hover::{hover_at_position, HoverInfo};
pub use parser::{ParsedTree, SyntaxParser};
pub use semantic_tokens::{pack_semantic_tokens, TOKEN_TYPE_LEGEND};
pub use symbols::{extract_document_symbols, DocumentSymbol};
pub use tokens::{tokenize_tree, tokenize_tree_range, SyntaxToken, Tokenizer};
