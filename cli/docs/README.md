# CLI Design

## Execution modes
CLI will support 2 execution modes:
* Interactive. In this mode CLI will read commands from terminal interactively.
* Batch. In this code all commands will be read from file or pipe and executed in series.

## Commands
Command format
```
indy-cli-rs> [<group>] <command> [[<main_param_name>=]<main_param_value>] [<param_name1>=<param_value1>] ... [<param_nameN>=<param_valueN>]
```
### Common commands

#### Help
Print list of groups with group help:
```
indy-cli-rs> help
```
Print list of group commands with command help:
```
indy-cli-rs> <group> help
```

Print command help, list of command param and help for each param:
```
indy-cli-rs> <group> <command> help
```

#### About
Print about and license info:
```
indy-cli-rs> about
```

#### Exit
Exit from CLI:
```
indy-cli-rs> exit
```

#### Prompt
Change command prompt:
```
indy-cli-rs> prompt <new_prompt>
```

#### Show
Print content of file:
```
indy-cli-rs> show [<file_path>]
```

### Wallets management commands (wallet group)
```
indy-cli-rs> wallet <command>
```

#### Wallet create
Create new wallet and attach to CLI:
```
indy-cli-rs> wallet create <wallet name> key [key_derivation_method=<key_derivation_method>] [storage_type=<storage_type>] [storage_config={config json}]
```

#### Wallet attach
Attach existing wallet to Indy CLI:
```
indy-cli-rs> wallet attach <wallet name> [storage_type=<storage_type>] [storage_config={config json}]
```

#### Wallet open
Open the wallet with specified name and make it available for commands that require wallet. If there was opened wallet it will be closed:
```
indy-cli-rs> wallet open <wallet name> key [key_derivation_method=<key_derivation_method>] [rekey] [rekey_derivation_method=<rekey_derivation_method>]
```

#### Wallet close
Close the opened wallet
```
indy-cli-rs> wallet close
```

#### Wallet delete
Delete the wallet
```
indy-cli-rs> wallet delete <wallet name> key [key_derivation_method=<key_derivation_method>]
```

#### Wallet detach
Detach wallet from Indy CLI
```
indy-cli-rs> wallet detach <wallet name>
```

#### Wallet list
List all attached wallets with corresponded status (indicates opened one):
```
indy-cli-rs> wallet list
```

### Export wallet
Exports opened wallet to the specified file.

```indy-cli
indy-cli-rs> wallet export export_path=<path-to-file> export_key=[<export key>] [export_key_derivation_method=<export_key_derivation_method>]
```

### Import wallet
Create new wallet and then import content from the specified file.

```indy-cli
indy-cli-rs> wallet import <wallet name> key=<key> [key_derivation_method=<key_derivation_method>] export_path=<path-to-file> export_key=<key used for export>  [storage_type=<storage_type>] [storage_config={config json}]
```

### Pool management commands
```
indy-cli-rs> pool <subcommand>
```

#### Create config
Create name pool (network) configuration
```
indy-cli-rs> pool create [name=]<pool name> gen_txn_file=<gen txn file path> 
```

#### Connect
Connect to Indy nodes pool and make it available for operation that require pool access. If there was pool connection it will be disconnected.
```
indy-cli-rs> pool connect [name=]<pool name> [protocol-version=<version>] [timeout=<timeout>] [extended-timeout=<timeout>] [pre-ordered-nodes=<node names>]
```

#### Refresh
Refresh a local copy of a pool ledger and updates pool nodes connections.
```
indy-cli-rs> pool refresh
```

#### Set Protocol Version
Set protocol version that will be used for ledger requests. One of: 1, 2. Unless command is called the default protocol version 2 is used.
```
indy-cli-rs> pool set-protocol-version [protocol-version=]<version>
```

#### Disconnect
Disconnect from Indy nodes pool
```
indy-cli-rs> pool disconnect
```

#### List
List all created pools configurations with status (indicates connected one)
```
indy-cli-rs> pool list
```

### Identity Management
```
indy-cli-rs> did <subcommand>
```

#### New
Create and store my DID in the opened wallet. Requires opened wallet.
```
indy-cli-rs> did new [did=<did>] [seed=<UTF-8, base64 or hex string>] [metadata=<metadata string>] [<method>=<did method name>]
```

#### List
List my DIDs stored in the opened wallet as table (did, verkey, metadata). Requires wallet to be opened.:
```
indy-cli-rs> did list
```

#### Use
Use the DID as identity owner for commands that require identity owner:
```
indy-cli-rs> did use [did=]<did>
```

#### Rotate key
Rotate keys for used DID. Sends NYM to the ledger with updated keys. Requires opened wallet and connection to pool:
```
indy-cli-rs> did rotate-key [seed=<UTF-8, base64 or hex string>]
```

#### Qualify DID
Update DID stored in the wallet to make fully qualified, or to do other DID maintenance:
```
indy-cli-rs> did qualify did=<did> method=<method>
```

