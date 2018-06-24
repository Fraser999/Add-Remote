use super::input_getter::{get_bool, get_string, get_uint};
use find_git;
use reqwest;
use reqwest::header::{Link, RelationType};
use serde_json::{self, Value};
use std::collections::HashMap;
use std::io::{self, Read, Stdin};
use std::path::PathBuf;
use std::process::{self, Command};

/// Base URL for sending GET requests to GitLab for retrieving info about repositories.
const GITLAB_API: &str = "https://gitlab.com/api/v4/projects/";
/// Base URL for sending GET requests to GitHub for retrieving info about repositories.
const GITHUB_API: &str = "https://api.github.com/repos/";

/// The Gitlab/GitHub username of the owner of a repository or fork.
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
struct Owner(pub String);

/// The Gitlab/GitHub name of a repository or fork.
#[derive(Clone, Default, Debug)]
struct Name(pub String);

/// The name given to a local remote.
#[derive(Default)]
struct RemoteAlias(pub String);

/// The URL of a repository of fork.
#[derive(Clone)]
enum Url {
    GitLab(String),
    GitHub(String),
}

impl Url {
    fn new(url: &str) -> Option<(Self, Owner, Name)> {
        let mut owner_and_repo = url.trim_left_matches("git@gitlab.com:");
        owner_and_repo = owner_and_repo.trim_left_matches("https://gitlab.com/");
        let checked_url = if owner_and_repo == url {
            owner_and_repo = url.trim_left_matches("git@github.com:");
            owner_and_repo = owner_and_repo.trim_left_matches("https://github.com/");
            if owner_and_repo == url {
                return None;
            }
            Url::GitHub(url.to_string())
        } else {
            Url::GitLab(url.to_string())
        };
        owner_and_repo = owner_and_repo.trim_right_matches(".git");
        let (owner, name) = Self::split_owner_and_repo(owner_and_repo);
        Some((checked_url, owner, name))
    }

    fn split_owner_and_repo(owner_and_repo: &str) -> (Owner, Name) {
        let mut split_itr = owner_and_repo.splitn(2, '/');
        (
            Owner(unwrap!(split_itr.next()).to_string()),
            Name(unwrap!(split_itr.next()).to_string()),
        )
    }

    fn value(&self) -> &str {
        match self {
            Url::GitLab(url) | Url::GitHub(url) => &url,
        }
    }
}

/// The main container for a repository's details.
pub struct Repo {
    /// The GitLab Personal Access Token taken from git config.
    gitlab_token: Option<String>,
    /// The GitHub Personal Access Token taken from git config.
    github_token: Option<String>,
    /// The collection of remotes for this repository.
    local_remotes: HashMap<Owner, (Name, RemoteAlias, Url)>,
    /// The collection of known forks (and the actual main "fork" a.k.a the source) which aren't
    /// already included in `local_remotes`.
    available_forks: Vec<(Owner, Url)>,
    /// The owner of the main fork/source.
    main_fork_owner: Owner,
    /// The name of the main fork/source.
    main_fork_name: Name,
    /// The URL of the main fork/source.
    main_fork_url: Url,
    /// The full path to the Git binary.
    git: PathBuf,
    /// Console's stdin stream.
    stdin: Stdin,
    /// The index of `available_forks` chosen by the user for addition as a remote.
    chosen_fork_index: usize,
    /// The name chosen by the user to use when adding the new remote.
    chosen_remote_alias: RemoteAlias,
}

impl Default for Repo {
    fn default() -> Self {
        let mut repo = Self::new_uninitialised();
        repo.gitlab_token = repo.get_from_gitconfig("add-remote.gitLabToken");
        repo.github_token = repo.get_from_gitconfig("add-remote.gitHubToken");
        repo.populate_local_remotes();
        repo.populate_main_fork_details();
        repo.populate_available_forks();
        repo
    }
}

impl Repo {
    /// Whether there any further remotes which _can_ be added.
    pub fn has_no_available_forks(&self) -> bool {
        self.available_forks.is_empty()
    }

    /// Displays the collection of available forks.
    pub fn show_available_forks(&self) {
        prnt_ln!("Available forks:");
        let first_column_width = self.available_forks.len().to_string().len() + 2;
        for (index, &(ref owner, _)) in self.available_forks.iter().enumerate() {
            prnt_ln!("{:<width$}{}", index, owner.0, width = first_column_width);
        }
    }

