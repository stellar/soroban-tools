# Contributing to stellar-cli

Thanks for taking the time to improve stellar-cli!

The following is a set of guidelines for contributions and may change over time.
Feel free to suggest improvements to this document in a pull request.We want to make it as easy as possible to contribute changes that help the Stellar network grow and
thrive. There are a few guidelines that we ask contributors to follow so that we can merge your
changes quickly.

## Getting Started

* Make sure you have a [GitHub account](https://github.com/signup/free).
* Create a GitHub issue for your contribution, assuming one does not already exist.
  * Clearly describe the issue including steps to reproduce if it is a bug.
* Fork the repository on GitHub.

## Setting up development environment

There are 2 ways to begin developing stellar-cli:

### Installing all required dependencies 

You may want to install all required dependencies locally. This includes installing `rustup`, `make`, `libudev`, `jq` from your package manager. After all dependencies are installed, you can start with running `make install` to build `stellar-cli` and install it! 

### Using `nix`

If you don't want to install necessary dependencies from above, you can run development shell using [nix](https://nixos.org/guides/how-nix-works/) (make sure to [install](https://nixos.org/download/) version above 2.20). After installing `nix`, simply run `nix develop` that will start new `bash` session in your current terminal. If you want to use different shell (e.g. `zsh`) you can run `nix develop -c zsh`

This session will have:
1. All required dependencies installed
2. `stellar` alias (overwriting your existing `stellar` installed via cargo, if any)
3. Configured auto-complete for the working git branch

You can add extra configuration in your `local.sh` file (for example, if you want to export some extra variables for your devshell you can put following in your `local.sh`:
```shell
#!/usr/bin/env bash
export STELLAR_NETWORK=testnet
```
Note that all of dependencies and configurations mentioned above is available only in your local development shell, not outside of it.


### Minor Changes

#### Documentation

For small changes to comments and documentation, it is not
always necessary to create a new GitHub issue. In this case, it is
appropriate to start the first line of a commit with 'doc' instead of
an issue number.

## Finding things to work on

The first place to start is always looking over the current GitHub issues for the project you are
interested in contributing to. Issues marked with [help wanted][help-wanted] are usually pretty
self-contained and a good place to get started.

Stellar.org also uses these same GitHub issues to keep track of what we are working on. If you see
any issues that are assigned to a particular person or have the `in progress` label, that means
someone is currently working on that issue this issue in the next week or two.

Of course, feel free to create a new issue if you think something needs to be added or fixed.


## Making Changes

* Fork the stellar-cli repo to your own Github account

* List the current configured remote repository for your fork. Your git remote
should initially look like this. 
   ```
   $ git remote -v
   > origin  https://github.com/YOUR_USERNAME/stellar-cli.git (fetch)
   > origin  https://github.com/YOUR_USERNAME/stellar-cli.git (push)
   ```

* Set the `stellar/stellar-cli` repo as the remote upstream repository that will
sync with your fork. 
  ```
  git remote add upstream https://github.com/stellar/stellar-cli.git
  ```

* Verify the new upstream repository you've specified for your fork.
  ```
  $ git remote -v
  > origin    https://github.com/YOUR_USERNAME/stellar-cli.git (fetch)
  > origin    https://github.com/YOUR_USERNAME/stellar-cli.git (push)
  > upstream  https://github.com/stellar/stellar-cli.git (fetch)
  > upstream  https://github.com/stellar/stellar-cli.git (push)
  ```

* Add git hooks for commits and pushes so that checks run before pushing:
  ```
  ./install_githooks.sh
  ```

* Create a topic branch for your changes in your local repo. When you push you should be able
to create PR based on upstream stellar/stellar-cli.

* Make sure you have added the necessary tests for your changes and make sure all tests pass.


## Submitting Changes

* All content, comments, pull requests and other contributions must comply with the
  [Stellar Code of Conduct][coc].
* Push your changes to a topic branch in your fork of the repository.
* Submit a pull request to the repo in the Stellar organization.
  * Include a descriptive [commit message][commit-msg].
  * Changes contributed via pull request should focus on a single issue at a time.
  * Rebase your local changes against the master branch. Resolve any conflicts that arise.


At this point you're waiting on us. We like to at least comment on pull requests within three
business days (typically, one business day). We may suggest some changes, improvements or
alternatives.

# Additional Resources

* #dev-discussion channel on [Discord](https://discord.gg/BYPXtmwX)

This document is inspired by:

[help-wanted]: https://github.com/stellar/stellar-cli/contribute 
[commit-msg]: https://github.com/erlang/otp/wiki/Writing-good-commit-messages
[coc]: https://github.com/stellar/.github/blob/master/CODE_OF_CONDUCT.md
