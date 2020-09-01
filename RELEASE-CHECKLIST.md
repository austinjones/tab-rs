1) For each crate, in the following list, check for changes since the last release, and publish to crates.io.  Also include any crates with entries in ./Cargo.toml's patch section.
   1) tab-api
   2) tab-websocket
   3) tab-command
   4) tab-daemon
   5) tab-pty
   6) tab-pty-process
2) Remove any cargo patches in `./Cargo.toml`.
3) Update `tab/Cargo.toml` to use the released crates.
4) Update `tab/Cargo.toml` with the new release version.
5) Open a PR, and merge to master.
6) Check that the master build succeeds.
7) Create a Github Release on master, and monitor the progress.