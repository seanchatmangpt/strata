#!/usr/bin/env bash
# module_ratchet.sh — one `pub mod <name>;` per src file, both directions.
# Catches: duplicate registration (e.g. double `pub mod pnwire;`), missing
# declaration, and lib.rs entries pointing at nonexistent files.
# bash 3.2-safe; exit nonzero on any violation.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
violations=0

# RATCHET_ALLOW — per-crate space-separated module allowlists.
# Modules consumed via #[path] includes from tests/ (registered never,
# deliberately not in lib.rs) belong here, one entry per crate, keyed by
# the crate's src dir. Changing one requires a receipt naming the #[path]
# consumer files.
#   strata_protocol: signer — included via #[path = "../src/signer.rs"]
#     from tests/signed_commit_flow_test.rs and tests/signer_test.rs
#     (pre-existing pattern; lib.rs registration deliberately absent).
RATCHET_ALLOW__STRATA_PROTOCOL="signer"
RATCHET_ALLOW__TEMPRUN=""

allowlisted() {
    # $1 = crate key, $2 = module name
    local list_var="RATCHET_ALLOW__${1}"
    eval "list=\${${list_var:-}-}"
    case " ${list:-} " in *" $2 "*) return 0 ;; *) return 1 ;; esac
}

# crates: src_dir | lib_rs | crate_key | excluded names (space-separated)
check_crate() {
    local src_dir="$1" lib_rs="$2" crate_key="$3" excludes="$4"
    local name src f
    local decls

    echo "== ratchet: $lib_rs vs $src_dir =="

    # Direction 1+2: every src file (not excluded, not lib.rs, not dir)
    # must have exactly one `pub mod <name>;` in lib.rs.
    for src in "$src_dir"/*.rs; do
        f="$(basename "$src")"
        name="${f%.rs}"
        [ "$f" = "lib.rs" ] && continue
        case " $excludes " in *" $name "*) continue ;; esac
        if allowlisted "$crate_key" "$name"; then
            continue
        fi
        decls="$(grep -c "^[[:space:]]*pub mod ${name}[[:space:]]*;" "$lib_rs" || true)"
        if [ "$decls" -eq 0 ]; then
            echo "VIOLATION: $src_dir/$f exists but has NO 'pub mod $name;' in $lib_rs"
            violations=$((violations + 1))
        elif [ "$decls" -gt 1 ]; then
            echo "VIOLATION: $lib_rs declares 'pub mod $name;' $decls times (expected exactly 1)"
            violations=$((violations + 1))
        fi
    done

    # Direction 3: every `pub mod` in lib.rs references an existing src/<name>.rs
    # (also catches duplicates: nonzero count pairs with the missing-file check
    # to surface phantom modules; duplicates of real files caught above).
    local mod_names
    mod_names="$(grep -oE '^[[:space:]]*pub mod [a-zA-Z_][a-zA-Z0-9_]*[[:space:]]*;' "$lib_rs" \
        | awk '{print $3}' | tr -d ';' | sort | uniq -d)"
    if [ -n "$mod_names" ]; then
        local dup
        for dup in $mod_names; do
            echo "VIOLATION: $lib_rs declares 'pub mod $dup;' more than once (duplicate registration)"
            violations=$((violations + 1))
        done
    fi
    while IFS= read -r name; do
        [ -z "$name" ] && continue
        if [ ! -f "$src_dir/$name.rs" ]; then
            echo "VIOLATION: $lib_rs declares 'pub mod $name;' but $src_dir/$name.rs does not exist"
            violations=$((violations + 1))
        fi
    done <<EOF
$(grep -oE '^[[:space:]]*pub mod [a-zA-Z_][a-zA-Z0-9_]*[[:space:]]*;' "$lib_rs" | awk '{print $3}' | tr -d ';')
EOF

    return 0
}

check_crate "$ROOT/strata_protocol/src" "$ROOT/strata_protocol/src/lib.rs" STRATA_PROTOCOL ""
check_crate "$ROOT/temprun/src" "$ROOT/temprun/src/lib.rs" TEMPRUN "pn_kernel"

if [ "$violations" -eq 0 ]; then
    echo "RATCHET PASS: 0 violations"
    exit 0
fi
echo "RATCHET FAIL: $violations violation(s)"
exit 1
