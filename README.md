# Overview

This repo is a minimal setup for building a plugin that can be used in Hypi's RAPID server pipelines.

You need to make some changes to adapt it:
1. Replace usage of `hypi/my-plugin` in `deploy-plugin.yml` with your own Docker hub user and plugin name
2. Change the name in `Cargo.toml` from `my-plugin` to your plugin's name
3. Change the `MyPlugin` struct name in `src/main.rs` to your own name

The rest is the standard rust workflow with cargo commands.

# Testing

As the only path into the plugin is via the gRPC interface, you can use the gRPC client to test your plugin with the standard rust rest kits i.e. #test #tokio::test and so on.
Use the types from `rapid_utils::plugin::plugin_client` to create a client instance (for integration testing).
Unit tests are again just standard rust tests, no Hypi dependency involved.
