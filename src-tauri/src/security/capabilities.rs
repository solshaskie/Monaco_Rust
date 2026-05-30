use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Permissions that can be granted to a principal (extension, agent, or user).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read file contents.
    ReadFile,
    /// Write/modify file contents.
    WriteFile,
    /// Create new files.
    CreateFile,
    /// Delete files.
    DeleteFile,
    /// Execute shell commands.
    ExecuteCommand,
    /// Access network resources.
    NetworkAccess,
    /// Access workspace file listing.
    ListDirectory,
    /// Read workspace configuration.
    ReadWorkspaceConfig,
    /// Modify workspace configuration.
    WriteWorkspaceConfig,
    /// Access clipboard.
    ClipboardAccess,
    /// Show notifications.
    ShowNotification,
    /// Full access (superuser mode — use sparingly).
    FullAccess,
}

impl Permission {
    /// Human-readable name for audit logging.
    pub fn name(&self) -> &'static str {
        match self {
            Permission::ReadFile => "read_file",
            Permission::WriteFile => "write_file",
            Permission::CreateFile => "create_file",
            Permission::DeleteFile => "delete_file",
            Permission::ExecuteCommand => "execute_command",
            Permission::NetworkAccess => "network_access",
            Permission::ListDirectory => "list_directory",
            Permission::ReadWorkspaceConfig => "read_workspace_config",
            Permission::WriteWorkspaceConfig => "write_workspace_config",
            Permission::ClipboardAccess => "clipboard_access",
            Permission::ShowNotification => "show_notification",
            Permission::FullAccess => "full_access",
        }
    }
}

/// A principal is an entity that can be granted permissions (extension ID, agent ID, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Principal {
    pub id: String,
    pub kind: PrincipalKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrincipalKind {
    Extension,
    Agent,
    User,
    System,
}

/// A single audit log entry.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditLogEntry {
    pub timestamp_ms: u64,
    pub principal_id: String,
    pub action: String,
    pub resource: String,
    pub allowed: bool,
    pub reason: String,
}

impl AuditLogEntry {
    pub fn new(
        principal_id: &str,
        action: &str,
        resource: &str,
        allowed: bool,
        reason: &str,
    ) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            timestamp_ms,
            principal_id: principal_id.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            allowed,
            reason: reason.to_string(),
        }
    }
}

/// An append-only audit log for security events.
#[derive(Debug)]
pub struct AuditLog {
    entries: Mutex<Vec<AuditLogEntry>>,
    max_entries: usize,
}

impl AuditLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
            max_entries,
        }
    }

    /// Records an access attempt.
    pub fn record(&self, entry: AuditLogEntry) {
        let mut entries = self.entries.lock().unwrap();
        entries.push(entry);
        // Trim oldest entries if over limit
        if entries.len() > self.max_entries {
            let excess = entries.len() - self.max_entries;
            entries.drain(0..excess);
        }
    }

    /// Returns all entries (oldest first).
    pub fn entries(&self) -> Vec<AuditLogEntry> {
        self.entries.lock().unwrap().clone()
    }

    /// Returns entries for a specific principal.
    pub fn entries_for(&self, principal_id: &str) -> Vec<AuditLogEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.principal_id == principal_id)
            .cloned()
            .collect()
    }

    /// Returns the most recent N entries.
    pub fn recent(&self, n: usize) -> Vec<AuditLogEntry> {
        let entries = self.entries.lock().unwrap();
        entries.iter().rev().take(n).cloned().collect()
    }

    /// Returns the total number of denied entries.
    pub fn denied_count(&self) -> usize {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| !e.allowed)
            .count()
    }
}

