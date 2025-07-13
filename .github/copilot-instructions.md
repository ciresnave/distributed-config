# Copilot Instructions for Distributed Config

<!-- Use this file to provide workspace-specific custom instructions to Copilot. For more details, visit https://code.visualstudio.com/docs/copilot/copilot-customization#_use-a-githubcopilotinstructionsmd-file -->

## Project Overview

This is a Rust library crate for distributed configuration management. The library provides:

- Hierarchical configuration structure
- Multiple configuration sources (files, environment, remote)
- Dynamic configuration updates with change notifications
- Schema validation with strong typing
- Distributed configuration synchronization
- Feature flags management
- Configuration versioning and history

## Code Style Guidelines

1. **Async/Await**: Use tokio async runtime throughout
2. **Error Handling**: Use `thiserror` for custom errors and `anyhow` for error context
3. **Serialization**: Use `serde` with appropriate derive macros
4. **Logging**: Use `tracing` for structured logging
5. **Concurrency**: Use `DashMap` and `parking_lot` for thread-safe data structures
6. **API Design**: Focus on ergonomic, type-safe APIs

## Module Organization

- `src/lib.rs` - Main library exports and documentation
- `src/manager.rs` - Core ConfigManager implementation
- `src/sources/` - Configuration source implementations (file, env, remote)
- `src/validation/` - Schema validation logic
- `src/watcher.rs` - Configuration change watching
- `src/value.rs` - ConfigValue type and conversions
- `src/error.rs` - Error types
- `src/backends/` - Optional distributed backends (Redis, etcd)

## Key Design Patterns

1. **Builder Pattern**: Use for configuring sources and managers
2. **Type Safety**: Leverage Rust's type system for configuration validation
3. **Async Streams**: Use for configuration change notifications
4. **Trait Objects**: Use for pluggable configuration sources and backends

## Testing Guidelines

- Unit tests for individual components
- Integration tests for end-to-end scenarios
- Mock external dependencies in tests
- Test error conditions and edge cases
- Use `tokio-test` for async test utilities
