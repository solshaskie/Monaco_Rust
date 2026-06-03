/**
 * Monaco Tokenizer Provider backed by WASM compute module.
 *
 * Bridges Monaco's tokenization API to the monaco-wasm compute module.
 * This is a proof-of-concept for W1.2: JS Glue Layer.
 */

import { tokenizeCached } from './wasm-glue.js';

function getModelPath(model) {
  if (!model || !model.uri) {
    return null;
  }
  if (model.uri.scheme === 'file') {
    return model.uri.path;
  }
  return model.uri.toString();
}

function getWasmSyncSource(model) {
  const path = getModelPath(model);
  const manager = window.wasmBufferSync;
  if (path && manager && typeof manager.getContent === 'function') {
    const content = manager.getContent(path);
    if (typeof content === 'string') {
      return content;
    }
  }
  return model.getValue();
}

/**
 * Convert WASM tokens to Monaco's ILineTokens format.
 * Each line is tokenized independently.
 */
function toMonacoLineTokens(wasmTokens, lineNumber) {
  const lineTokens = wasmTokens.filter(t => t.line === lineNumber);
  if (lineTokens.length === 0) {
    return { tokens: [], endState: null };
  }

  const tokens = [];
  let offset = 0;
  for (const tok of lineTokens) {
    const tokenType = toMonacoTokenType(tok.token_type);
    tokens.push(offset);           // start offset
    tokens.push(tok.text.length);  // length
    tokens.push(tokenType);        // token type ID (we'll use string for now)
    offset += tok.text.length;
  }

  return {
    tokens: tokens,
    endState: null, // stateless tokenizer for now
  };
}

/**
 * Map WASM token types to Monaco token types.
 */
function toMonacoTokenType(wasmType) {
  const mapping = {
    keyword: 'keyword',
    identifier: 'identifier',
    string: 'string',
    number: 'number',
    comment: 'comment',
    operator: 'operator',
    punctuation: 'delimiter',
  };
  return mapping[wasmType] || 'identifier';
}

/**
 * Create a Monaco TokensProvider backed by WASM tokenization.
 * @param {string} languageId — e.g. 'rust', 'javascript'
 */
export function createWasmTokensProvider(languageId) {
  let fullTokens = null;
  let lastSource = null;

  return {
    getInitialState() {
      return { line: 0 };
    },

    tokenize(line, state) {
      // For now, tokenize the full source on first call.
      // In production, this would use incremental tokenization.
      return {
        tokens: [],
        endState: state,
      };
    },
  };
}

/**
 * Register WASM-backed tokenization for a language.
 * Call after Monaco is loaded and WASM is initialized.
 */
export async function registerWasmTokenizer(monaco, languageId) {
  // Monaco's advanced tokenization requires a full monarch grammar.
  // For the hybrid architecture, we expose WASM tokenization as a
  // semantic token provider instead, which is more flexible.

  monaco.languages.registerDocumentSemanticTokensProvider(languageId, {
    getLegend() {
      return {
        tokenTypes: [
          'namespace', 'type', 'class', 'enum', 'interface',
          'struct', 'typeParameter', 'parameter', 'variable', 'property',
          'enumMember', 'event', 'function', 'method', 'macro',
          'keyword', 'modifier', 'comment', 'string', 'number',
          'regexp', 'operator',
        ],
        tokenModifiers: [
          'declaration', 'definition', 'readonly', 'static',
          'deprecated', 'abstract', 'async', 'modification',
          'documentation', 'defaultLibrary',
        ],
      };
    },

    provideDocumentSemanticTokens(model, _lastResultId, _token) {
      return provideSemanticTokens(model);
    },

    provideDocumentSemanticTokensEdits(model, _lastResultId, _token) {
      return provideSemanticTokens(model);
    },

    releaseDocumentSemanticTokens(_resultId) {
      // No-op for now
    },
  });
}

const tokenizationGeneration = new Map(); // model uri -> generation counter
const semanticTokenCache = new Map(); // model uri -> Uint32Array

async function provideSemanticTokens(model) {
  const path = getModelPath(model);
  const uri = model.uri.toString();
  const manager = window.wasmBufferSync;

  if (path && manager && manager.isSyncBarrierActive(path)) {
    return { data: semanticTokenCache.get(uri) || new Uint32Array(0) };
  }

  const source = getWasmSyncSource(model);
  const language = model.getLanguageId();
  const resource = path || uri;

  const gen = (tokenizationGeneration.get(uri) || 0) + 1;
  tokenizationGeneration.set(uri, gen);

  try {
    const wasmTokens = await tokenizeCached(source, language, resource);
    // Discard result if a newer tokenization request arrived
    if (tokenizationGeneration.get(uri) !== gen) {
      return { data: semanticTokenCache.get(uri) || new Uint32Array(0) };
    }
    const data = encodeSemanticTokens(wasmTokens);
    semanticTokenCache.set(uri, data);
    return { data };
  } catch (e) {
    console.warn('[WASM Tokenizer] Failed:', e);
    return { data: semanticTokenCache.get(uri) || new Uint32Array(0) };
  }
}

/**
 * Encode WASM tokens into Monaco's semantic token format.
 * Format: [deltaLine, deltaStartChar, length, tokenType, tokenModifiers]...
 */
function encodeSemanticTokens(tokens) {
  const data = [];
  let prevLine = 0;
  let prevChar = 0;

  for (const tok of tokens) {
    const line = tok.line;
    const startChar = tok.start;
    const length = tok.end - tok.start;
    const tokenType = mapToSemanticTokenType(tok.token_type);
    const tokenModifiers = 0;

    const deltaLine = line - prevLine;
    const deltaStartChar = deltaLine === 0 ? startChar - prevChar : startChar;

    data.push(deltaLine, deltaStartChar, length, tokenType, tokenModifiers);

    prevLine = line;
    prevChar = startChar;
  }

  return new Uint32Array(data);
}

function mapToSemanticTokenType(wasmType) {
  const mapping = {
    keyword: 13,    // keyword
    identifier: 8, // variable
    string: 18,    // string
    number: 19,    // number
    comment: 17,   // comment
    operator: 21,  // operator
    punctuation: 8, // variable (fallback)
  };
  return mapping[wasmType] ?? 8;
}
