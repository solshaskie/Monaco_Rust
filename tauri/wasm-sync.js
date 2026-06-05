/**
 * WASM buffer sync manager.
 *
 * Keeps a frontend-side shadow of Rust-owned buffer truth so WASM compute
 * surfaces can consume verified snapshots/deltas instead of relying only on
 * Monaco model state.
 */

function applyLineDelta(content, delta) {
  // Incremental splice: find byte offsets of start/end lines instead of
  // splitting the entire file into an array. O(replaced lines) vs O(total lines).
  let startOffset = 0;
  for (let i = 0; i < delta.start_line && startOffset < content.length; i++) {
    const nl = content.indexOf('\n', startOffset);
    if (nl === -1) {
      startOffset = content.length;
      break;
    }
    startOffset = nl + 1;
  }

  let endOffset = startOffset;
  const deleteCount = delta.end_line - delta.start_line;
  for (let i = 0; i < deleteCount && endOffset < content.length; i++) {
    const nl = content.indexOf('\n', endOffset);
    if (nl === -1) {
      endOffset = content.length;
      break;
    }
    endOffset = nl + 1;
  }

  return content.slice(0, startOffset) + delta.text + content.slice(endOffset);
}

function countLines(content) {
  if (!content) return 0;
  let count = 1;
  for (let i = 0; i < content.length; i++) {
    if (content[i] === '\n') count++;
  }
  return count;
}

const DEFAULT_HEARTBEAT_MS = 100;

export class WasmBufferSyncManager {
  constructor(invoke, wasmMod, heartbeatMs = DEFAULT_HEARTBEAT_MS) {
    this.invoke = invoke;
    this.wasmMod = wasmMod;
    this.buffers = new Map();
    this.heartbeatTimers = new Map();
    this.syncBarriers = new Map();
    this.listeners = new Set();
    this.heartbeatMs = heartbeatMs;
  }

  onUpdate(listener) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
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
        const previousVersionId = this.getVersionId(path);
        try {
          const result = await this._syncJson(path, this.buffers.get(path));
          this.syncBarriers.delete(path);
          this._emitBufferUpdate('refresh-settled', path, previousVersionId);
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
      const previousVersionId = this.getVersionId(path);
      try {
        await this._syncJson(path, this.buffers.get(path));
      } catch (e) {
        // ignore
      }
      this.syncBarriers.delete(path);
      this._emitBufferUpdate('flush-settled', path, previousVersionId);
    }
  }

  async prime(path) {
    const previousVersionId = this.getVersionId(path);
    const buffer = await this._syncJson(path, null);
    this._emitBufferUpdate('prime', path, previousVersionId);
    return buffer;
  }

  async refresh(path) {
    const previous = this.buffers.get(path);
    const previousVersionId = previous ? previous.versionId : null;
    const buffer = await this._syncJson(path, previous);
    this._emitBufferUpdate('refresh', path, previousVersionId);
    return buffer;
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
    const previousVersionId = this.getVersionId(path);
    if (this.heartbeatTimers.has(path)) {
      clearTimeout(this.heartbeatTimers.get(path));
      this.heartbeatTimers.delete(path);
    }
    this.syncBarriers.delete(path);
    this.buffers.delete(path);
    if (this.wasmMod && typeof this.wasmMod.invalidate_token_cache === 'function') {
      this.wasmMod.invalidate_token_cache(path);
    }
    this._emitBufferUpdate('clear', path, previousVersionId);
  }

  getBuffer(path) {
    return this.buffers.get(path) || null;
  }

  getContent(path) {
    const entry = this.buffers.get(path);
    return entry ? entry.content : null;
  }

  getVersionId(path) {
    const entry = this.buffers.get(path);
    return entry ? entry.versionId : null;
  }

  _emit(event) {
    for (const listener of this.listeners) {
      try {
        listener(event);
      } catch (error) {
        console.warn('[WasmBufferSyncManager] update listener failed', error);
      }
    }
  }

  _emitBufferUpdate(type, path, previousVersionId) {
    const buffer = this.buffers.get(path) || null;
    const versionId = buffer ? buffer.versionId : null;
    this._emit({
      type,
      path,
      buffer,
      versionId,
      previousVersionId,
      changed: previousVersionId !== versionId,
      barrierActive: this.isSyncBarrierActive(path),
    });
  }

  async _syncJson(path, previous) {
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