    /// Runs `git remote -v` and returns the output.
    pub fn git_remote_verbose_output(&self) -> String {
        let output = unwrap!(Command::new(&self.git).args(&["remote", "-v"]).output());
        assert!(output.status.success(), "Failed to run 'git remote -v'");
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.trim().to_string()
    }

    /// Ask the user to chooses which available fork to add as a new remote.
    pub fn choose_fork(&mut self) {
        let default = self.suggest_fork();
        loop {
            if let Some(value) = default {
                yellow!("Choose fork (enter index number) [{}]: ", value);
            } else {
                yellow!("Choose fork (enter index number): ");
            }
            #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
            match get_uint(&mut self.stdin.lock(), default) {
                Err(error) => {
                    red_ln!("{}", error);
                }
                Ok(value) if value < self.available_forks.len() as u64 => {
                    self.chosen_fork_index = value as usize;
                    return;
                }
                Ok(_) => {
                    red_ln!("Must be one of the listed indices.");
                }
            }
        }
    }

    /// Ask the user to choose the name for the new remote.
    pub fn choose_local_remote_alias(&mut self) -> bool {
        let default = self.suggest_alias();
        loop {
            yellow!("Choose name to assign to remote [{}]: ", default);
            match get_string(&mut self.stdin.lock()) {
                Err(error) => {
                    red_ln!("{}", error);
                }
                Ok(value) => {
                    if value.is_empty() {
                        self.chosen_remote_alias = RemoteAlias(default);
                        return false;
                    } else {
                        self.chosen_remote_alias = RemoteAlias(value);
                        return true;
                    }
                }
            }
        }
    }

    /// Ask the user whether to add the alias to the global git-config and if so, then try to add
    /// it.
    pub fn offer_to_set_alias(&self) {
        let fork_name = &(self.available_forks[self.chosen_fork_index].0).0;
        let alias = &self.chosen_remote_alias.0;
        loop {
            yellow!(
                "Do you want to set this alias '{}' -> '{}' in your global git-config? [Y/n]: ",
                fork_name,
                alias
            );
            match get_bool(&mut self.stdin.lock(), Some(true)) {
                Err(error) => {
                    red_ln!("{}", error);
                }
                Ok(false) => return,
                Ok(true) => {
                    let git_config_arg = format!("add-remote.forkAlias.{}", fork_name);
                    let output = unwrap!(
                        Command::new(&self.git)
                            .args(&[
                                "config",
                                "--global",
                                "--replace-all",
                                &git_config_arg,
                                alias
                            ])
                            .output()
                    );
                    if output.status.success() {
                        green_ln!(
                            "Alias '{}' -> '{}' successfully set in your global git-config",
                            fork_name,
                            alias
                        );
                    } else {
                        red_ln!(
                            "Failed to run 'git config --global --replace-all {} {}'",
                            git_config_arg,
                            alias
                        );
                    }
                    return;
                }
            }
        }
    }

    /// Process the user's choices, i.e. add the new remote.  Also calls `git fetch` for the new
    /// remote and displays the remotes when complete.
    #[cfg_attr(feature = "cargo-clippy", allow(use_debug))]
    pub fn set_remote(&self) {
        prnt_ln!("");
        let remotes_before = self.git_remote_verbose_output();

        // Add the remote.
        let chosen_url = self.available_forks[self.chosen_fork_index].1.value();
        let chosen_alias = &self.chosen_remote_alias.0;
        let mut command = Command::new(&self.git);
        let _ = command.args(&["remote", "add", chosen_alias, chosen_url]);
        let output = unwrap!(command.output());
        if !output.status.success() {
            red_ln!("Failed to run {:?}:", command);
            prnt_ln!("{}", String::from_utf8_lossy(&output.stdout));
            prnt_ln!("{}", String::from_utf8_lossy(&output.stderr));
            process::exit(-4);
        }

        // Disable pushing for the new remote.
        command = Command::new(&self.git);
        let _ = command.args(&["remote", "set-url", "--push", chosen_alias, "disable_push"]);
        let output = unwrap!(command.output());
        assert!(output.status.success());

        // Fetch from the new remote.
        cyan_ln!("Fetching from {}\n", chosen_url);
        command = Command::new(&self.git);
        let _ = command.args(&["fetch", chosen_alias]);
        let output = unwrap!(command.output());
        assert!(output.status.success());

        // Display the remotes, with the new one highlighted in green.
        let remotes_after = self.git_remote_verbose_output();
        let mut before_itr = remotes_before.lines();
        let mut line_before = before_itr.next();
        for line in remotes_after.lines() {
            if line_before.unwrap_or_default() == line {
                prnt_ln!("{}", line);
                line_before = before_itr.next();
            } else {
                dark_cyan_ln!("{}", line);
            }
        }

        let mut branches = self.git_branch_verbose_output(chosen_alias);
        if branches.is_empty() {
            branches = self.git_branch_verbose_output(&chosen_alias.to_lowercase());
        }
        prnt_ln!("\n{}", branches);
    }

