# Changelog

All notable changes to the `@tevalabs/xelma-bindings` package will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] - 2026-04-28

### Added
- `FutureOracleData` (24) and `PayoutOverflow` (25) entries to `ContractError` map, bringing bindings in sync with all 25 contract error codes.
- Error-code parity guard in `parity.js`: compares `errors.rs` enum variants against `ContractError` map and fails CI if any code is missing, renamed, or added without a corresponding bindings update.

## [1.1.0] - 2026-03-25

### Added
- Initial formal release with automated CI validation.
- Standardized package name to `@tevalabs/xelma-bindings`.
- Build + ABI parity publish guard.
- Release workflow in GitHub Actions.
