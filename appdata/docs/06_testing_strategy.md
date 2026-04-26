# Testing Strategy

## Purpose

This document records the planned quality strategy across unit, integration, and visual regression testing.

## Planned testing layers

- Rust unit tests attached to each module
- integration tests for end-to-end document flows
- screenshot-based visual regression tests
- export round-trip validation

## Modularity rule

Tests should invoke the real Rust modules they validate instead of relying on detached standalone Rust scripts. The Python test runner required by the specification should remain an external orchestration layer that invokes Rust tests, collects outputs, and generates reports, while the core assertions remain inside the Rust codebase.

## Planned test placement

- module-local unit tests for parser, model, editor, renderer helpers, and exporter logic
- Rust integration tests for cross-module flows
- Python orchestration for unified execution and report generation only

## Required output locations

- `unit_tests/` for test outputs and collected artifacts
- `unit_reports/` for generated markdown summaries
- `appdata/unit_tests/` for persistent markdown descriptions of project-specific test workflows

## Mandatory reporting behavior

The test runner should generate a markdown report after execution, with emphasis on failed tests and actionable diagnosis.

## Current round-trip check

The default ODT semantic round-trip check is documented in `appdata/unit_tests/default_odt_round_trip.md`.

It is run with:

```bash
cargo run --example roundtrip_default
```

This check loads `sample_docs/sample_text_base.odt`, saves `sample_docs/sample_text_base_test.odt`, reloads it, and compares the supported editor model. The current exporter is not expected to produce a byte-identical ODT package because it rebuilds a reduced package and re-encodes images, so the success condition is `semantic reload comparison: ok`.

## Relationship with progress reports

Test reports in `unit_reports/` are execution artifacts. Periodic project progress summaries should instead be written in `appdata/progress/` whenever implementation status materially changes.

## Later expansion topics

- test naming conventions
- module-to-test ownership mapping
- fixture organization
- sample document strategy
- regression policy
- failure classification matrix