    fn new_uninitialised() -> Self {
        let git = unwrap!(find_git::git_path(), "Unable to find Git executable.");
        Self {
            gitlab_token: None,
            github_token: None,
            local_remotes: HashMap::new(),
            available_forks: Vec::new(),
            main_fork_owner: Owner::default(),
            main_fork_name: Name::default(),
            main_fork_url: Url::GitLab(String::new()),
            git,
            stdin: io::stdin(),
            chosen_fork_index: 1 << 31,
            chosen_remote_alias: RemoteAlias::default(),
        }
    }

    /// Query GitHub's API and return the contents of the response along with an optional link to
    /// the next page if one exists.  Panics on failure.
    fn send_get(request: &str) -> (String, Option<String>) {
        let mut response = unwrap!(reqwest::get(request));
        if !response.status().is_success() {
            panic!(
                "\nFailed to GET {}\nResponse status: {}\nResponse headers:\n{}\n\nNote that \
                 Personal Access Tokens are required in some cases.\nFor full details, see \
                 https://github.com/Fraser999/Add-Remote#personal-access-tokens.",
                request,
                response.status(),
                response.headers()
            );
        }
        let next_page_link: Option<String> = response.headers().get::<Link>().and_then(|link| {
            let link_to_next = link.values().iter().find(|&link_value| {
                link_value.rel().map_or(false, |relation_types| {
                    relation_types
                        .iter()
                        .any(|relation_type| *relation_type == RelationType::Next)
                })
            });
            link_to_next.map(|link_value| link_value.link().to_string())
        });
        let mut content = String::new();
        let _ = unwrap!(response.read_to_string(&mut content));
        (content, next_page_link)
    }

    /// Calls `git remote show` and `git remote get-url <name>` for each remote found to populate
    /// `local_remotes`.  If the initial Git command fails, we assume it's because this process is
    /// not being executed from within a Git repository, so we print an error message to that effect
    /// exit with a non-zero code.
    fn populate_local_remotes(&mut self) {
        let local_remotes_output =
            unwrap!(Command::new(&self.git).args(&["remote", "show"]).output());
        // Get list of local remotes.
        if !local_remotes_output.status.success() {
            red_ln!(
                "Failed to execute 'git remote show'.  Execute this program from inside a Git \
                 repository."
            );
            process::exit(-1);
        }
        let stdout = String::from_utf8_lossy(&local_remotes_output.stdout);
        let local_remotes = stdout.trim().to_string();

        // For each, get the URL, and break this down to get the owner too.
        for remote_alias in local_remotes.lines() {
            let url_output = unwrap!(
                Command::new(&self.git)
                    .args(&["remote", "get-url", remote_alias])
                    .output()
            );
            assert!(
                url_output.status.success(),
                "Failed to run 'git remote get-url {}'",
                remote_alias
            );
            let stdout = String::from_utf8_lossy(&url_output.stdout);
            if let Some((url, owner, name)) = Url::new(stdout.trim()) {
                let _ = self.local_remotes
                    .insert(owner, (name, RemoteAlias(remote_alias.to_string()), url));
            } else {
                continue;
            }
        }
        if self.local_remotes.is_empty() {
            red_ln!(
                "This repository doesn't appear to be hosted on GitLab or GitHub.  'add-remote' \
                 can only be used with GitLab or GitHub projects."
            );
            process::exit(-2);
        }
    }

