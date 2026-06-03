/**
 * WASM buffer sync manager.
 *
 * Keeps a frontend-side shadow of Rust-owned buffer truth so WASM compute
 * surfaces can consume verified snapshots/deltas instead of relying only on
 * Monaco model state.
 */

function applyLineDelta(content, delta) {
  const lines = content.split('\n');
  const replacement = delta.text.length > 0 ? delta.text.split('\n') : [];
  lines.splice(delta.start_line, delta.end_line - delta.start_line, ...replacement);
  return lines.join('\n');
}

function countLines(content) {
  return content.split('\n').length;
}

const DEFAULT_HEARTBEAT_MS = 100;

export class WasmBufferSyncManager {
  constructor(invoke, wasmMod, heartbeatMs = DEFAULT_HEARTBEAT_MS) {
    this.invoke = invoke;
    this.wasmMod = wasmMod;
    this.buffers = new Map();
    this.heartbeatTimers = new Map();
    this.syncBarriers = new Map();
    this.heartbeatMs = heartbeatMs;
  }

  /**
   * Return true if the given path has a pending sync barrier
   * (i.e. tokenization should be paused for this buffer).
   */
  isSyncBarrierActive(path) {
    return !!this.syncBarriers.get(path);
  }

  /**
   * Debounced refresh for event-driven bulk edits.
   * Resets a heartbeat timer on every call; the actual sync happens
   * only after `heartbeatMs` of quiescence. This batches rapid agent
   * edits into a single WASM sync + tokenization cycle.
   */
  async refreshDebounced(path) {
    if (this.heartbeatTimers.has(path)) {
      clearTimeout(this.heartbeatTimers.get(path));
      this.heartbeatTimers.delete(path);
    }

    this.syncBarriers.set(path, true);

    return new Promise((resolve) => {
      const timer = setTimeout(async () => {
        this.heartbeatTimers.delete(path);
        try {
          const result = await this.refresh(path);
          this.syncBarriers.delete(path);
          resolve(result);
        } catch (error) {
          this.syncBarriers.delete(path);
          resolve(null);
        }
      }, this.heartbeatMs);
      this.heartbeatTimers.set(path, timer);
    });
  }

  /**
   * Flush all pending heartbeat timers immediately.
   * Call this before actions that need up-to-date shadow state.
   */
  async flush() {
    const timers = Array.from(this.heartbeatTimers.entries());
    this.heartbeatTimers.clear();
    for (const [path, timer] of timers) {
      clearTimeout(timer);
      this.syncBarriers.set(path, true);
      try {
        await this.refresh(path);
      } catch (e) {
        // ignore
      }
      this.syncBarriers.delete(path);
    }
  }

  async prime(path) {
    const response = await this.invoke('wasm_sync_state_json', {
      request: {
        path,
        previous_content: null,
        previous_version_id: null,
      },
    });
    this._consume(path, response);
    return this.buffers.get(path) || null;
  }

  async refresh(path) {
    const previous = this.buffers.get(path);
    const response = await this.invoke('wasm_sync_state_json', {
      request: {
        path,
        previous_content: previous ? previous.content : null,
        previous_version_id: previous ? previous.versionId : null,
      },
    });
    this._consume(path, response);
    return this.buffers.get(path) || null;
  }

  async primeBinary(path) {
    const binary = await this.invoke('wasm_sync_state_binary', {
      request: {
        path,
        previous_content: null,
        previous_version_id: null,
      },
    });
    if (!this.wasmMod) {
      throw new Error('WASM module not loaded; call initWasmCompute() first');
    }
    const response = this.wasmMod.apply_binary_delta(binary);
    this._consumeBinary(path, response);
    return this.buffers.get(path) || null;
  }

  async refreshBinary(path) {
    const previous = this.buffers.get(path);
    const binary = await this.invoke('wasm_sync_state_binary', {
      request: {
        path,
        previous_content: previous ? previous.content : null,
        previous_version_id: previous ? previous.versionId : null,
      },
    });
    if (!this.wasmMod) {
      throw new Error('WASM module not loaded; call initWasmCompute() first');
    }
    const response = this.wasmMod.apply_binary_delta(binary);
    this._consumeBinary(path, response);
    return this.buffers.get(path) || null;
  }

  clear(path) {
    if (this.heartbeatTimers.has(path)) {
      clearTimeout(this.heartbeatTimers.get(path));
      this.heartbeatTimers.delete(path);
    }
    this.syncBarriers.delete(path);
    this.buffers.delete(path);
    if (this.wasmMod && typeof this.wasmMod.invalidate_token_cache === 'function') {
      this.wasmMod.invalidate_token_cache(path);
    }
  }

  getBuffer(path) {
    return this.buffers.get(path) || null;
  }

  getContent(path) {
    const entry = this.buffers.get(path);
    return entry ? entry.content : null;
  }

  _consume(path, response) {
    if (!response || !response.mode) {
      return;
    }

    if (response.mode === 'snapshot' && response.snapshot) {
      this.buffers.set(path, {
        path,
        content: response.snapshot.content,
        versionId: response.snapshot.version_id,
        lineCount: response.snapshot.line_count,
      });
      return;
    }

    if (response.mode === 'delta' && response.delta) {
      const previous = this.buffers.get(path);
      if (!previous) {
        return;
      }
      const content = applyLineDelta(previous.content, response.delta);
      this.buffers.set(path, {
        path,
        content,
        versionId: response.delta.version_id,
        lineCount: countLines(content),
      });
    }
  }

  _consumeBinary(path, response) {
    if (!response || !response.mode) {
      return;
    }

    if (response.mode === 'snapshot' && response.content != null) {
      this.buffers.set(path, {
        path,
        content: response.content,
        versionId: response.version_id,
        lineCount: response.line_count ?? countLines(response.content),
      });
      return;
    }

    if (response.mode === 'delta' && response.text != null) {
      const previous = this.buffers.get(path);
      if (!previous) {
        return;
      }
      const content = applyLineDelta(previous.content, {
        start_line: response.start_line,
        end_line: response.end_line,
        text: response.text,
      });
      this.buffers.set(path, {
        path,
        content,
        versionId: response.version_id,
        lineCount: countLines(content),
      });
    }
  }
}
