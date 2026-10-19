#!/usr/bin/env bash
#
# Exits 0 when $ENDPOINT is a collector URL a build could actually POST to,
# and 1 when it is not.
#
# The release gate for the usage collector endpoint, and the *only* shell copy
# of it. `release.yml` used to carry this check inline twice -- once in the
# build job and once in the .rpm job -- beside a third copy in Rust,
# `usage::sender::is_usable`, with nothing but comments keeping the three in
# step. They drifted the way that sentence predicts: all three accepted
# `https://flume-usage.<your-subdomain>.workers.dev/v1/usage`, the placeholder
# from the docs, because each checked only for `https://` and one more byte.
# Every release from 1.0.x to 1.4.0 shipped with it, compiled reporting that
# was configured as far as the diagnostics report could tell, and POSTed to a
# hostname that cannot resolve.
#
# So the rule is now stated once here, once in Rust, and
# `the_release_gate_agrees_with_is_usable` in src-tauri/src/usage/sender.rs
# runs this script over the same table of URLs as the Rust tests and fails if
# the two answers ever differ. Change one, change the other, and that test
# will say so.
#
# The rule: `https://`, then a host of two or more non-empty labels of
# letters, digits and `-`, the last starting with a letter; then optionally
# `:` and a port from 1 to 65535; then optionally a path, query or fragment.
# The Rust doc comment on `is_usable` says why each part is there. Trailing
# whitespace is trimmed first: reqwest's URL parser normalises it away, so
# rejecting it here would fail a release over a secret that works.
#
# The first version of this checked only that every character *could*
# appear in a hostname, which let `https://:` and `https://host:notaport`
# through. The regex now checks the shape, and the port range is arithmetic
# because a regex cannot say "at most 65535" legibly.
#
# Reads the URL from $ENDPOINT rather than an argument, so the secret never
# appears in a process listing.

set -u
LC_ALL=C

url="${ENDPOINT-}"
trimmed="${url%"${url##*[![:space:]]}"}"
# Groups: 1 a non-final label and its dot, 2 the final label, 3 `:port`,
# 4 the port's digits, 5 the path.
re='^https://([A-Za-z0-9-]+\.)+([A-Za-z][A-Za-z0-9-]*)(:([0-9]{1,5}))?([/?#].*)?$'

[[ $trimmed =~ $re ]] || exit 1

port="${BASH_REMATCH[4]}"
# 10# so a leading zero is not read as octal.
if [[ -n $port ]] && ! (( 10#$port >= 1 && 10#$port <= 65535 )); then
  exit 1
fi
exit 0
