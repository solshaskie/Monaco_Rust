use tree_sitter::{Language, Parser, Tree};

/// A syntax parser that can parse source code using Tree-sitter grammars.
pub struct SyntaxParser {
    parser: Parser,
}

/// The result of parsing a source file.
pub struct ParsedTree {
    pub tree: Tree,
    pub source: String,
}

impl SyntaxParser {
    /// Creates a new parser for the Rust language.
    pub fn for_rust() -> Result<Self, String> {
        Self::with_language(tree_sitter_rust::LANGUAGE.into(), "Rust")
    }

    /// Creates a new parser for the JavaScript language.
    pub fn for_javascript() -> Result<Self, String> {
        Self::with_language(tree_sitter_javascript::LANGUAGE.into(), "JavaScript")
    }

    /// Creates a new parser for the TypeScript language.
    pub fn for_typescript() -> Result<Self, String> {
        Self::with_language(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "TypeScript",
        )
    }

    fn with_language(language: Language, name: &str) -> Result<Self, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| format!("Failed to set {} language: {}", name, e))?;
        Ok(Self { parser })
    }

    /// Parses the given source code and returns the syntax tree.
    pub fn parse(&mut self, source: &str) -> Result<ParsedTree, String> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| "Parsing failed".to_string())?;
        Ok(ParsedTree {
            tree,
            source: source.to_string(),
        })
    }

    /// Re-parses source code given a previous tree for incremental parsing.
    pub fn parse_incremental(
        &mut self,
        source: &str,
        previous_tree: &Tree,
    ) -> Result<ParsedTree, String> {
        let tree = self
            .parser
            .parse(source, Some(previous_tree))
            .ok_or_else(|| "Incremental parsing failed".to_string())?;
        Ok(ParsedTree {
            tree,
            source: source.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_can_parse_rust() {
        let mut parser = SyntaxParser::for_rust().unwrap();
        let source = "fn main() { println!(\"hello\"); }";
        let parsed = parser.parse(source).unwrap();

        let root = parsed.tree.root_node();
        assert_eq!(root.kind(), "source_file");
        assert!(!root.has_error());
    }

    #[test]
    fn parser_detects_syntax_error() {
        let mut parser = SyntaxParser::for_rust().unwrap();
        let source = "fn main() { println!(\"hello\" }"; // missing closing paren
        let parsed = parser.parse(source).unwrap();

        let root = parsed.tree.root_node();
        assert!(root.has_error());
    }

    #[test]
    fn incremental_parse_works() {
        let mut parser = SyntaxParser::for_rust().unwrap();
        let source1 = "fn main() {\n    println!(\"hello\");\n}";
        let parsed1 = parser.parse(source1).unwrap();

        let source2 = "fn main() {\n    println!(\"hello world\");\n}";
        let parsed2 = parser.parse_incremental(source2, &parsed1.tree).unwrap();

        let root = parsed2.tree.root_node();
        assert_eq!(root.kind(), "source_file");
    }

    #[test]
    fn parser_can_parse_javascript() {
        let mut parser = SyntaxParser::for_javascript().unwrap();
        let source = "function main() { console.log('hello'); }";
        let parsed = parser.parse(source).unwrap();

        let root = parsed.tree.root_node();
        assert_eq!(root.kind(), "program");
        assert!(!root.has_error());
    }

    #[test]
    fn parser_can_parse_typescript() {
        let mut parser = SyntaxParser::for_typescript().unwrap();
        let source = "function main(): void { console.log('hello'); }";
        let parsed = parser.parse(source).unwrap();

        let root = parsed.tree.root_node();
        assert_eq!(root.kind(), "program");
        assert!(!root.has_error());
    }
}
