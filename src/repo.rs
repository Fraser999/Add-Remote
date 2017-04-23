use super::input_getter::{get_string, get_uint};
use super::known_users::get_users;
use find_git;
use reqwest;
use serde_json::{self, Value};
use std::collections::HashMap;
use std::io::{self, Read, Stdin};
use std::path::PathBuf;
use std::process::{self, Command};

/// Base URL for sending GET requests to GitHub for retrieving info about repositories.
const GITHUB_API: &'static str = "https://api.github.com/repos/";

/// The GitHub username of the owner of a repository or fork.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
struct Owner(pub String);

/// The GitHub name of a repository or fork.
#[derive(Default)]
struct Name(pub String);

/// The name given to a local remote.
#[derive(Default)]
struct RemoteAlias(pub String);

/// The GitHub URL of a repository of fork.
#[derive(Clone, Default)]
struct Url(pub String);

/// The main container for a repository's details.
pub struct Repo {
    /// The collection of remotes for this repository.
    local_remotes: HashMap<Owner, (Name, RemoteAlias, Url)>,
    /// The collection of known forks (and the actual main "fork" a.k.a the source) which aren't
    /// already included in `local_remotes`.
    available_forks: Vec<(Owner, Url)>,
    /// The owner of the main fork/source.
    main_fork_owner: Owner,
    /// The name of the main fork/source.
    main_fork_name: Name,
    /// The GitHub URL of the main fork/source.
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
    fn default() -> Repo {
        let git = unwrap!(find_git::git_path(), "Unable to find Git executable.");
        let mut repo = Repo {
            local_remotes: HashMap::new(),
            available_forks: Vec::new(),
            main_fork_owner: Owner::default(),
            main_fork_name: Name::default(),
            main_fork_url: Url::default(),
            git: git,
            stdin: io::stdin(),
            chosen_fork_index: 1 << 31,
            chosen_remote_alias: RemoteAlias::default(),
        };
        repo.populate_local_remotes();
        repo.populate_main_fork_details();
        repo.populate_available_forks();
        repo
    }
}

impl Repo {
    /// Whether there any further remotes which _can_ be added.
    pub fn has_no_available_forks(&self) -> bool { self.available_forks.is_empty() }

