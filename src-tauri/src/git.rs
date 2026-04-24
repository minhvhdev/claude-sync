use git2::{Cred, RemoteCallbacks, Repository, Signature, ResetType};
use std::path::Path;
use log::info;
use crate::error::AppError;

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
                "skills",
                "plugins",
                ".claude.json",
            ],
        }
    }

    pub fn clone_or_open(&mut self, url: &str) -> Result<(), AppError> {
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

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);

        match builder.clone(url, path) {
            Ok(repo) => {
                self.repo = Some(repo);
                info!("Repository cloned successfully");
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to clone repository: {}", e);
                Err(AppError::git(format!("Failed to clone: {}", e)))
            }
        }
    }

    pub fn pull(&self) -> Result<(), AppError> {
        let repo = self.repo.as_ref().ok_or_else(|| AppError::system("Repository not initialized"))?;

        info!("Pulling from origin");

        // Fetch from origin
        let mut remote = repo.find_remote("origin").map_err(|e| AppError::git(e.to_string()))?;
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, _username, _cred_type| {
            Cred::userpass_plaintext("x-access-token", &self.token)
        });
        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(callbacks);

        let fetch_opts_err = remote.fetch(&["main"], Some(&mut fo), None);
        if let Err(e) = fetch_opts_err {
            info!("Fetch returned error: {}. This might happen if remote is empty.", e);
            return Ok(());
        }

        // Get the remote commit
        let remote_branch = match repo.find_reference("refs/remotes/origin/main") {
            Ok(r) => r,
            Err(_) => {
                info!("No remote origin/main found. Repository might be empty.");
                return Ok(());
            }
        };

        let remote_oid = remote_branch.target().ok_or_else(|| AppError::git("No target for remote branch"))?;
        let commit = repo.find_commit(remote_oid).map_err(|e| AppError::git(e.to_string()))?;
        
        // Ensure HEAD exists or point it to main
        if repo.head().is_err() {
            repo.set_head("refs/heads/main").unwrap_or(());
        }

        repo.reset(&commit.into_object(), ResetType::Hard, None)
            .map_err(|e| AppError::git(e.to_string()))?;

        info!("Pull completed");
        Ok(())
    }

    pub fn push(&self, _message: &str) -> Result<(), AppError> {
        let repo = self.repo.as_ref().ok_or_else(|| AppError::system("Repository not initialized"))?;
        info!("Pushing changes to remote origin");

        let mut remote = repo.find_remote("origin").map_err(|e| AppError::git(e.to_string()))?;
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, _username, _cred_type| {
            Cred::userpass_plaintext("x-access-token", &self.token)
        });
        let mut push_opts = git2::PushOptions::new();
        push_opts.remote_callbacks(callbacks);

        let head = match repo.head() {
            Ok(h) => h,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                info!("Nothing to push yet (UnbornBranch).");
                return Ok(());
            }
            Err(e) => return Err(AppError::git(e.to_string())),
        };

        let branch_name = head.name().map(|n| n.split('/').last().unwrap_or("main")).unwrap_or("main");
        let refname = format!("refs/heads/{}", branch_name);
        
        // Force push format: +src:dst
        let refspec = format!("+{}:{}", refname, refname);
        
        remote.push(&[&refspec], Some(&mut push_opts)).map_err(|e| AppError::git(e.to_string()))?;

        info!("Push completed");
        Ok(())
    }

    pub fn add_all_and_commit(&self, message: &str) -> Result<(), AppError> {
        let repo = self.repo.as_ref().ok_or_else(|| AppError::system("Repository not initialized"))?;

        // Stage all changes
        let mut index = repo.index().map_err(|e| AppError::git(e.to_string()))?;
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| AppError::git(e.to_string()))?;
        index.write().map_err(|e| AppError::git(e.to_string()))?;

        // Check for changes (don't create empty commits)
        let statuses = repo.statuses(None).map_err(|e| AppError::git(e.to_string()))?;
        if statuses.is_empty() {
             info!("No changes to commit before push");
             return Ok(());
        }

        let tree_id = index.write_tree().map_err(|e| AppError::git(e.to_string()))?;
        let tree = repo.find_tree(tree_id).map_err(|e| AppError::git(e.to_string()))?;

        let signature = Signature::now("Claude Sync", "sync@claude.ai")
            .map_err(|e| AppError::git(e.to_string()))?;

        let parent_commit = match repo.head() {
            Ok(head) => Some(head.peel_to_commit().map_err(|e| AppError::git(e.to_string()))?),
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                info!("Unborn branch detected. Preparing initial commit.");
                None
            }
            Err(e) => return Err(AppError::git(e.to_string())),
        };

        let mut parents = Vec::new();
        if let Some(ref p) = parent_commit {
            parents.push(p);
        }

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        ).map_err(|e| AppError::git(e.to_string()))?;

        info!("Commit created: {}", message);
        Ok(())
    }

    pub fn _get_sync_items(&self) -> Vec<&'static str> {
        vec![
            "settings.json",
            "CLAUDE.md",
            "commands",
            "agents",
            "skills",
            "plugins",
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
