#!/usr/bin/env bash
# Load jump-label test scenarios into the terminal.
# Run this in the source pane, then trigger zellij-flash with s.

R=$'\033[0m'
B=$'\033[1m'
D=$'\033[2m'
CYN=$'\033[38;5;117m'
YLW=$'\033[38;5;221m'
GRN=$'\033[38;5;114m'
MGT=$'\033[38;5;183m'
GRY=$'\033[38;5;245m'
RED=$'\033[38;5;203m'
ORG=$'\033[38;5;215m'

ruler() { printf "${GRY}%s${R}\n" "──────────────────────────────────────────────────────────────────────────────"; }
header() { printf "\n${B}${CYN}%s${R}\n" "$*"; }
sub()    { printf "${GRY}%s${R}\n" "$*"; }
expect() { printf "  ${GRY}Expected: ${R}%s\n" "$*"; }
prompt() { printf "\n  ${MGT}${B}Type s →${R} ${B}%s${R}\n" "$*"; }
words()  {
    printf "  "
    for w in "$@"; do printf "${B}%-8s${R}" "$w"; done
    echo
}

printf "${B}Jump-label behaviour tests${R}\n"
printf "${CYN}══════════════════════════════════════════════════════════════════════════════${R}\n"

# ── Scenario A ───────────────────────────────────────────────────────────────

header "SCENARIO A — unique continuations"
sub "Each \"wo\" word has a different next char → labels are the continuation chars."
echo
words wor woa won wob wox
prompt "wo"
expect "'r'→wor  'a'→woa  'n'→won  'b'→wob  'x'→wox"

ruler

# ── Scenario B ───────────────────────────────────────────────────────────────

header "SCENARIO B — ambiguous continuation"
sub "word and worm share continuation 'r' → 'r' must be excluded as a label."
echo
words word worm woa won
prompt "wo"
expect "${RED}'r' NOT a label${R} (word+worm both continue with r)  |  ${GRN}'a'→woa  'n'→won${R}"

ruler

# ── Scenario C ───────────────────────────────────────────────────────────────

header "SCENARIO C — mixed bag"
sub "card/care/cart share 'r'; cat/cab/can each have a unique next char."
echo
words card care cart cat cab can
prompt "ca"
expect "${RED}'r' NOT a label${R} (card, care, cart)  |  ${GRN}'t'→cat  'b'→cab  'n'→can${R}"

ruler

# ── Scenario D ───────────────────────────────────────────────────────────────

header "SCENARIO D — partial match flood"
sub "Single-char search exceeds the label pool → all matches highlight, no labels."
echo
printf "  "
for w in alpha about any all also another already always away \
          array apply after at above among across again alone \
          along apart area ask age air aim act add art aid; do
    printf "${ORG}%-10s${R}" "$w"
done
echo; echo

prompt "a"
expect "${YLW}all 'a' chars light up (partial, too many)${R}  footer: \"keep typing\""
prompt "ab"
expect "fewer matches — labels appear for about/above"

ruler

# ── Scenario E ───────────────────────────────────────────────────────────────

header "SCENARIO E — end-of-line match (no continuation)"
sub "\"wo\" at EOL has no next char → gets a pool label, not a continuation label."
echo
words wor woa
printf "  ${B}wo${R}\n"    # bare "wo" at end of its line — no following char
echo
prompt "wo"
expect "${GRN}'r'→wor  'a'→woa${R}  |  ${CYN}\"wo\" at EOL gets a pool label (not r or a)${R}"

printf "\n${CYN}══════════════════════════════════════════════════════════════════════════════${R}\n\n"
