import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SetupProps {
  onConfigured: () => void;
}

export function Setup({ onConfigured }: SetupProps) {
  const [repoUrl, setRepoUrl] = useState("");
  const [token, setToken] = useState("");
  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const handleTestConnection = async () => {
    if (!repoUrl || !token) {
      setError("Please enter both repo URL and token");
      return;
    }

    setTesting(true);
    setError(null);

    try {
      await invoke("test_connection", { repoUrl, token });
      setSuccess(true);
      setError(null);
    } catch (e) {
      setError(`Connection failed: ${e}`);
      setSuccess(false);
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    if (!repoUrl || !token) {
      setError("Please enter both repo URL and token");
      return;
    }

    setSaving(true);
    setError(null);

    try {
      await invoke("configure", { repoUrl, token });
      onConfigured();
    } catch (e) {
      setError(`Configuration failed: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="setup-page">
      <h1>Claude Sync Setup</h1>
      <p className="subtitle">Sync your Claude Code settings across machines</p>

      <div className="form-group">
        <label htmlFor="repo-url">GitHub Repository URL</label>
        <input
          id="repo-url"
          type="text"
          placeholder="https://github.com/username/claude-settings"
          value={repoUrl}
          onChange={(e) => setRepoUrl(e.target.value)}
        />
      </div>

      <div className="form-group">
        <label htmlFor="token">Personal Access Token</label>
        <input
          id="token"
          type="password"
          placeholder="ghp_xxxxxxxxxxxx"
          value={token}
          onChange={(e) => setToken(e.target.value)}
        />
        <p className="hint">Stored securely in Windows Credential Manager</p>
      </div>

      {error && <div className="error-message">{error}</div>}
      {success && <div className="success-message">Connection successful!</div>}

      <div className="button-group">
        <button
          className="btn-secondary"
          onClick={handleTestConnection}
          disabled={testing || !repoUrl || !token}
        >
          {testing ? "Testing..." : "Test Connection"}
        </button>
        <button
          className="btn-primary"
          onClick={handleSave}
          disabled={saving || !repoUrl || !token}
        >
          {saving ? "Saving..." : "Save & Start"}
        </button>
      </div>

      <div className="info-box">
        <h3>What gets synced:</h3>
        <ul>
          <li>settings.json</li>
          <li>CLAUDE.md</li>
          <li>commands/</li>
          <li>agents/</li>
          <li>.claude.json</li>
        </ul>
        <p className="note">Your credentials and projects are never synced.</p>
      </div>
    </div>
  );
}
