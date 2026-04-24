import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface SyncState {
  status: "Synced" | "Syncing" | "Error" | "NotConfigured";
  last_sync: number | null;
  pending_changes: boolean;
  sync_credentials: boolean;
}

export function Status() {
  const [syncState, setSyncState] = useState<SyncState | null>(null);
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    loadState();

    const unlistenPush = listen("tray-push-sync", () => {
      handlePush();
    });

    const unlistenPull = listen("tray-pull-sync", () => {
      handlePull();
    });

    return () => {
      unlistenPush.then((fn) => fn());
      unlistenPull.then((fn) => fn());
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

  const handlePush = async () => {
    if (syncing) return;

    setSyncing(true);
    setErrorMsg(null);
    setSyncState((prev) => prev ? { ...prev, status: "Syncing" } : null);

    try {
      await invoke("push_sync");
      await loadState();
    } catch (e: any) {
      console.error("Push failed:", e);
      // Construct user-friendly error
      if (e && e.kind) {
          setErrorMsg(`${e.kind}: ${e.message}`);
      } else {
          setErrorMsg(String(e));
      }
      setSyncState((prev) => prev ? { ...prev, status: "Error" } : null);
    } finally {
      setSyncing(false);
    }
  };

  const handlePull = async () => {
    if (syncing) return;

    setSyncing(true);
    setErrorMsg(null);
    setSyncState((prev) => prev ? { ...prev, status: "Syncing" } : null);

    try {
      await invoke("pull_sync");
      await loadState();
    } catch (e: any) {
      console.error("Pull failed:", e);
      if (e && e.kind) {
          setErrorMsg(`${e.kind}: ${e.message}`);
      } else {
          setErrorMsg(String(e));
      }
      setSyncState((prev) => prev ? { ...prev, status: "Error" } : null);
    } finally {
      setSyncing(false);
    }
  };

  const handleToggleAutostart = async (enabled: boolean) => {
    try {
      await invoke("set_autostart", { enabled });
      setAutostartEnabled(enabled);
    } catch (e: any) {
      console.error("Failed to toggle autostart:", e);
      alert(e.message || String(e));
    }
  };

  const formatLastSync = (timestamp: number | null): string => {
    if (!timestamp) return "Never";
    const date = new Date(timestamp * 1000);
    return date.toLocaleString();
  };

  const handleToggleSyncCredentials = async (enabled: boolean) => {
    try {
      await invoke("set_sync_credentials", { enabled });
      setSyncState((prev) => prev ? { ...prev, sync_credentials: enabled } : null);
    } catch (e: any) {
      console.error("Failed to toggle sync credentials:", e);
      alert(e.message || String(e));
    }
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
        
        {errorMsg && (
            <div className="error-message" style={{ marginTop: "1rem", color: "#ef4444", fontSize: "0.9rem" }}>
                {errorMsg}
            </div>
        )}
      </div>

      <div className="actions" style={{ display: 'flex', gap: '1rem', marginBottom: '2rem' }}>
        <button
          className="btn-primary"
          onClick={handlePush}
          disabled={syncing}
        >
          {syncing ? "Working..." : "Push to GitHub"}
        </button>
        <button
          className="btn-secondary"
          onClick={handlePull}
          disabled={syncing}
        >
          {syncing ? "Working..." : "Pull from GitHub"}
        </button>
      </div>

      <div className="toggle-group">
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

        <div className="toggle-row">
          <div className="toggle-info">
            <span className="toggle-label">Sync Credentials</span>
            <span className="toggle-desc">Also sync .credentials.json</span>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={syncState?.sync_credentials ?? false}
              onChange={(e) => handleToggleSyncCredentials(e.target.checked)}
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
          <li>skills/</li>
          <li>plugins/</li>
          <li>.claude.json</li>
          <li>.credentials.json (Optional)</li>
        </ul>
      </div>
    </div>
  );
}
