# S04 Security Posture — Stored Credentials

Starting with S04, rtf stores bank-sync provider credentials directly in
the SQLite database (`provider_credentials` table, migration 004). Concretely:

- **SimpleFIN** — the access URL (`https://<user>:<pass>@<host>/<path>`) is
  stored as plaintext JSON. The URL contains embedded basic-auth credentials
  that grant read-only access to the user's linked US bank data.
- **Pluggy** — `client_id`, `client_secret`, and `item_id` are stored as
  plaintext JSON. Together they grant read-only access to the linked Brazilian
  bank data for the specific `item_id`.

## Threat model

This is a **single-user CLI tool** running on the user's own machine. The
database file (`rtf.db`) sits alongside the user's other local data and
inherits the filesystem's access controls. We treat it with the same care as:

- `~/.ssh/id_rsa`
- `~/.aws/credentials`
- `~/.config/gh/hosts.yml`

If an attacker has read access to the user's home directory, they can already
read every other credential on the machine — credential-stealing malware is
out of scope for this layer.

## What we do

- **File-system permissions** — the SQLite file is created by rusqlite with
  default user-only perms; we don't broaden them. Users should not commit
  `rtf.db` to version control. (The default `--db` path points at the
  user's working directory; we do not ship a `.gitignore` entry, so users
  running `rtf` inside a git repo should verify their `.gitignore`.)
- **No network exposure** — the DB file never leaves the machine unless the
  user copies it. Nothing in rtf uploads, backs up, or shares it.
- **Rotation by upsert** — re-running `rtf simplefin setup <token>` or
  `rtf pluggy setup ...` replaces the stored credentials atomically.
  Rotation is a one-liner.

## What we do NOT do (yet)

- **Keychain integration.** macOS Keychain, Linux secret-service, or Windows
  Credential Manager would add OS-level protection. Deferred; out of scope
  for M001.
- **Application-level encryption.** Would require a user-supplied password
  on every CLI invocation. Ergonomics-hostile for a frequently-run sync tool.
  Revisit if we ever ship a daemon mode.
- **Credential-at-rest encryption with a derived key.** Same tradeoff.
- **Audit log of credential access.** Each sync call implicitly reads
  credentials; logging that adds noise without meaningful signal in a
  single-user model.

## Recommendations for the user

1. Keep `rtf.db` on an encrypted-at-rest disk (default on modern
   macOS/Linux laptops).
2. Do not commit `rtf.db` to any repository.
3. Treat the `rtf simplefin setup` token and the Pluggy client secret
   as you would a password: paste once, don't echo into shell history if
   you can help it (`set +o history`, paste, re-enable).

## Revisit triggers

Reconsider this posture if any of:

- rtf grows a multi-user mode.
- A daemon/server deployment is introduced.
- We add cloud backup or sync of the local DB.
- Keychain integration becomes a cheap addition (<2h).
