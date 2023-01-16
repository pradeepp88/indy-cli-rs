## CLI for Indy Ledger

This is command line interface for Indy, which provides a distributed-ledger-based
foundation for self-sovereign identity. 

It provides the commands to:
* Manage wallets
* Manage pool configurations
* Manage DIDs
* Sending transactions to distributed ledger

### Execution modes
CLI supports 2 execution modes:
* Interactive. In this mode CLI reads commands from terminal interactively. To start this mode just run `indy-cli-rs`
without params.
* Batch. In this mode all commands will be read from text file or pipe and executed in series. To start this mode run
`indy-cli-rs <path-to-text-file>`. Batch mode supports the same commands as interactive mode. Note that by default if some
command finishes with an error batch execution will be interrupted. To prevent this start command with `-`.
For example, `-wallet create test`. In this case the result of this command will be ignored. Comments can also be made
by beginning the line with a `#`.

### Getting help
The most simple way is just start cli by `indy-cli-rs` command and put `help` command. Also, you can look to
[CLI design](./docs) doc that contains the list of commands and architecture overview.

### Options
* -h and --help - Print usage.
* --logger-config - Init logger according to a config file (default no logger initialized).
* --plugins - Load plugins in Libindy (usage: <lib-1-name>:<init-func-1-name>,...,<lib-n-name>:<init-func-n-name>).
* --config - Define config file for CLI initialization. A config file can contain the following fields:
    * loggerConfig - path to a logger config file (is equal to usage of "--logger-config" option).
    * taaAcceptanceMechanism - transaction author agreement acceptance mechanism to be used when sending write transactions to the Ledger.


### Notes
Indy-CLI--rs depends on `term` rust library that has a system dependency on terminfo database. 
That is why CLI Debian package additionally installs `libncursesw5-dev` library.
More about it read [here](https://crates.io/crates/term) at `Packaging and Distributing` section.