#### Set DID Metadata
Update metadata for DID stored in the wallet:
```
indy-cli-rs> did set-metadata did=<did> metadata=<metadata>
```

### Ledger transactions/messages
```
indy-cli-rs> ledger <subcommand>
```

#### NYM transaction
Send NYM transaction
```
ledger nym did=<did-value> [verkey=<verkey-value>] [role=<role-value>] [sign=<true or false>] [send=<true or false>] [endorser=<endorser did>]
```

#### GET_NYM transaction
Send GET_NYM transaction
```
ledger get-nym did=<did-value> [send=<true or false>]
```

#### ATTRIB transaction
Send ATTRIB transaction
```
ledger attrib did=<did-value> [hash=<hash-value>] [raw=<raw-value>] [enc=<enc-value>] [sign=<true or false>]  [send=<true or false>] [endorser=<endorser did>]
```

#### GET_ATTRIB transaction
Send GET_ATTRIB transaction
```
ledger get-attrib did=<did-value> [raw=<raw-value>] [hash=<hash-value>] [enc=<enc-value>] [send=<true or false>]
```

#### SCHEMA transaction
Send SCHEMA transaction
```
ledger schema name=<name-value> version=<version-value> attr_names=<attr_names-value> [sign=<true or false>]  [send=<true or false>] [endorser=<endorser did>]
```

#### GET_SCHEMA transaction
```
ledger get-schema did=<did-value> name=<name-value> version=<version-value> [send=<true or false>]
```

#### CRED_DEF transaction
Send CRED_DEF transaction
```
ledger cred-def schema_id=<schema_id-value> signature_type=<signature_type-value> [tag=<tag>] primary=<primary-value> [revocation=<revocation-value>] [sign=<true or false>]  [send=<true or false>] [endorser=<endorser did>]
```

#### GET_CRED_DEF transaction
Send GET_CRED_DEF transaction
```
ledger get-cred-def schema_id=<schema_id-value> signature_type=<signature_type-value> origin=<origin-value> [send=<true or false>]
```

#### NODE transaction
Send NODE transaction
```
ledger node target=<target-value> alias=<alias-value> [node_ip=<node_ip-value>] [node_port=<node_port-value>] [client_ip=<client_ip-value>] [client_port=<client_port-value>] [blskey=<blskey-value>] [blskey_pop=<blskey-proof-of-possession>] [services=<services-value>] [sign=<true or false>]  [send=<true or false>]
```

#### GET_VALIDATOR_INFO transaction
Send GET_VALIDATOR_INFO transaction to get info from all nodes
```
ledger get-validator-info [nodes=<node names>] [timeout=<timeout>]
```

#### POOL_UPGRADE transaction
Send POOL_UPGRADE transaction
```
ledger pool-upgrade name=<name> version=<version> action=<start or cancel> sha256=<sha256> [timeout=<timeout>] [schedule=<schedule>] [justification=<justification>] [reinstall=<true or false (default false)>] [force=<true or false (default false)>] [package=<package>] [sign=<true or false>]  [send=<true or false>]
```

#### POOL_CONFIG transaction
Send POOL_CONFIG transaction
```
ledger pool-config writes=<true or false (default false)> [force=<true or false (default false)>] [sign=<true or false>]  [send=<true or false>]
```

#### POOL_RESTART transaction
Send POOL_RESTART transaction
```
ledger pool-restart action=<start or cancel> [datetime=<datetime>] [nodes=<node names>] [timeout=<timeout>]
```

#### Custom transaction
Send custom transaction with user defined json body and optional signature
```
ledger custom [txn=]<txn-json-value> [sign=<true|false>]
```

#### AUTH_RULE transaction
Send AUTH_RULE transaction
```
ledger auth-rule txn_type=<txn type> action=<add or edit> field=<txn field> [old_value=<value>] [new_value=<new_value>] constraint=<{constraint json}> [sign=<true or false>]  [send=<true or false>]
```

#### GET_AUTH_RULE transaction
Send GET_AUTH_RULE transaction
```
ledger get-auth-rule [txn_type=<txn type>] [action=<ADD or EDIT>] [field=<txn field>] [old_value=<value>] [new_value=<new_value>] [send=<true or false>]
```

#### Add multi signature to transaction
Add multi signature by current DID to transaction
```
ledger sign-multi txn=<txn_json>
```

#### Save transaction to a file.
Save stored into CLI context transaction to a file.
```
ledger save-transaction file=<path to file>
```

#### Load transaction from a file.
Read transaction from a file and store it into CLI context.
```
ledger load-transaction file=<path to file>
```

#### TXN_AUTHR_AGRMT transaction.
Request to add a new version of Transaction Author Agreement to the ledger.
```
ledger txn-author-agreement [text=<agreement content>] [file=<file with agreement>] version=<version> [ratification-timestamp=<timestamp>] [retirement-timestamp=<timestamp>]  [sign=<true or false>]  [send=<true or false>]
```

