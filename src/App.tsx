import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Setup } from "./pages/Setup";
import { Status } from "./pages/Status";
import "./App.css";

function App() {
  const [configured, setConfigured] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    checkConfiguration();
  }, []);

  const checkConfiguration = async () => {
    try {
      const repoUrl = await invoke<string | null>("get_repo_url");
      setConfigured(repoUrl !== null);
    } catch (e) {
      console.error("Failed to check configuration:", e);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="loading">
        <div className="spinner"></div>
        <p>Loading...</p>
      </div>
    );
  }

  if (!configured) {
    return <Setup onConfigured={() => setConfigured(true)} />;
  }

  return <Status />;
}

export default App;