    /// Send GET to Gitlab/GitHub to allow retrieval of the main fork/source's details.
    fn populate_main_fork_details(&mut self) {
        let (owner, name, url) = unwrap!(
            self.local_remotes
                .iter()
                .map(|(owner, (name, _, url))| (owner.clone(), name.clone(), url.clone()))
                .next()
        );
        match url {
            Url::GitLab(_) => {
                if self.gitlab_token.is_none() {
                    red_ln!(
                        "This repository is hosted on GitLab.  To use 'add-remote' with a GitLab \
                         project, you must add a GitLab Personal Access Token with \"api\" scope \
                         to your git config under the key 'add-remote.gitLabToken'.  For full \
                         details, see \
                         https://github.com/Fraser999/Add-Remote#personal-access-tokens."
                    );
                    process::exit(-3);
                };
                self.main_fork_owner = owner;
                self.main_fork_name = name;
                while self.get_gitlab_parent() {}
            }
            Url::GitHub(_) => {
                let mut request = format!("{}{}/{}", GITHUB_API, owner.0, name.0);
                if let Some(ref token) = self.github_token {
                    request = format!("{}?access_token={}", request, token);
                }
                let response = Self::send_get(&request).0;
                let response_as_json: Value = unwrap!(serde_json::from_str(&response));
                self.main_fork_owner = match response_as_json["source"]["owner"]["login"] {
                    Value::Null => {
                        Owner(unwrap!(response_as_json["owner"]["login"].as_str()).to_string())
                    }
                    Value::String(ref owner) => Owner(owner.trim_matches('"').to_string()),
                    _ => unreachable!(),
                };
                self.main_fork_name = match response_as_json["source"]["name"] {
                    Value::Null => Name(unwrap!(response_as_json["name"].as_str()).to_string()),
                    Value::String(ref name) => Name(name.trim_matches('"').to_string()),
                    _ => unreachable!(),
                };
                self.main_fork_url = match response_as_json["source"]["ssh_url"] {
                    Value::Null => {
                        Url::GitHub(unwrap!(response_as_json["ssh_url"].as_str()).to_string())
                    }
                    Value::String(ref url) => Url::GitHub(url.trim_matches('"').to_string()),
                    _ => unreachable!(),
                };
            }
        }
    }

    /// If the GitLab repo defined by `self.main_fork_owner` and `self.main_fork_name` is a fork,
    /// these values are updated to those of the forked-from project and `true` is returned.
    /// Otherwise, if it's not a fork they are left unmodified, `self.main_fork_url` is set, and
    /// `false` is returned.
    fn get_gitlab_parent(&mut self) -> bool {
        let request = format!(
            "{}{}%2F{}?private_token={}",
            GITLAB_API,
            self.main_fork_owner.0,
            self.main_fork_name.0.replace("/", "%2F"),
            unwrap!(self.gitlab_token.as_ref())
        );
        let response = Self::send_get(&request).0;
        let response_as_json: Value = unwrap!(serde_json::from_str(&response));
        if let Value::Null = response_as_json["forked_from_project"] {
            self.main_fork_url =
                Url::GitLab(unwrap!(response_as_json["ssh_url_to_repo"].as_str()).to_string());
            return false;
        }
        let (owner, name) = Url::split_owner_and_repo(unwrap!(
            response_as_json["forked_from_project"]["path_with_namespace"].as_str()
        ));
        self.main_fork_owner = owner;
        self.main_fork_name = name;
        true
    }

    /// Send GET to Gitlab/GitHub to retrieve the list of forks and their details.
    fn populate_available_forks(&mut self) {
        let first_url = unwrap!(
            self.local_remotes
                .values()
                .map(|(_, _, url)| url.clone())
                .next()
        );
        let mut optional_request = match first_url {
            Url::GitLab(_) => Some(format!(
                "{}{}%2F{}/forks?per_page=200;private_token={}",
                GITLAB_API,
                self.main_fork_owner.0,
                self.main_fork_name.0.replace("/", "%2F"),
                unwrap!(self.gitlab_token.as_ref())
            )),
            Url::GitHub(_) => {
                let mut request = format!(
                    "{}{}/{}/forks?per_page=100",
                    GITHUB_API, self.main_fork_owner.0, self.main_fork_name.0
                );
                if let Some(ref token) = self.github_token {
                    request = format!("{};access_token={}", request, token);
                }
                Some(request)
            }
        };

        while let Some(request) = optional_request {
            let (response, next_page_link) = Self::send_get(&request);
            let response_as_json: Value = unwrap!(serde_json::from_str(&response));
            if let Value::Array(values) = response_as_json {
                for value in &values {
                    let (owner, url) = match first_url {
                        Url::GitLab(_) => {
                            let (owner, _) = Url::split_owner_and_repo(unwrap!(
                                value["path_with_namespace"].as_str()
                            ));
                            let url = unwrap!(value["ssh_url_to_repo"].as_str()).to_string();
                            let subfork_count = unwrap!(value["forks_count"].as_u64());
                            if owner != self.main_fork_owner && subfork_count > 0 {
                                yellow_ln!(
                                    "{} which is a fork of {} has {} fork{} being ignored.",
                                    url,
                                    self.main_fork_url.value(),
                                    subfork_count,
                                    if subfork_count > 1 { "s" } else { "" },
                                );
                            }
                            (owner, Url::GitLab(url))
                        }
                        Url::GitHub(_) => {
                            let owner = unwrap!(value["owner"]["login"].as_str()).to_string();
                            let url = unwrap!(value["ssh_url"].as_str()).to_string();
                            (Owner(owner), Url::GitHub(url))
                        }
                    };
                    if !self.local_remotes.contains_key(&owner) {
                        self.available_forks.push((owner, url));
                    }
                }
            }
            optional_request = next_page_link;
        }
        // Add the main fork/source's details too if required.
        if !self.local_remotes.contains_key(&self.main_fork_owner) {
            self.available_forks
                .push((self.main_fork_owner.clone(), self.main_fork_url.clone()));
        }
        self.available_forks
            .sort_by_key(|&(ref owner, _)| owner.0.to_lowercase());
    }

