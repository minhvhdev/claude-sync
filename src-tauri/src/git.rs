use git2::{Cred, RemoteCallbacks, Repository, Signature, ResetType};
use std::path::Path;
use log::info;

pub struct GitEngine {
    repo: Option<Repository>,
    repo_path: String,
    token: String,
    _sync_items: Vec<&'static str>,
}

impl GitEngine {
    pub fn new(repo_path: String, token: String) -> Self {
        Self {
            repo: None,
            repo_path,
            token,
            _sync_items: vec![
                "settings.json",
                "CLAUDE.md",
                "commands",
                "agents",
                ".claude.json",
            ],
        }
    }

    pub fn clone_or_open(&mut self, url: &str) -> Result<(), String> {
        let path = Path::new(&self.repo_path);

        if path.exists() && Repository::open(path).is_ok() {
            info!("Opening existing repository at {}", self.repo_path);
            self.repo = Repository::open(path).ok();
            return self.pull();
        }

        info!("Cloning repository from {}", url);
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, _username, _cred_type| {
            Cred::userpass_plaintext("x-access-token", &self.token)
        });

        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(callbacks);

        match git2::Repository::clone(url, path) {
            Ok(repo) => {
                self.repo = Some(repo);
                info!("Repository cloned successfully");
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to clone repository: {}", e);
                Err(format!("Failed to clone: {}", e))
            }
        }
    }

    pub fn pull(&self) -> Result<(), String> {
        let repo = self.repo.as_ref().ok_or("Repository not initialized")?;

        info!("Pulling from origin");

        // Fetch from origin
        let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, _username, _cred_type| {
            Cred::userpass_plaintext("x-access-token", &self.token)
        });
        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(callbacks);

        remote.fetch(&["main"], Some(&mut fo), None).map_err(|e| e.to_string())?;

        // Get the remote commit
        let head = repo.head().map_err(|e| e.to_string())?;
        let remote_oid = head.target().ok_or("No head oid")?;

        // Perform a reset to the remote commit
        let commit = repo.find_commit(remote_oid).map_err(|e| e.to_string())?;
        repo.reset(&commit.into_object(), ResetType::Hard, None)
            .map_err(|e| e.to_string())?;

        info!("Pull completed");
        Ok(())
    }

    pub fn push(&self, message: &str) -> Result<(), String> {
        let repo = self.repo.as_ref().ok_or("Repository not initialized")?;

        info!("Pushing with message: {}", message);

        // Stage all changes
        let mut index = repo.index().map_err(|e| e.to_string())?;
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| e.to_string())?;
        index.write().map_err(|e| e.to_string())?;

        // Create commit if there are changes
        let statuses = repo.statuses(None).map_err(|e| e.to_string())?;
        if statuses.is_empty() {
            info!("No changes to commit");
            return Ok(());
        }

        let tree_id = index.write_tree().map_err(|e| e.to_string())?;
        let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

        let signature = Signature::now("Claude Sync", "sync@claude.ai")
            .map_err(|e| e.to_string())?;

        let head = repo.head().map_err(|e| e.to_string())?;
        let parent = head.peel_to_commit().map_err(|e| e.to_string())?;

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent],
        ).map_err(|e| e.to_string())?;

        // Push
        let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, _username, _cred_type| {
            Cred::userpass_plaintext("x-access-token", &self.token)
        });
        let mut push_opts = git2::PushOptions::new();
        push_opts.remote_callbacks(callbacks);

        let refname = format!("refs/heads/{}", head.name().ok_or("No ref name")?.split('/').last().unwrap_or("main"));
        remote.push(&[&refname], Some(&mut push_opts)).map_err(|e| e.to_string())?;

        info!("Push completed");
        Ok(())
    }

    pub fn add_all_and_commit(&self, message: &str) -> Result<(), String> {
        let repo = self.repo.as_ref().ok_or("Repository not initialized")?;

        // Stage all changes
        let mut index = repo.index().map_err(|e| e.to_string())?;
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| e.to_string())?;
        index.write().map_err(|e| e.to_string())?;

        let tree_id = index.write_tree().map_err(|e| e.to_string())?;
        let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

        let signature = Signature::now("Claude Sync", "sync@claude.ai")
            .map_err(|e| e.to_string())?;

        let head = repo.head().map_err(|e| e.to_string())?;
        let parent = head.peel_to_commit().map_err(|e| e.to_string())?;

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent],
        ).map_err(|e| e.to_string())?;

        info!("Commit created: {}", message);
        Ok(())
    }

    pub fn _get_sync_items(&self) -> Vec<&'static str> {
        vec![
            "settings.json",
            "CLAUDE.md",
            "commands",
            "agents",
            ".claude.json",
        ]
    }

    pub fn _set_repo_path(&mut self, path: String) {
        self.repo_path = path;
    }

    pub fn _set_token(&mut self, token: String) {
        self.token = token;
    }
}
