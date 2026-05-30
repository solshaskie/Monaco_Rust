pub mod parser;
pub mod tokens;
pub mod folding;
pub mod semantic_tokens;
pub mod hover;
pub mod diagnostics;
pub mod symbols;
pub mod completion;

pub use parser::{SyntaxParser, ParsedTree};
pub use tokens::{SyntaxToken, Tokenizer, tokenize_tree, tokenize_tree_range};
pub use folding::{FoldingRange, extract_folding_ranges};
pub use semantic_tokens::{pack_semantic_tokens, TOKEN_TYPE_LEGEND};
pub use hover::{HoverInfo, hover_at_position};
pub use diagnostics::{DiagnosticSeverity, SyntaxDiagnostic, collect_syntax_diagnostics};
pub use symbols::{DocumentSymbol, extract_document_symbols};
pub use completion::{CompletionItem, collect_completions, prefix_at_position};
