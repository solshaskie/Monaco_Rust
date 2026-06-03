use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Defines per-extension or per-operation resource limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SandboxLimits {
    /// Maximum buffer size in bytes (default: 50MB).
    pub max_buffer_size_bytes: usize,
    /// Maximum number of open buffers (default: 100).
    pub max_open_buffers: usize,
    /// Maximum total memory across all buffers (default: 200MB).
    pub max_total_memory_bytes: usize,
    /// Maximum decoration count per buffer (default: 10_000).
    pub max_decorations_per_buffer: usize,
    /// Maximum operation duration in milliseconds (default: 5000).
    pub max_operation_duration_ms: u64,
    /// Maximum file size that can be opened (default: 100MB).
    pub max_file_size_bytes: usize,
    /// Maximum undo stack size per buffer (default: 10_000).
    pub max_undo_stack_size: usize,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            max_buffer_size_bytes: 50 * 1024 * 1024, // 50 MB
            max_open_buffers: 100,
            max_total_memory_bytes: 200 * 1024 * 1024, // 200 MB
            max_decorations_per_buffer: 10_000,
            max_operation_duration_ms: 5000,
            max_file_size_bytes: 100 * 1024 * 1024, // 100 MB
            max_undo_stack_size: 10_000,
        }
    }
}

impl SandboxLimits {
    /// Creates strict limits for untrusted extensions.
    pub fn strict() -> Self {
        Self {
            max_buffer_size_bytes: 5 * 1024 * 1024, // 5 MB
            max_open_buffers: 10,
            max_total_memory_bytes: 20 * 1024 * 1024, // 20 MB
            max_decorations_per_buffer: 1_000,
            max_operation_duration_ms: 1000,
            max_file_size_bytes: 10 * 1024 * 1024, // 10 MB
            max_undo_stack_size: 1_000,
        }
    }

    /// Creates permissive limits for trusted built-ins.
    pub fn permissive() -> Self {
        Self {
            max_buffer_size_bytes: 200 * 1024 * 1024, // 200 MB
            max_open_buffers: 500,
            max_total_memory_bytes: 1024 * 1024 * 1024, // 1 GB
            max_decorations_per_buffer: 100_000,
            max_operation_duration_ms: 30_000,
            max_file_size_bytes: 500 * 1024 * 1024, // 500 MB
            max_undo_stack_size: 50_000,
        }
    }
}

/// Tracks actual resource usage against limits.
#[derive(Debug, Default)]
pub struct ResourceQuota {
    limits: SandboxLimits,
    open_buffer_count: usize,
    total_memory_bytes: usize,
    operation_count: u64,
    violation_count: u64,
}

impl ResourceQuota {
    pub fn new(limits: SandboxLimits) -> Self {
        Self {
            limits,
            open_buffer_count: 0,
            total_memory_bytes: 0,
            operation_count: 0,
            violation_count: 0,
        }
    }

    /// Checks if opening a buffer of the given size is allowed.
    pub fn can_open_buffer(&mut self, size_bytes: usize) -> Result<(), String> {
        if self.open_buffer_count >= self.limits.max_open_buffers {
            self.violation_count += 1;
            return Err(format!(
                "Open buffer limit exceeded: {} / {}",
                self.open_buffer_count, self.limits.max_open_buffers
            ));
        }
        if size_bytes > self.limits.max_file_size_bytes {
            self.violation_count += 1;
            return Err(format!(
                "File size {} exceeds limit {}",
                size_bytes, self.limits.max_file_size_bytes
            ));
        }
        let projected_total = self.total_memory_bytes + size_bytes;
        if projected_total > self.limits.max_total_memory_bytes {
            self.violation_count += 1;
            return Err(format!(
                "Total memory limit would be exceeded: {} + {} > {}",
                self.total_memory_bytes, size_bytes, self.limits.max_total_memory_bytes
            ));
        }
        Ok(())
    }

    /// Records that a buffer was opened.
    pub fn record_buffer_opened(&mut self, size_bytes: usize) {
        self.open_buffer_count += 1;
        self.total_memory_bytes += size_bytes;
        self.operation_count += 1;
    }

    /// Records that a buffer was closed.
    pub fn record_buffer_closed(&mut self, size_bytes: usize) {
        if self.open_buffer_count > 0 {
            self.open_buffer_count -= 1;
        }
        self.total_memory_bytes = self.total_memory_bytes.saturating_sub(size_bytes);
    }

    /// Records a buffer resize (e.g. after edits grow the content).
    pub fn record_buffer_resize(&mut self, old_size: usize, new_size: usize) {
        let delta = new_size.saturating_sub(old_size);
        let projected = self.total_memory_bytes + delta;
        if projected > self.limits.max_total_memory_bytes {
            self.violation_count += 1;
        }
        self.total_memory_bytes = self.total_memory_bytes.saturating_sub(old_size) + new_size;
    }

    /// Checks if an operation is within the time limit.
    pub fn check_operation_timeout(&self, duration_ms: u64) -> Result<(), String> {
        if duration_ms > self.limits.max_operation_duration_ms {
            return Err(format!(
                "Operation timeout: {}ms > {}ms",
                duration_ms, self.limits.max_operation_duration_ms
            ));
        }
        Ok(())
    }

    /// Returns current violation count.
    pub fn violation_count(&self) -> u64 {
        self.violation_count
    }

    /// Returns current total memory usage.
    pub fn total_memory_bytes(&self) -> usize {
        self.total_memory_bytes
    }