    /// Displays the collection of available forks.
    pub fn show_available_forks(&self) {
        prnt_ln!("Available forks:");
        for (index, &(ref owner, _)) in self.available_forks.iter().enumerate() {
            prnt_ln!("{:<4}{}", index, owner.0);
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
            #[cfg_attr(feature="cargo-clippy", allow(cast_possible_truncation))]
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
    pub fn choose_local_remote_alias(&mut self) {
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
                        return;
                    } else {
                        self.chosen_remote_alias = RemoteAlias(value);
                        return;
                    }
                }
            }
        }
    }

    /// Process the user's choices, i.e. add the new remote.  Also calls `git fetch` for the new
    /// remote and displays the remotes when complete.
    #[cfg_attr(feature="cargo-clippy", allow(use_debug))]
    pub fn set_remote(&self) {
        prnt_ln!("");
        let remotes_before = self.git_remote_verbose_output();

        // Add the remote.
        let chosen_url = &(self.available_forks[self.chosen_fork_index].1).0;
        let chosen_alias = &self.chosen_remote_alias.0;
        let mut command = Command::new(&self.git);
        let _ = command.args(&["remote", "add", chosen_alias, chosen_url]);
        let output = unwrap!(command.output());
        if !output.status.success() {
            red_ln!("Failed to run {:?}:", command);
            prnt_ln!("{}", String::from_utf8_lossy(&output.stdout));
            prnt_ln!("{}", String::from_utf8_lossy(&output.stderr));
            process::exit(-2);
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
    }

    /// Query GitHub's API and return the contents of the response.  Panics on failure.
    fn github_get(request: &str) -> String {
        let mut response = unwrap!(reqwest::get(request));
        if !response.status().is_success() {
            panic!("\nFailed to GET {}\nResponse status: {}\nResponse headers:\n{}",
                   request,
                   response.status(),
                   response.headers());
        }
        let mut content = String::new();
        let _ = unwrap!(response.read_to_string(&mut content));
        content
    }

    /// Calls `git remote show` and `git remote get-url <name>` for each remote found to populate
    /// `local_remotes`.  If the initial Git command fails, we assume it's because this process is
    /// not being executed from within a Git repository, so we print an error message to that effect
    /// exit with a non-zero code.
    fn populate_local_remotes(&mut self) {
        let local_remotes_output = unwrap!(Command::new(&self.git)
                                               .args(&["remote", "show"])
                                               .output());
        // Get list of local remotes.
        if !local_remotes_output.status.success() {
            red_ln!("Failed to execute 'git remote show'.  Execute this program from inside a Git \
                    repository.");
            process::exit(-1);
        }
        let stdout = String::from_utf8_lossy(&local_remotes_output.stdout);
        let local_remotes = stdout.trim().to_string();

        // For each, get the URL, and break this down to get the owner too.
        for remote_alias in local_remotes.lines() {
            let url_output = unwrap!(Command::new(&self.git)
                                         .args(&["remote", "get-url", remote_alias])
                                         .output());
            assert!(url_output.status.success(),
                    "Failed to run 'git remote get-url {}'",
                    remote_alias);
            let stdout = String::from_utf8_lossy(&url_output.stdout);
            let url = stdout.trim().to_string();
            let owner;
            let name;
            {
                let mut owner_and_repo = url.trim_left_matches("git@github.com:");
                owner_and_repo = owner_and_repo.trim_left_matches("https://github.com/");
                owner_and_repo = owner_and_repo.trim_right_matches(".git");
                let mut split_itr = owner_and_repo.split('/');
                owner = Owner(unwrap!(split_itr.next()).to_string());
                name = Name(unwrap!(split_itr.next()).to_string());
            }
            let _ = self.local_remotes
                .insert(owner, (name, RemoteAlias(remote_alias.to_string()), Url(url)));
        }
    }

    /// Send GET to GitHub to allow retrieval of the main fork/source's details.
    fn populate_main_fork_details(&mut self) {
        let (owner, &(ref name, ..)) = unwrap!(self.local_remotes.iter().next());
        let request = format!("{}{}/{}", GITHUB_API, owner.0, name.0);
        let response = Self::github_get(&request);
        let response_as_json: Value = unwrap!(serde_json::from_str(&response));
        self.main_fork_owner = match response_as_json["source"]["owner"]["login"] {
            Value::Null => Owner(unwrap!(response_as_json["owner"]["login"].as_str()).to_string()),
            Value::String(ref owner) => Owner(owner.trim_matches('"').to_string()),
            _ => unreachable!(),
        };
        self.main_fork_name = match response_as_json["source"]["name"] {
            Value::Null => Name(unwrap!(response_as_json["name"].as_str()).to_string()),
            Value::String(ref name) => Name(name.trim_matches('"').to_string()),
            _ => unreachable!(),
        };
        self.main_fork_url = match response_as_json["source"]["html_url"] {
            Value::Null => Url(unwrap!(response_as_json["html_url"].as_str()).to_string()),
            Value::String(ref url) => Url(url.trim_matches('"').to_string()),
            _ => unreachable!(),
        };
    }

    /// Send GET to GitHub to retrieve the list of forks and their details.
    fn populate_available_forks(&mut self) {
        let request = format!("{}{}/{}/forks?per_page=100",
                              GITHUB_API,
                              self.main_fork_owner.0,
                              self.main_fork_name.0);
        let response = Self::github_get(&request);
        let response_as_json: Value = unwrap!(serde_json::from_str(&response));
        if let Value::Array(values) = response_as_json {
            for value in &values {
                let owner = Owner(unwrap!(value["owner"]["login"].as_str()).to_string());
                let url = Url(unwrap!(value["html_url"].as_str()).to_string());
                if !self.local_remotes.contains_key(&owner) {
                    self.available_forks.push((owner, url));
                }
            }
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
    /// the available one if there is only one available, then the main fork/source owner, then
    /// "maidsafe", otherwise returns `None`.
    fn suggest_fork(&self) -> Option<u64> {
        // Return 0 if there's only one available.
        if self.available_forks.len() == 1 {
            return Some(0);
        }
        // Choose the main fork/source owner if available.
        if let Ok(index) = self.available_forks
               .binary_search_by_key(&self.main_fork_owner.0.to_lowercase(),
                                     |&(ref owner, _)| owner.0.to_lowercase()) {
            return Some(index as u64);
        }
        // Next look for "maidsafe".
        if let Ok(index) = self.available_forks
               .binary_search_by_key(&"maidsafe".to_string(),
                                     |&(ref owner, _)| owner.0.to_lowercase()) {
            return Some(index as u64);
        }
        None
    }

    /// Suggests a name to use for the remote.  Uses "upstream" if the chosen fork is the main fork/
    /// source, then falls back to the map of known users, and finally suggests the owner name.
    fn suggest_alias(&self) -> String {
        let chosen_owner = &self.available_forks[self.chosen_fork_index].0;
        if *chosen_owner == self.main_fork_owner {
            return "upstream".to_string();
        }
        if let Some(alias) = get_users().get(&chosen_owner.0) {
            alias.clone()
        } else {
            chosen_owner.0.clone()
        }
    }
}
