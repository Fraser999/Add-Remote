//! An interactive CLI tool to add a remote fork to a local Git repository.

#![forbid(warnings)]
#![warn(missing_copy_implementations, trivial_casts, trivial_numeric_casts, unsafe_code,
        unused_extern_crates, unused_import_braces, unused_qualifications, unused_results,
        variant_size_differences)]
#![cfg_attr(feature="cargo-clippy", deny(clippy, clippy_pedantic))]

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
use std::process;

// TODO
// - handle pagination properly in `Repo::populate_available_forks()` (only handles 100 forks now)
// - use config file for default "upstream" value and for "known users"

/// Main function.
fn main() {
    unwrap!(ctrlc::set_handler(move || process::exit(0)), "Error setting Ctrl-C handler");

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