#### DISABLE_ALL_TXN_AUTHR_AGRMTS transaction.
Disable All Transaction Author Agreements on the ledger.
```
ledger disable-all-txn-author-agreements  [sign=<true or false>]  [send=<true or false>]
```

#### SET_TXN_AUTHR_AGRMT_AML transaction.
Request to add new acceptance mechanisms for transaction author agreement.
```
ledger txn-acceptance-mechanisms [aml=<acceptance mechanisms>] [file=<file with acceptance mechanisms>] version=<version> [context=<some context>]  [sign=<true or false>]  [send=<true or false>]
```

#### GET_TXN_AUTHR_AGRMT_AML transaction.
Get a list of acceptance mechanisms set on the ledger.
```
ledger get-acceptance-mechanisms [timestamp=<timestamp>] [version=<version>] [send=<true or false>]
```

## Examples

#### Create pool configuration and connect to pool
```
indy-cli-rs> pool create sandbox gen_txn_file=/etc/sovrin/sandbox.txn
indy-cli-rs> pool connect sandbox
pool(sandbox):indy-cli-rs> pool list
```

#### Create and open wallet (Sqlite storage type used by default)
```
sandbox|indy-cli-rs> wallet create alice_wallet key
sandbox|indy-cli-rs> wallet open alice_wallet key
pool(sandbox):wallet(alice_wallet):indy-cli-rs> wallet list
```

#### Create and open Postgres wallet
```
sandbox|indy-cli-rs> wallet create wallet_pstg key storage_type=postgres_storage storage_config={"url":"localhost:5432"} storage_credentials={"account":"postgres","password":"mysecretpassword","admin_account":"postgres","admin_password":"mysecretpassword"}
sandbox|indy-cli-rs> wallet open wallet_pstg key storage_credentials={"account":"postgres","password":"mysecretpassword","admin_account":"postgres","admin_password":"mysecretpassword"}
```

#### Create random DID and use it for the next commands
```
pool(sandbox):wallet(alice_wallet):indy-cli-rs> did new
pool(sandbox):wallet(alice_wallet):indy-cli-rs> did use PDwYxJ7hSXdM2PCz3yX16z
pool(sandbox):wallet(alice_wallet):did(PDw...16z):indy-cli-rs> did list
```

#### Create DID in the wallet from seed (and custom metadata) and use it for the next commands
```
pool(sandbox):wallet(alice_wallet):indy-cli-rs> did new seed=SEED0000000000000000000000000001 metadata="Alice DID"
pool(sandbox):wallet(alice_wallet):indy-cli-rs> did use Av63wJYM7xYR4AiygYq4c3
pool(sandbox):wallet(alice_wallet):did(Av6...4c3):indy-cli-rs> did list
```

#### Post new NYM to the ledger
```
pool(sandbox):wallet(alice_wallet):did(Av6...4c3):indy-cli-rs> ledger nym did=PDwYxJ7hSXdM2PCz3yX16z verkey=~JpZc1EarKfUwSpUTJ3ii3N
```

#### Send GET_NYM transaction
```
pool(sandbox):wallet(alice_wallet):did(Av6...4c3):indy-cli-rs> ledger get-nym did=MYDID000000000000000000001
```

#### Post new Schema to the Ledger
```
pool(sandbox):wallet(alice_wallet):did(Av6...4c3):indy-cli-rs> ledger schema name=Alice_Schema version=1.0 attr_names=firstname,surname
```

#### Prepare transaction for posting new Schema to the Ledger
```
pool(sandbox):wallet(alice_wallet):did(Av6...4c3):indy-cli-rs> ledger schema name=Alice_Schema version=1.0 attr_names=firstname,surname endorser=EndorserDID000000000000001 send=false
```

#### Load transaction from a file and sign it 
```
pool(sandbox):wallet(alice_wallet):did(Av6...4c3):indy-cli-rs> ledger load-transaction file=txn.json
pool(sandbox):wallet(alice_wallet):did(Av6...4c3):indy-cli-rs> ledger sign-multi
pool(sandbox):wallet(alice_wallet):did(Av6...4c3):indy-cli-rs> ledger save-transaction file=txn.json
```

#### Load transaction from a file and post to the Ledger 
```
pool(sandbox):wallet(alice_wallet):did(Av6...4c3):indy-cli-rs> ledger load-transaction file=txn.json
pool(sandbox):wallet(alice_wallet):did(Av6...4c3):indy-cli-rs> ledger custom context
```

#### Change wallet key
```
sandbox|indy-cli-rs> wallet open alice_wallet key rekey=endorser rekey_derivation_method=argon2i
```

#### Run CLI with batch script
```
indy-cli-rs /path/to/script-file
```

#### Run CLI with config
```
indy-cli-rs --config /path/to/config.json
```

#### Run CLI with logger
```
indy-cli-rs --logger-config /path/to/logger.yml
```
