#!/usr/bin/env bash
# doc_cite_check.sh — static anti-regression checker for doc citations.
#
# Scans the canonical status docs for three drift classes:
#   (R1) `path:line` citations where the path does not exist on disk
#   (R2) N/M pass-count figures with NO date anywhere on the same line
#   (R3) test-count claims with no producing command on the same line
#
# Fail-closed: any violation -> exit 1 with file:line listing.
#
# Usage:
#   scripts/doc_cite_check.sh [ROOT]     # default ROOT = repo root of script
#   scripts/doc_cite_check.sh --self-test
#
# Suppression: append `<!-- doc-cite-ignore -->` on a line to exempt that
# line from all rules (use only for wire constants / non-claim figures).

set -u

SELF_TEST=0
ROOT=""
for arg in "$@"; do
  case "$arg" in
    --self-test) SELF_TEST=1 ;;
    *) ROOT="$arg" ;;
  esac
done

if [ "$SELF_TEST" = "1" ]; then
  set -e
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  REPO="$(cd "$(dirname "$0")/.." && pwd)"
  mkdir -p "$TMP/docs"
  for f in README.md KNOWN_GAPS.md CHANGELOG.md docs/cas.md docs/CI.md; do
    [ -f "$REPO/$f" ] && cp "$REPO/$f" "$TMP/$f"
  done
  # Inject a bad path:line citation -> R1 must fire.
  printf '\nSee `docs/definitely_missing_file_xyz.md:42` for details.\n' >> "$TMP/docs/cas.md"
  # Inject an undated pass-count under a fresh undated header -> R2 must fire.
  printf '\n## Undated Section\n\nThe suite reports 12/12 with no further context.\n' >> "$TMP/docs/CI.md"
  # Inject an undated test-count under the same header -> R3 must fire.
  printf '\nTotal of 340 tests pass in the valve tier.\n' >> "$TMP/docs/CI.md"
  OUT="$("$0" "$TMP" 2>&1)" || true
  rc=0
  echo "$OUT" | grep -q "definitely_missing_file_xyz.md:42" || { echo "SELF-TEST FAIL: R1 did not fire"; rc=1; }
  echo "$OUT" | grep -q "12/12" || { echo "SELF-TEST FAIL: R2 did not fire"; rc=1; }
  echo "$OUT" | grep -q "340 tests" || { echo "SELF-TEST FAIL: R3 did not fire"; rc=1; }
  if [ "$rc" = "0" ]; then echo "SELF-TEST PASS: all three rules fire on injected violations"; fi
  exit "$rc"
fi

ROOT="${ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$ROOT" || exit 1

FILES=(README.md KNOWN_GAPS.md CHANGELOG.md docs/cas.md docs/CI.md)

# Single perl pass per file, emitting violations directly.
VIOL="$(for f in "${FILES[@]}"; do
  [ -f "$f" ] || continue
  CURFILE="$f" perl -CSD -e '
    my $file = $ENV{CURFILE};
    open(my $fh, "<", $file) or die "open $file: $!";
    my @lines = <$fh>;
    my $n = 0;
    # date_seen[j] = 1 if a date appears on line j+1 (0-based j)
    my @dated = map { /20[0-9]{2}-[0-9]{2}-[0-9]{2}|Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec/ ? 1 : 0 } @lines;
    # Section context start per line: nearest header if it (or the body up to
    # this line) carries a date; otherwise the nearest PARENT header (level
    # strictly less than the nearest header level) — an undated sub-header
    # like `### Added` inherits the dated release block above it.
    my @start = ();
    {
      my $h = 0; my $hl = 0; my $p = -1;
      for my $i (0..$#lines) {
        if ($lines[$i] =~ /^\s{0,3}(#+)\s/) {
          my $L = length($1);
          $p = -1;
          for (my $k = $#hdrs; $k >= 0; $k--) {
            if ($lvl[$k] < $L) { $p = $hdrs[$k]; last; }
          }
          push @hdrs, $i; push @lvl, $L;
          $h = $i; $hl = $L;
        }
        # Dated context: own section body [h..i], else ONLY the parent
        # header line itself (a file preamble or sibling-section date must
        # not blanket-cover this section). No parent -> nearest header.
        $start[$i] = ($dated[$h] || (grep { $dated[$_] } $h .. $i)) ? $h
                   : (($p >= 0 && $dated[$p]) ? $p : $h);
      }
    }
    foreach my $l (@lines) {
      $n++;
      next if $l =~ /doc-cite-ignore/;
      # R1: `dir/file.ext:NN` citation whose path does not exist.
      # Requires a `/` so bare basenames (`cas.rs:279`) are not flagged —
      # basenames are ambiguous when several same-named files exist.
      while ($l =~ /([A-Za-z0-9_][A-Za-z0-9_\.\/\-]*\/[A-Za-z0-9_\.\/\-]*\.(?:md|rs|ex|exs|ttl|toml|sh|txt|json|yaml|yml)):(\d+)\b/g) {
        my ($p, $ln) = ($1, $2);
        print "$file:$n: [R1] cited path does not exist: $p:$ln\n" unless -e $p;
      }
      # R2: N/M pass-count with no date in the enclosing dated section.
      my $dated_ctx = 0;
      for my $j ($start[$n-1] .. $n-1) { $dated_ctx = 1 if $dated[$j]; }
      if (!$dated_ctx && $l =~ /(?<![.\/\w])(\d+\/\d+)(?![.\/\w])/) {
        my $m = $1;
        # 1/256 is the documented fail-closed ambiguity bound, not a count
        next if $m eq "1/256";
        print "$file:$n: [R2] N/M pass-count with no date in context: $m\n";
      }
      # R3: test-count claim with no producing command on the line or the
      # line immediately above.
      if ($l !~ /(cargo test|mix test|just\s|ctest|pytest|go test|_court)/) {
        my $prev = $n >= 2 ? $lines[$n-2] : "";
        if ($prev !~ /(cargo test|mix test|just\s|ctest|pytest|go test|_court)/) {
          if ($l =~ /(\b\d+\s*(?:passed|failing|tests?\s+(?:pass|fail))|\b\d+\s*#\[\s*test\])/i) {
            print "$file:$n: [R3] test-count claim with no producing command nearby: $1\n";
          }
        }
      }
    }
  '
done)"

if [ -n "$VIOL" ]; then
  echo "doc_cite_check: FAIL — doc citation drift detected ($ROOT)"
  echo "$VIOL"
  N=$(printf '%s\n' "$VIOL" | wc -l | tr -d ' ')
  echo "violations: $N"
  exit 1
fi
echo "doc_cite_check: PASS (${FILES[*]})"
exit 0
