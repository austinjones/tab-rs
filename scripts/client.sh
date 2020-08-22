#!/usr/bin/env bash
killall tab-daemon
cargo run --bin tab-cli -- --dev "$@"