/// A capability registry that manages permissions per principal.
///
/// Permissions are positive-grant: if not explicitly granted, access is denied.
/// The `FullAccess` permission bypasses all checks.
#[derive(Debug)]
pub struct CapabilityRegistry {
    grants: Mutex<HashMap<String, HashSet<Permission>>>,
    audit: AuditLog,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
            audit: AuditLog::new(10_000),
        }
    }

    pub fn with_audit_capacity(max_entries: usize) -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
            audit: AuditLog::new(max_entries),
        }
    }

    /// Grants a permission to a principal.
    pub fn grant(&self, principal_id: &str, permission: Permission) {
        let mut grants = self.grants.lock().unwrap();
        grants
            .entry(principal_id.to_string())
            .or_default()
            .insert(permission);
    }

    /// Grants multiple permissions to a principal.
    pub fn grant_many(&self, principal_id: &str, permissions: &[Permission]) {
        let mut grants = self.grants.lock().unwrap();
        let set = grants.entry(principal_id.to_string()).or_default();
        for p in permissions {
            set.insert(*p);
        }
    }

    /// Revokes a permission from a principal.
    pub fn revoke(&self, principal_id: &str, permission: Permission) {
        let mut grants = self.grants.lock().unwrap();
        if let Some(set) = grants.get_mut(principal_id) {
            set.remove(&permission);
        }
    }

    /// Revokes all permissions from a principal.
    pub fn revoke_all(&self, principal_id: &str) {
        let mut grants = self.grants.lock().unwrap();
        grants.remove(principal_id);
    }

    /// Checks if a principal has a permission (and logs the attempt).
    pub fn check(&self, principal: &Principal, permission: Permission, resource: &str) -> bool {
        let allowed = self.has_permission_internal(&principal.id, permission);
        let reason = if allowed {
            "permission granted"
        } else {
            "permission denied"
        };
        self.audit.record(AuditLogEntry::new(
            &principal.id,
            permission.name(),
            resource,
            allowed,
            reason,
        ));
        allowed
    }

    /// Checks without logging (for internal use).
    fn has_permission_internal(&self, principal_id: &str, permission: Permission) -> bool {
        let grants = self.grants.lock().unwrap();
        if let Some(set) = grants.get(principal_id) {
            if set.contains(&Permission::FullAccess) {
                return true;
            }
            return set.contains(&permission);
        }
        false
    }

    /// Returns the permissions granted to a principal.
    pub fn permissions_for(&self, principal_id: &str) -> Vec<Permission> {
        let grants = self.grants.lock().unwrap();
        grants
            .get(principal_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Returns the audit log.
    pub fn audit_log(&self) -> &AuditLog {
        &self.audit
    }

    /// Pre-configures a built-in extension principal with safe defaults.
    pub fn setup_builtin_extension(&self, principal_id: &str) {
        self.grant_many(
            principal_id,
            &[
                Permission::ReadFile,
                Permission::ListDirectory,
                Permission::ReadWorkspaceConfig,
            ],
        );
    }

    /// Pre-configures an agent principal with moderate access.
    pub fn setup_agent(&self, principal_id: &str) {
        self.grant_many(
            principal_id,
            &[
                Permission::ReadFile,
                Permission::WriteFile,
                Permission::CreateFile,
                Permission::ListDirectory,
                Permission::ReadWorkspaceConfig,
                Permission::ShowNotification,
            ],
        );
    }

    /// Pre-configures the system principal with full access.
    pub fn setup_system(&self) {
        self.grant("system", Permission::FullAccess);
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        let registry = Self::new();
        registry.setup_system();
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_registry_grants_and_checks() {
        let registry = CapabilityRegistry::new();
        let ext = Principal {
            id: "ext1".to_string(),
            kind: PrincipalKind::Extension,
        };

        assert!(!registry.check(&ext, Permission::ReadFile, "/foo"));
        registry.grant("ext1", Permission::ReadFile);
        assert!(registry.check(&ext, Permission::ReadFile, "/foo"));
    }

    #[test]
    fn capability_registry_revokes() {
        let registry = CapabilityRegistry::new();
        registry.grant("ext1", Permission::WriteFile);
        let ext = Principal {
            id: "ext1".to_string(),
            kind: PrincipalKind::Extension,
        };
        assert!(registry.check(&ext, Permission::WriteFile, "/foo"));
        registry.revoke("ext1", Permission::WriteFile);
        assert!(!registry.check(&ext, Permission::WriteFile, "/foo"));
    }

    #[test]
    fn full_access_bypasses_all() {
        let registry = CapabilityRegistry::new();
        registry.grant("admin", Permission::FullAccess);
        let admin = Principal {
            id: "admin".to_string(),
            kind: PrincipalKind::User,
        };
        assert!(registry.check(&admin, Permission::DeleteFile, "/etc"));
        assert!(registry.check(&admin, Permission::NetworkAccess, "https://example.com"));
    }

    #[test]
    fn audit_log_records_denied_access() {
        let registry = CapabilityRegistry::new();
        let ext = Principal {
            id: "ext1".to_string(),
            kind: PrincipalKind::Extension,
        };
        registry.check(&ext, Permission::WriteFile, "/foo");
        let entries = registry.audit_log().entries_for("ext1");
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].allowed);
        assert_eq!(entries[0].action, "write_file");
    }

    #[test]
    fn audit_log_trims_old_entries() {
        let registry = CapabilityRegistry::with_audit_capacity(3);
        let ext = Principal {
            id: "ext1".to_string(),
            kind: PrincipalKind::Extension,
        };
        registry.grant("ext1", Permission::ReadFile);
        for i in 0..5 {
            registry.check(&ext, Permission::ReadFile, &format!("/file{}", i));
        }
        let entries = registry.audit_log().entries();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn setup_builtin_extension_has_safe_defaults() {
        let registry = CapabilityRegistry::new();
        registry.setup_builtin_extension("builtin1");
        let perms = registry.permissions_for("builtin1");
        assert!(perms.contains(&Permission::ReadFile));
        assert!(perms.contains(&Permission::ListDirectory));
        assert!(!perms.contains(&Permission::WriteFile));
        assert!(!perms.contains(&Permission::ExecuteCommand));
    }

    #[test]
    fn setup_agent_has_moderate_access() {
        let registry = CapabilityRegistry::new();
        registry.setup_agent("agent1");
        let perms = registry.permissions_for("agent1");
        assert!(perms.contains(&Permission::ReadFile));
        assert!(perms.contains(&Permission::WriteFile));
        assert!(!perms.contains(&Permission::ExecuteCommand));
        assert!(!perms.contains(&Permission::NetworkAccess));
    }
}
