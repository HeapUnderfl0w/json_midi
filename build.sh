#!/usr/bin/env bash

[ -d "out" ] && rm -Rvf out
mkdir out

echo "<<<<< Building Release [Strict] >>>>>"
cargo build --release
cp -v target/release/json_midi out/json_midi

echo "<<<<< Building Release [Relaxed] >>>>>"
cargo build --release --no-default-features
cp -v target/release/json_midi out/json_midi_relaxed_parsing