    /// Returns current open buffer count.
    pub fn open_buffer_count(&self) -> usize {
        self.open_buffer_count
    }
}

/// A per-principal security sandbox that enforces resource and memory bounds.
///
/// Each extension or agent gets its own sandbox instance with configurable limits.
#[derive(Debug)]
pub struct SecuritySandbox {
    limits: SandboxLimits,
    quotas: Mutex<HashMap<String, ResourceQuota>>,
}

impl SecuritySandbox {
    pub fn new(limits: SandboxLimits) -> Self {
        Self {
            limits,
            quotas: Mutex::new(HashMap::new()),
        }
    }

    pub fn default_instance() -> Arc<Self> {
        Arc::new(Self::new(SandboxLimits::default()))
    }

    /// Gets or creates the quota tracker for a principal.
    fn quota_for(
        &self,
        principal: &str,
    ) -> std::sync::MutexGuard<'_, HashMap<String, ResourceQuota>> {
        let mut quotas = self.quotas.lock().unwrap();
        quotas
            .entry(principal.to_string())
            .or_insert_with(|| ResourceQuota::new(self.limits));
        quotas
    }

    /// Checks if a principal can open a buffer of the given size.
    pub fn check_buffer_open(&self, principal: &str, size_bytes: usize) -> Result<(), String> {
        let mut quotas = self.quota_for(principal);
        let quota = quotas.get_mut(principal).unwrap();
        quota.can_open_buffer(size_bytes)
    }

    /// Records a buffer open for a principal.
    pub fn record_buffer_opened(&self, principal: &str, size_bytes: usize) {
        let mut quotas = self.quota_for(principal);
        let quota = quotas.get_mut(principal).unwrap();
        quota.record_buffer_opened(size_bytes);
    }

    /// Records a buffer close for a principal.
    pub fn record_buffer_closed(&self, principal: &str, size_bytes: usize) {
        let mut quotas = self.quota_for(principal);
        let quota = quotas.get_mut(principal).unwrap();
        quota.record_buffer_closed(size_bytes);
    }

    /// Records a buffer resize for a principal.
    pub fn record_buffer_resize(&self, principal: &str, old_size: usize, new_size: usize) {
        let mut quotas = self.quota_for(principal);
        let quota = quotas.get_mut(principal).unwrap();
        quota.record_buffer_resize(old_size, new_size);
    }

    /// Returns the total violation count across all principals.
    pub fn total_violations(&self) -> u64 {
        let quotas = self.quotas.lock().unwrap();
        quotas.values().map(|q| q.violation_count()).sum()
    }

    /// Returns a summary of resource usage across all principals.
    pub fn usage_summary(&self) -> HashMap<String, (usize, usize, u64)> {
        let quotas = self.quotas.lock().unwrap();
        quotas
            .iter()
            .map(|(k, q)| {
                (
                    k.clone(),
                    (
                        q.open_buffer_count(),
                        q.total_memory_bytes(),
                        q.violation_count(),
                    ),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_default_limits_allow_reasonable_use() {
        let sandbox = SecuritySandbox::new(SandboxLimits::default());
        assert!(sandbox.check_buffer_open("ext1", 1024).is_ok());
        sandbox.record_buffer_opened("ext1", 1024);
        assert_eq!(sandbox.total_violations(), 0);
    }

    #[test]
    fn sandbox_enforces_buffer_count_limit() {
        let limits = SandboxLimits {
            max_open_buffers: 2,
            ..SandboxLimits::default()
        };
        let sandbox = SecuritySandbox::new(limits);
        sandbox.record_buffer_opened("ext1", 100);
        sandbox.record_buffer_opened("ext1", 100);
        assert!(sandbox.check_buffer_open("ext1", 100).is_err());
        assert!(sandbox.total_violations() >= 1);
    }

    #[test]
    fn sandbox_enforces_file_size_limit() {
        let limits = SandboxLimits {
            max_file_size_bytes: 100,
            ..SandboxLimits::default()
        };
        let sandbox = SecuritySandbox::new(limits);
        assert!(sandbox.check_buffer_open("ext1", 50).is_ok());
        assert!(sandbox.check_buffer_open("ext1", 150).is_err());
    }

    #[test]
    fn sandbox_enforces_total_memory_limit() {
        let limits = SandboxLimits {
            max_total_memory_bytes: 200,
            ..SandboxLimits::default()
        };
        let sandbox = SecuritySandbox::new(limits);
        sandbox.record_buffer_opened("ext1", 150);
        assert!(sandbox.check_buffer_open("ext1", 100).is_err());
    }

    #[test]
    fn sandbox_records_usage_summary() {
        let sandbox = SecuritySandbox::new(SandboxLimits::default());
        sandbox.record_buffer_opened("ext1", 1024);
        sandbox.record_buffer_opened("ext2", 2048);
        let summary = sandbox.usage_summary();
        assert_eq!(summary.get("ext1"), Some(&(1, 1024, 0)));
        assert_eq!(summary.get("ext2"), Some(&(1, 2048, 0)));
    }

    #[test]
    fn sandbox_strict_limits_are_tighter() {
        let strict = SandboxLimits::strict();
        let default = SandboxLimits::default();
        assert!(strict.max_buffer_size_bytes < default.max_buffer_size_bytes);
        assert!(strict.max_open_buffers < default.max_open_buffers);
        assert!(strict.max_total_memory_bytes < default.max_total_memory_bytes);
    }
}
