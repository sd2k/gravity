#!/usr/bin/env bash

# This script generates the guest bindings.
# It requires a custom build of `wit-bindgen-go` from
# https://github.com/bytecodealliance/go-modules/pull/367.
# To install, clone the repo, check out that branch, and run
# `go install ./cmd/wit-bindgen-go`.

set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

wit-bindgen-go generate $SCRIPT_DIR/wit/basic.wit --out $SCRIPT_DIR/guest
