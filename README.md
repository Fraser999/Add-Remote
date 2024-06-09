# add-remote

An interactive CLI tool to add a remote fork to a local Git repository.  When run from a Git repo,
it queries GitLab or GitHub as appropriate for the full list of forks and offers simple choices for
adding one under a local alias.  The added fork will be configured with a pull-url only; the
push-url will be disabled.

![Demonstration of add-remote](demo.svg "Demonstration of add-remote")

## Install

```
cargo install add-remote
```

## Run

Simply `cd` to a Git repository and run `add-remote`.

## Configure

`add-remote` will display all forks which aren't currently copied locally, then ask you to choose
one and to provide an alias for it.

### Preferred Fork

It will offer a default selection (i.e. just hit <kbd>return</kbd> to select it) if it can.  The
default will be chosen as follows:

* if there's only one fork available, it will be selected, or else
* the main fork/source owner if not already added locally, or else
* the fork indicated by the Git config value of `add-remote.preferredFork` if set, and if that fork
is not already added locally

You can set `add-remote.preferredFork` (e.g. to `CasperLabs`) by running:

```
git config --global --add add-remote.preferredFork CasperLabs
```

### Fork Aliases

Having chosen the fork to add, you will then be asked to provide an alias for it.  Again, a default
value will be presented, chosen as follows:

* if this is the main fork/source owner, uses the Git config value of
`add-remote.mainForkOwnerAlias` if set, or else uses `"upstream"`
* uses the Git config value from the map of aliases under the subkey `add-remote.forkAlias` if set
* uses the fork-owner's name

You can set `add-remote.mainForkOwnerAlias` (e.g. to `owner`) by running:

```
git config --global --add add-remote.mainForkOwnerAlias owner
```

Default aliases can be added to your .gitconfig file under the subkey
`add-remote.forkAlias.<owner's name>` by running e.g:

```
git config --global --add add-remote.forkAlias.anthonywilliams Anthony
git config --global --add add-remote.forkAlias.hsutter Herb
```

### Personal Access Tokens

To use `add-remote` with any GitLab repository or with a private GitHub one, you need to provide a
Personal Access Token via git config.

For GitLab, [create a token](https://gitlab.com/profile/personal_access_tokens) ensuring it has
"read_api" scope, then add it to your .gitconfig:

```
git config --global --add add-remote.gitLabToken <GitLab Token's Value>
```

For GitHub, [create a token](https://github.com/settings/tokens) ensuring it has full "repo" scope,
then add it **along with your GitHub username** separated with a colon `:` to your .gitconfig:

```
git config --global --add add-remote.gitHubToken <GitHub Username:GitHub Token's Value>
```

Having run these Git config commands, your .gitconfig should contain the following:

```
[add-remote]
    preferredFork = CasperLabs
    mainForkOwnerAlias = owner
    gitLabToken = <GitLab Token's Value>
    gitHubToken = <GitHub Username:GitHub Token's Value>
[add-remote "forkAlias"]
    anthonywilliams = Anthony
    hsutter = Herb
```

## Note

`add-remote` only works with projects hosted on GitLab or GitHub.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://opensource.org/licenses/Apache-2.0)
* MIT License ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