    /// Suggests an index of `available_forks` to use as a default for the user's choice.  Favours
    /// the available one if there is only one available, then the main fork/source owner, then the
    /// Git config value of `add-remote.preferredFork` if it's set, otherwise returns `None`.
    fn suggest_fork(&self) -> Option<u64> {
        // Return 0 if there's only one available.
        if self.available_forks.len() == 1 {
            return Some(0);
        }
        // Choose the main fork/source owner if available.
        if let Ok(index) = self.available_forks
            .binary_search_by_key(&self.main_fork_owner.0.to_lowercase(), |&(ref owner, _)| {
                owner.0.to_lowercase()
            }) {
            return Some(index as u64);
        }
        // Next look for `add-remote.preferredFork` in Git config.
        self.get_from_gitconfig("add-remote.preferredFork")
            .and_then(|preferred| {
                self.available_forks
                    .binary_search_by_key(&preferred, |&(ref owner, _)| owner.0.to_lowercase())
                    .ok()
            })
            .map(|index| index as u64)
    }

    /// Suggests a name to use for the remote.  Uses the Git config value for
    /// `add-remote.mainForkOwnerAlias` (or "upstream" if this is not set) if the chosen fork is the
    /// main fork/source, then falls back to the map of known users (entries under the Git config
    /// subkey of `add-remote.forkAlias`), and finally suggests the owner name.
    fn suggest_alias(&self) -> String {
        let chosen_owner = &self.available_forks[self.chosen_fork_index].0;
        let alias_arg = if *chosen_owner == self.main_fork_owner {
            "add-remote.mainForkOwnerAlias".to_string()
        } else {
            format!("add-remote.forkAlias.{}", chosen_owner.0)
        };
        self.get_from_gitconfig(&alias_arg).unwrap_or_else(|| {
            if *chosen_owner == self.main_fork_owner {
                "upstream".to_string()
            } else {
                chosen_owner.0.clone()
            }
        })
    }

    fn get_from_gitconfig(&self, key: &str) -> Option<String> {
        let output = unwrap!(Command::new(&self.git).args(&["config", key]).output());
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }

    /// Runs `git branch --list <Alias>/* -vr --sort=-committerdate` and returns the output.
    fn git_branch_verbose_output(&self, alias: &str) -> String {
        let alias_arg = format!("{}/*", alias);
        let output = unwrap!(
            Command::new(&self.git)
                .args(&[
                    "branch",
                    "--list",
                    &alias_arg,
                    "-vr",
                    "--sort=-committerdate"
                ])
                .output()
        );
        assert!(
            output.status.success(),
            "Failed to run 'git branch --list {} -vr --sort=-committerdate'",
            alias_arg
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn populate_available_forks() {
        let mut repo = Repo::new_uninitialised();
        let _ = repo.local_remotes.insert(
            Owner("Fraser999".to_string()),
            (
                Name("cargo".to_string()),
                RemoteAlias("origin".to_string()),
                Url::GitHub("git@github.com:Fraser999/cargo.git".to_string()),
            ),
        );
        repo.populate_main_fork_details();
        repo.populate_available_forks();
        repo.show_available_forks();
        assert!(repo.available_forks.len() > 100);
    }
}
