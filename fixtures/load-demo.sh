#!/usr/bin/env bash
# Populate the terminal with coloured demo content for a zellij-flash screencast.
# Run this in the source pane, then trigger zellij-flash.

R=$'\033[0m'         # reset
B=$'\033[1m'         # bold
D=$'\033[2m'         # dim
U=$'\033[4m'         # underline
GRN=$'\033[38;5;114m'   # green
YLW=$'\033[38;5;221m'   # yellow
CYN=$'\033[38;5;117m'   # cyan
MGT=$'\033[38;5;183m'   # magenta / prompt colour
ORG=$'\033[38;5;215m'   # orange  (tokens / secrets)
GRY=$'\033[38;5;245m'   # mid-grey
RED=$'\033[38;5;203m'   # red (error tones)
BLU=$'\033[38;5;75m'    # blue

prompt() { printf "\n${MGT}${B}❯${R} ${B}%s${R}\n\n" "$*"; }
url()    { printf "${CYN}${U}%s${R}" "$*"; }
ok()     { printf "${GRN}${B}✓${R}  %s\n" "$*"; }
hash()   { printf "${YLW}%s${R}" "$*"; }
token()  { printf "${ORG}%s${R}" "$*"; }
hdr()    { printf "${B}%s${R}\n" "$*"; }
dim()    { printf "${GRY}%s${R}" "$*"; }
bold()   { printf "${B}%s${R}" "$*"; }

# ── README ────────────────────────────────────────────────────────────────────

prompt "cat README.md"

hdr "# Synthwave CLI — Retro-flavoured cloud tooling"
echo
echo "A lightweight CLI for provisioning Synthwave Cloud environments."
echo
printf "Full documentation: "; url "https://docs.synthwave.io/cli/getting-started"; echo
printf "Source:             "; url "https://github.com/synthwave-io/swcli"; echo
printf "Changelog:          "; url "https://github.com/synthwave-io/swcli/blob/main/CHANGELOG.md"; echo
printf "Issues:             "; url "https://github.com/synthwave-io/swcli/issues"; echo
echo
hdr "## Quick install"
echo
printf "    ${CYN}curl${R} -fsSL "; url "https://releases.synthwave.io/install.sh"; printf " ${GRY}|${R} bash\n"
echo
printf "Or via Homebrew:\n\n"
printf "    ${CYN}brew${R} install synthwave-io/tap/swcli\n"
echo
printf "Verify the binary checksum after download:\n\n"
printf "    ${GRY}SHA256${R}  "; hash "a3f8c2e1d09b74f65a21cc8e4d0b93f7282a1cd6e5f84017b3a9e2f6c0d81475"; echo
printf "    ${GRY}FILE  ${R}  swcli-v2.4.1-darwin-arm64.tar.gz\n"
echo
hdr "## First-time setup"
echo
cat <<STEPS
1. Authenticate with your workspace:

       swcli auth login --workspace acme-corp

   This opens $(url "https://auth.synthwave.io/device") and prints a one-time code.
   Paste it in the browser to complete the flow.

2. Export your API token (generated at $(url "https://app.synthwave.io/settings/tokens")):

       export SYNTHWAVE_TOKEN=$(token "swt_prod_4Xk9mNpL2qRvTyUwJhBzCdEfGiAoSn8x")

3. Initialise your first environment:

       swcli env init --name staging --region eu-west-1 \\
           --template $(url "https://github.com/synthwave-io/templates/blob/main/node-api.yaml")

4. Deploy:

       swcli deploy --env staging --ref main

STEPS

hdr "## Configuration reference"
echo
printf "Default config path: ${GRY}~/.config/swcli/config.yaml${R}\n"
printf "Schema docs:         "; url "https://docs.synthwave.io/cli/config-schema"; echo
echo

# ── Auth flow ────────────────────────────────────────────────────────────────

prompt "swcli auth login --workspace acme-corp"

printf "Opening "; url "https://auth.synthwave.io/device"; printf " ...\n"
printf "Your one-time code: ${B}${YLW}XKPQ-7742${R}\n\n"
printf "${GRY}Waiting for browser confirmation...${R}\n"
ok "Authenticated as $(bold "stefan@acme-corp.io") (workspace: $(bold "acme-corp"))"
printf "${GRY}Token stored in ~/.config/swcli/credentials${R}\n"

# ── Env init ─────────────────────────────────────────────────────────────────

prompt "swcli env init --name staging --region eu-west-1"

printf "Fetching template list from "; url "https://api.synthwave.io/v1/templates"; printf " ...\n"
ok "Environment $(bold '"staging"') created"
printf "  ${GRY}ID:      ${R} "; token "env_01HX9MNTK4BVWZ3GRYQF5PD72J"; echo
printf "  ${GRY}Region:  ${R} eu-west-1\n"
printf "  ${GRY}Endpoint:${R} "; url "https://staging.acme-corp.synthwave.io"; echo

# ── Deploy ───────────────────────────────────────────────────────────────────

prompt "swcli deploy --env staging --ref main"

printf "Resolving ref ${B}main${R} → "; hash "a3f8c2e1d09b74f65a21cc8e4d0b93f7282a1cd6e5f84017b3a9e2f6c0d81475"; echo
printf "Uploading artefacts to "; url "https://artefacts.synthwave.io/acme-corp/staging/"; printf " ...\n"
printf "Build log: "; url "https://app.synthwave.io/builds/bld_7rK2mPxNqLvT4yUw"; echo
echo
printf "  ${GRY}[1/4]${R} Installing dependencies    "; ok "${GRY}12.3s${R}"
printf "  ${GRY}[2/4]${R} Running tests               "; ok "${GRY} 8.7s${R}"
printf "  ${GRY}[3/4]${R} Building production bundle  "; ok "${GRY}23.1s${R}"
printf "  ${GRY}[4/4]${R} Deploying to edge nodes     "; ok "${GRY} 4.2s${R}"
echo
ok "Deploy complete — $(url "https://staging.acme-corp.synthwave.io")"
printf "  ${GRY}Deployment ID:${R} "; token "dep_9wX3nBzC1eAoTfGy6hKvMrQp"; echo

# ── Env list ─────────────────────────────────────────────────────────────────

prompt "swcli env list"

printf "${B}${GRY}%-12s %-12s %-10s %s${R}\n" "NAME" "REGION" "STATUS" "ENDPOINT"
printf "%-12s %-12s ${GRN}%-10s${R} " "staging" "eu-west-1" "healthy"
url "https://staging.acme-corp.synthwave.io"; echo
printf "%-12s %-12s ${GRN}%-10s${R} " "prod" "us-east-1" "healthy"
url "https://acme-corp.synthwave.io"; echo
printf "%-12s %-12s ${YLW}%-10s${R} " "preview" "eu-west-1" "sleeping"
url "https://preview.acme-corp.synthwave.io"; echo

# ── Config file ──────────────────────────────────────────────────────────────

prompt "cat ~/.config/swcli/config.yaml"

printf "${GRY}workspace:${R}   acme-corp\n"
printf "${GRY}default_env:${R} staging\n"
printf "${GRY}api_base:${R}    "; url "https://api.synthwave.io/v1"; echo
printf "${GRY}log_level:${R}   info\n"

printf "\n${MGT}${B}❯${R} "
