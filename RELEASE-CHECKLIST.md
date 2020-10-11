1) Document all new features in README.md
2) Update all dependencies to the latest with `cargo update`, and run `cargo test --release`
3) For each crate, in the following list, check for changes since the last release, and publish to crates.io.  Also include any crates with entries in ./Cargo.toml's patch section.
   1) tab-api
   2) tab-websocket
   3) tab-command
   4) tab-daemon
   5) tab-pty
   6) tab-pty-process
4) Remove any cargo patches in `./Cargo.toml`.
5) Update `tab/Cargo.toml` to use the released crates.
6) Update `tab/Cargo.toml` with the new release version.
7) Open a PR, and merge to main.
8) Check that the Github Actions workflow on main succeeds.
9) Create a Github Release on main, and monitor the progress.