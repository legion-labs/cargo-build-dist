# cargo-dockerize subcommand

The goal of this cargo sub command is to package a given crate binaries in a docker container, it takes into consideration the crate dependencies to define if the crate version should be bumped effectively, it is more useful in the context of a monorepo like the one Legion Labs maintains.

## How to:

Add target/debug in path and run `cargo dockerize build`

you should a similar output to this:

```
[DockerPacakge { name: "cargo-dockerize", version: "0.1.0", toml_path: "C:\\workspace\\github.com\\legion-labs\\cargo-docker\\Cargo.toml", binaries: ["cargo-dockerize"], docker_settings: DockerSettings { deps_hash: "aa" }, deps: [Dependency { name: "cargo_metadata", version: "0.14.0" }, Dependency { name: "cargo_toml", version: "0.9.2" }, Dependency { name: "clap", version: "2.33.3" }, Dependency { name: "serde", version: "1.0.126" }, Dependency { 
name: "serde_json", version: "1.0.64" }] }]
```
