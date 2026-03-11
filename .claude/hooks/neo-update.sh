#!/bin/bash
# Automatically runs `neo update <file>` after Claude edits a file.
# Fires on PostToolUse for Write and Edit tools.

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
CWD=$(echo "$INPUT" | jq -r '.cwd // empty')

# Nothing to do if no file path or no neo.md in project root
[ -z "$FILE_PATH" ] && exit 0
[ -z "$CWD" ] && exit 0
[ ! -f "${CWD}/neo.md" ] && exit 0

# Resolve neo binary: prefer release build, then debug, then PATH
if [ -f "${CWD}/target/release/neo" ]; then
  NEO_BIN="${CWD}/target/release/neo"
elif [ -f "${CWD}/target/debug/neo" ]; then
  NEO_BIN="${CWD}/target/debug/neo"
elif command -v neo &>/dev/null; then
  NEO_BIN="neo"
else
  exit 0
fi

cd "$CWD" && "$NEO_BIN" update "$FILE_PATH" 2>/dev/null
exit 0
