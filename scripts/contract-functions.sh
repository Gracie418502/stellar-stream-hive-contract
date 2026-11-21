#!/usr/bin/env bash
#
# Prints the README "Contracts at a glance" table, generated from the public
# functions in each contract's src/lib.rs.
#
# Grouping rules (applied to each `pub fn` in the contract's impl block):
#   Admin  - `initialize`, or a body that reads the stored admin or requires
#            the admin's authorization (pause, unpause, transfer_admin,
#            set_stream_contract).
#   Read   - a getter or query: a name starting with get_, is_ or count_,
#            or one of the named accessors (streamed_amount,
#            withdrawable_amount, balance_of_stream, total_locked,
#            stream_count).
#   Write  - everything else that changes state.
#
# Usage:
#   scripts/contract-functions.sh          print the table to stdout
#
# Regenerate the README table after changing a contract's public functions:
#   scripts/contract-functions.sh > table.md   (then paste into README.md)

set -euo pipefail

cd "$(dirname "$0")/.."

printf '| Contract | Write | Read | Admin |\n'
printf '|---|---|---|---|\n'

for lib in contracts/vault/src/lib.rs contracts/stream/src/lib.rs contracts/registry/src/lib.rs; do
  contract="$(basename "$(dirname "$(dirname "$lib")")")"
  awk -v contract="$contract" '
    function flush(   list) {
      if (fn == "") return
      if (fn ~ /^(get_|is_|count_)/ || fn ~ /^(stream_count|streamed_amount|withdrawable_amount|balance_of_stream|total_locked)$/) {
        kind = "read"
      } else if (fn == "initialize" || body ~ /storage::get_admin\(|admin\.require_auth\(/) {
        kind = "admin"
      } else {
        kind = "write"
      }
      names[kind] = names[kind] (names[kind] == "" ? "" : ", ") "`" fn "`"
      fn = ""
    }
    /^    pub fn / {
      flush()
      line = $0
      sub(/^    pub fn /, "", line)
      sub(/[(<].*/, "", line)
      fn = line
      body = ""
    }
    fn != "" { body = body $0 "\n" }
    /^    }$/ { flush() }
    END {
      printf "| `%s` | %s | %s | %s |\n", contract, names["write"], names["read"], names["admin"]
    }
  ' "$lib"
done
