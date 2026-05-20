#!/usr/bin/env bash
# Populate the terminal with demo content for a zellij-flash screencast.
# Run this in the source pane, then trigger zellij-flash to demo selection.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cat "$SCRIPT_DIR/demo-session.txt"
