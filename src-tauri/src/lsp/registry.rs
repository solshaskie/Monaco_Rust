use std::collections::HashMap;

use super::client::LspClient;

/// Configuration for how to spawn an external language server for a given language.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub command: String,
    pub args: Vec<String>,
}

impl ServerConfig {
    pub fn new(command: &str, args: &[&str]) -> Self {
        Self {
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// A registry that maps file extensions/URIs to active LSP clients.
/// Clients are lazily spawned on first access per workspace root.
pub struct LspRegistry {
    configs: HashMap<String, ServerConfig>,
    clients: HashMap<String, LspClient>,
    workspace_root: String,
}

impl LspRegistry {
    pub fn new(workspace_root: String) -> Self {
        let mut configs = HashMap::new();
        // Default configurations for supported languages
        configs.insert("rust".to_string(), ServerConfig::new("rust-analyzer", &[]));
        configs.insert(
            "typescript".to_string(),
            ServerConfig::new("typescript-language-server", &["--stdio"]),
        );
        configs.insert(
            "javascript".to_string(),
            ServerConfig::new("typescript-language-server", &["--stdio"]),
        );

        Self {
            configs,
            clients: HashMap::new(),
            workspace_root,
        }
    }

    /// Returns a mutable reference to the LSP client for the given language,
    /// spawning it if necessary.
    pub fn client_for_language(&mut self, language_id: &str) -> Option<&mut LspClient> {
        if !self.clients.contains_key(language_id) {
            let config = self.configs.get(language_id)?;
            let args: Vec<&str> = config.args.iter().map(|s| s.as_str()).collect();
            let client = LspClient::spawn(&config.command, &args, &self.workspace_root).ok()?;
            self.clients.insert(language_id.to_string(), client);
        }
        self.clients.get_mut(language_id)
    }

    /// Returns a mutable reference to the LSP client inferred from a file path.
    pub fn client_for_path(&mut self, path: &str) -> Option<&mut LspClient> {
        let ext = path.rfind('.').map(|i| &path[i..]);
        let language_id = match ext {
            Some(".rs") => "rust",
            Some(".ts") | Some(".tsx") | Some(".mts") | Some(".cts") => "typescript",
            Some(".js") | Some(".mjs") | Some(".cjs") => "javascript",
            _ => return None,
        };
        self.client_for_language(language_id)
    }

    /// Registers a custom server configuration for a language.
    pub fn register_config(&mut self, language_id: &str, config: ServerConfig) {
        self.configs.insert(language_id.to_string(), config);
    }

    /// Notifies all active servers that a document was opened.
    pub fn did_open(&mut self, path: &str, language_id: &str, version: i32, text: &str) {
        if let Some(client) = self.client_for_language(language_id) {
            let uri = path_to_uri(path);
            client.did_open(&uri, language_id, version, text);
        }
    }

    /// Notifies all active servers that a document was changed.
    pub fn did_change(
        &mut self,
        path: &str,
        language_id: &str,
        version: i32,
        changes: Vec<serde_json::Value>,
    ) {
        if let Some(client) = self.client_for_language(language_id) {
            let uri = path_to_uri(path);
            client.did_change(&uri, version, changes);
        }
    }

    /// Notifies all active servers that a document was closed.
    pub fn did_close(&mut self, path: &str, language_id: &str) {
        if let Some(client) = self.client_for_language(language_id) {
            let uri = path_to_uri(path);
            client.did_close(&uri);
        }
    }
}

fn path_to_uri(path: &str) -> String {
    if path.starts_with("file://") {
        path.to_string()
    } else {
        format!("file://{}", path)
    }
}
