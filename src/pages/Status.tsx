import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface SyncState {
  status: "Synced" | "Syncing" | "Error" | "NotConfigured";
  last_sync: number | null;
  watcher_enabled: boolean;
  pending_changes: boolean;
}

export function Status() {
  const [syncState, setSyncState] = useState<SyncState | null>(null);
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [syncing, setSyncing] = useState(false);

  useEffect(() => {
    loadState();

    const unlistenSync = listen("trigger-sync", () => {
      handleSyncNow();
    });

    const unlistenShutdown = listen("shutdown-sync", () => {
      handleShutdownSync();
    });

    return () => {
      unlistenSync.then((fn) => fn());
      unlistenShutdown.then((fn) => fn());
    };
  }, []);

  const loadState = async () => {
    try {
      const state = await invoke<SyncState>("get_sync_state");
      setSyncState(state);
    } catch (e) {
      console.error("Failed to load sync state:", e);
    }

    try {
      const autostart = await invoke<boolean>("get_autostart_enabled");
      setAutostartEnabled(autostart);
    } catch (e) {
      console.error("Failed to get autostart state:", e);
    }
  };

  const handleSyncNow = async () => {
    if (syncing) return;

    setSyncing(true);
    setSyncState((prev) => prev ? { ...prev, status: "Syncing" } : null);

    try {
      await invoke("sync_now");
      await loadState();
    } catch (e) {
      console.error("Sync failed:", e);
      setSyncState((prev) => prev ? { ...prev, status: "Error" } : null);
    } finally {
      setSyncing(false);
    }
  };

  const handleShutdownSync = async () => {
    console.log("Shutdown sync triggered");
    try {
      await invoke("sync_now");
    } catch (e) {
      console.error("Shutdown sync failed:", e);
    }
  };

  const handleToggleWatcher = async (enabled: boolean) => {
    try {
      await invoke("toggle_watcher", { enabled });
      setSyncState((prev) => prev ? { ...prev, watcher_enabled: enabled } : null);
    } catch (e) {
      console.error("Failed to toggle watcher:", e);
    }
  };

  const handleToggleAutostart = async (enabled: boolean) => {
    try {
      await invoke("set_autostart", { enabled });
      setAutostartEnabled(enabled);
    } catch (e) {
      console.error("Failed to toggle autostart:", e);
    }
  };

  const formatLastSync = (timestamp: number | null): string => {
    if (!timestamp) return "Never";
    const date = new Date(timestamp * 1000);
    return date.toLocaleString();
  };

  const getStatusColor = (status: string): string => {
    switch (status) {
      case "Synced": return "#22c55e";
      case "Syncing": return "#eab308";
      case "Error": return "#ef4444";
      default: return "#6b7280";
    }
  };

  const getStatusText = (status: string): string => {
    switch (status) {
      case "Synced": return "Synced";
      case "Syncing": return "Syncing...";
      case "Error": return "Error";
      case "NotConfigured": return "Not Configured";
      default: return status;
    }
  };

  return (
    <div className="status-page">
      <h1>Claude Sync</h1>

      <div className="status-card">
        <div className="status-indicator">
          <span
            className="status-dot"
            style={{ backgroundColor: syncState ? getStatusColor(syncState.status) : "#6b7280" }}
          />
          <span className="status-text">
            {syncState ? getStatusText(syncState.status) : "Loading..."}
          </span>
        </div>
        <p className="last-sync">Last sync: {formatLastSync(syncState?.last_sync ?? null)}</p>
      </div>

      <div className="actions">
        <button
          className="btn-primary btn-large"
          onClick={handleSyncNow}
          disabled={syncing}
        >
          {syncing ? "Syncing..." : "Sync Now"}
        </button>
      </div>

      <div className="toggle-group">
        <div className="toggle-row">
          <div className="toggle-info">
            <span className="toggle-label">Realtime Watcher</span>
            <span className="toggle-desc">Automatically sync when files change</span>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={syncState?.watcher_enabled ?? true}
              onChange={(e) => handleToggleWatcher(e.target.checked)}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>

        <div className="toggle-row">
          <div className="toggle-info">
            <span className="toggle-label">Start with Windows</span>
            <span className="toggle-desc">Launch automatically on startup</span>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={autostartEnabled}
              onChange={(e) => handleToggleAutostart(e.target.checked)}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
      </div>

      <div className="info-section">
        <h3>Synced Items</h3>
        <ul className="sync-list">
          <li>settings.json</li>
          <li>CLAUDE.md</li>
          <li>commands/</li>
          <li>agents/</li>
          <li>.claude.json</li>
        </ul>
      </div>

      {syncState?.pending_changes && (
        <div className="pending-banner">
          Pending changes will be synced on next sync
        </div>
      )}
    </div>
  );
}
