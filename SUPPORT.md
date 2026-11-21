# Getting help

This repository holds the on-chain Soroban contracts (`vault`, `stream`,
`registry`) and their deploy script. Start with the documentation, then pick
the right channel below.

## Read the docs first

- [README.md](README.md) — what the contracts do and how to run the tests.
- [docs/stream.md](docs/stream.md), [docs/vault.md](docs/vault.md) and
  [docs/registry.md](docs/registry.md) — function reference, errors and
  security notes for each contract.
- [docs/events.md](docs/events.md) — event topics and data fields for indexers.
- [docs/cli.md](docs/cli.md) — copy-paste `stellar contract invoke` examples.
- [CONTRIBUTING.md](CONTRIBUTING.md) — local setup, toolchain and test commands.

## Where to ask

- **Usage questions** (how to call a function, how to set up a local build):
  open a [new issue](https://github.com/Stellar-Hive/stellar-stream-hive-contract/issues/new/choose)
  and apply the `question` label. GitHub Discussions is not enabled for this
  repository.
- **Questions about the indexing API or the web frontend** belong in their
  sister repositories, linked from the [README](README.md#architecture).

## What belongs in an issue

- **Bug reports**: a contract, test or script does something other than the
  docs say. Use the bug report template and include a failing `cargo test`
  case if you have one.
- **Feature requests**: a change to the contracts or the streaming model. Use
  the feature request template.
- **Documentation problems**: wrong or missing text in the docs or README.
  Use the [documentation template](https://github.com/Stellar-Hive/stellar-stream-hive-contract/issues/new?template=documentation.md),
  which asks for the source of truth. Check the text against the contract
  source first.

Please keep issues free of private keys, seed phrases and wallet secrets.

## Security reports

Do **not** open a public issue for a suspected vulnerability, even if you are
unsure it is exploitable. Report it privately through
**Security → Report a vulnerability** on this repository. See
[SECURITY.md](SECURITY.md) for scope and expected response times.
