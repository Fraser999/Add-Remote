//! An interactive CLI tool to add a remote fork to a local Git repository.

#![forbid(warnings)]
#![warn(
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_results,
    variant_size_differences,
    clippy::all,
    clippy::pedantic
)]

#[macro_use]
extern crate colour;
extern crate ctrlc;
extern crate find_git;
extern crate reqwest;
extern crate serde_json;
#[macro_use]
extern crate unwrap;

/// Reads and validates input from a stream.
mod input_getter;
/// Main struct which holds the details for the current Git repository.
mod repo;

use repo::Repo;
use std::{env, process};

/// Main function.
fn main() {
    unwrap!(
        ctrlc::set_handler(move || process::exit(0)),
        "Error setting Ctrl-C handler"
    );
    let args: Vec<_> = env::args().collect();

    if args
        .iter()
        .any(|arg| arg == "-h" || arg == "/?" || arg == "--help")
    {
        return print_help();
    }

    if args
        .iter()
        .any(|arg| arg == "-v" || arg == "-V" || arg == "--version")
    {
        prnt_ln!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let mut repo = Repo::default();
    if repo.has_no_available_forks() {
        yellow_ln!("There are no forks available which aren't already a remote:");
        prnt_ln!("{}", repo.git_remote_verbose_output());
        return;
    }
    repo.show_available_forks();
    repo.choose_fork();
    if repo.choose_local_remote_alias() {
        repo.offer_to_set_alias();
    }
    repo.set_remote();
}

/// Prints the help message.
fn print_help() {
    prnt!(
        r#"
Add a remote fork to a local Git repository.  When run from a Git repo, it queries GitLab or GitHub
for the full list of forks and offers simple choices for adding one under a local alias.  The added
fork will be configured with a pull-url only; the push-url will be disabled.

Configuration
=============
'add-remote' will display all forks which aren't currently copied locally, then ask you to choose
one and to provide an alias for it.

It will offer a default selection (i.e. just hit <return> to select it) if it can.  The default will
be chosen as follows:

* if there's only one fork available, it will be selected, or else
* the main fork/source owner if not already added locally, or else
* the fork indicated by the Git config value of "#
    );
    dark_cyan!("add-remote.preferredFork");
    prnt!(
        r#" if set, and if that fork
  is not already added locally

You can set "#
    );
    dark_cyan!("add-remote.preferredFork");
    prnt_ln!(" (e.g. to 'maidsafe') by running:\n");
    yellow_ln!("    git config --global --add add-remote.preferredFork maidsafe");
    prnt!(
        r#"
Having chosen the fork to add, you will then be asked to provide an alias for it.  Again, a default
value will be presented, chosen as follows:

* if this is the main fork/source owner, uses the Git config value of "#
    );
    dark_cyan_ln!("add-remote.mainForkOwnerAlias");
    prnt!(
        r#"  if set, or else uses "upstream"
* uses the Git config value from the map of aliases under the subkey "#
    );
    dark_cyan!("add-remote.forkAlias");
    prnt!(
        r#" if set
* uses the fork-owner's name

You can set "#
    );
    dark_cyan!("add-remote.mainForkOwnerAlias");
    prnt_ln!(" (e.g. to 'owner') by running:\n");
    yellow_ln!("    git config --global --add add-remote.mainForkOwnerAlias owner");
    prnt!(
        r#"
Default aliases can be added to your .gitconfig file under the subkey
"#
    );
    dark_cyan!("add-remote.forkAlias.<owner's name>");
    prnt_ln!(" by running e.g:\n");
    yellow_ln!("    git config --global --add add-remote.forkAlias.anthonywilliams Anthony");
    yellow_ln!("    git config --global --add add-remote.forkAlias.hsutter Herb");
    prnt_ln!(
        r#"
To use `add-remote` with any GitLab repository or with a private GitHub one, you need to provide a
Personal Access Token via git config.

For GitLab, create a token (https://gitlab.com/profile/personal_access_tokens) ensuring it has "api"
scope, then add it to your .gitconfig:
"#
    );
    yellow_ln!("    git config --global --add add-remote.gitLabToken <GitLab Token's Value>");
    prnt_ln!(
        r#"
For GitHub, create a token (https://github.com/settings/tokens) ensuring it has full "repo" scope,
then add it to your .gitconfig:
"#
    );
    yellow_ln!("    git config --global --add add-remote.gitHubToken <GitHub Token's Value>");
    prnt_ln!(
        r#"
Having run these Git config commands, your .gitconfig should contain the following:
"#
    );
    dark_green_ln!(
        r#"[add-remote]
    preferredFork = maidsafe
    mainForkOwnerAlias = owner
    gitLabToken = <GitLab Token's Value>
    gitHubToken = <GitHub Token's Value>
[add-remote "forkAlias"]
    anthonywilliams = Anthony
    hsutter = Herb
"#
    );
}
