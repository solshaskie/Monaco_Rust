use crate::syntax::tokens::SyntaxToken;

/// LSP-style semantic token type legend.
pub const TOKEN_TYPE_LEGEND: &[&str] = &[
    "keyword",    // 0
    "identifier", // 1
    "string",     // 2
    "number",     // 3
    "comment",    // 4
    "operator",   // 5
    "type",       // 6
    "macro",      // 7
];

fn token_type_index(token_type: &str) -> u32 {
    match token_type {
        "keyword" => 0,
        "identifier" => 1,
        "string" => 2,
        "number" => 3,
        "comment" => 4,
        "operator" => 5,
        "type" => 6,
        "macro" => 7,
        _ => 1, // default to identifier
    }
}

/// Packs SyntaxTokens into LSP SemanticTokens format.
/// Each token is encoded as 5 uint32 values:
/// [deltaLine, deltaStartChar, length, tokenType, tokenModifiers]
pub fn pack_semantic_tokens(tokens: &[SyntaxToken]) -> Vec<u32> {
    let mut data = Vec::with_capacity(tokens.len() * 5);

    let mut sorted = tokens.to_vec();
    sorted.sort_by(|a, b| {
        a.start_line.cmp(&b.start_line)
            .then_with(|| a.start_column.cmp(&b.start_column))
    });

    let mut prev_line = 0u32;
    let mut prev_char = 0u32;

    for token in &sorted {
        let delta_line = token.start_line.saturating_sub(prev_line);
        let delta_start = if delta_line == 0 {
            token.start_column.saturating_sub(prev_char)
        } else {
            token.start_column.saturating_sub(1)
        };
        let length = (token.end_column.saturating_sub(token.start_column)) as u32;
        let type_index = token_type_index(&token.token_type);

        data.push(delta_line);
        data.push(delta_start);
        data.push(length);
        data.push(type_index);
        data.push(0); // tokenModifiers = none

        prev_line = token.start_line;
        prev_char = token.start_column;
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_single_token() {
        let tokens = vec![SyntaxToken {
            token_type: "keyword".to_string(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 3,
            text: "fn".to_string(),
        }];
        let packed = pack_semantic_tokens(&tokens);
        assert_eq!(packed, vec![1, 0, 2, 0, 0]);
    }

    #[test]
    fn pack_tokens_same_line() {
        let tokens = vec![
            SyntaxToken {
                token_type: "keyword".to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 3,
                text: "fn".to_string(),
            },
            SyntaxToken {
                token_type: "identifier".to_string(),
                start_line: 1,
                start_column: 4,
                end_line: 1,
                end_column: 8,
                text: "main".to_string(),
            },
        ];
        let packed = pack_semantic_tokens(&tokens);
        assert_eq!(packed, vec![1, 0, 2, 0, 0, 0, 3, 4, 1, 0]);
    }

    #[test]
    fn pack_tokens_multi_line() {
        let tokens = vec![
            SyntaxToken {
                token_type: "keyword".to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 4,
                text: "let".to_string(),
            },
            SyntaxToken {
                token_type: "number".to_string(),
                start_line: 2,
                start_column: 5,
                end_line: 2,
                end_column: 6,
                text: "1".to_string(),
            },
        ];
        let packed = pack_semantic_tokens(&tokens);
        // First token: line=1, char=0, len=3, type=keyword(0)
        // Second token: deltaLine=1, deltaChar=4 (5-1), len=1, type=number(3)
        assert_eq!(packed, vec![1, 0, 3, 0, 0, 1, 4, 1, 3, 0]);
    }
}
