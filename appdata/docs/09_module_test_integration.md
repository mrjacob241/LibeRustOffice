# Module Test Integration

## Purpose

This document defines how testing should be integrated into the Rust codebase so that the test structure remains modular and evolves with the production modules.

## Core rule

Tests should validate the real Rust modules directly. They should not primarily live as detached standalone Rust scripts that duplicate wiring already present in the application.

## Test integration model

The planned structure is:

- module-local unit tests for isolated logic
- crate-level integration tests for multi-module flows
- Python-based orchestration for unified execution, artifact collection, and markdown report generation only

## Module-local unit tests

Each core module should carry its own focused unit coverage.

Examples:

- `document_io` tests should verify package reading and validation behavior
- `odt_parser` tests should verify XML parsing, namespace handling, and style extraction
- `document_model` tests should verify invariants, normalization, and identity rules
- `editor_core` tests should verify command execution, cursor behavior, and undo/redo logic
- `export_pipeline` tests should verify serialization rules
- `screenshot_analysis` tests should verify diff metrics and classification behavior

## Why module-local tests are preferred

- the tests stay close to the code they protect
- refactors are less likely to leave tests outdated
- module ownership remains clear
- debugging is faster because failing tests point directly to the relevant subsystem

## Crate-level integration tests

Integration tests should cover the boundaries between modules rather than replacing unit coverage.

Priority flows:

- import `.odt` -> parse -> validate model
- parse -> render -> capture screenshot metadata
- edit -> normalize -> rerender
- export -> reopen -> compare supported structure

The current executable default-document round-trip flow lives in `examples/roundtrip_default.rs` and is described in `appdata/unit_tests/default_odt_round_trip.md`. It directly invokes the real importer and exporter, writes `sample_docs/sample_text_base_test.odt`, reloads it, and compares the supported internal model rather than relying on package byte identity.

## Role of the Python test runner

The Python runner required by the project specification should:

- invoke the Rust test commands
- collect stdout/stderr and test artifacts
- store outputs in `unit_tests/`
- generate markdown summaries in `unit_reports/`
- highlight failed tests first

It should not become the main place where business-logic assertions are written, and it should not contain duplicated application behavior that belongs inside Rust modules.

## Fixture strategy

Fixtures should be shared carefully.

Recommended approach:

- small module-specific fixtures for unit tests
- sample `.odt` and reference `.png` files for integration and visual checks
- malformed fixtures for parser and error-path validation

## Logging expectations during tests

Tests that exercise meaningful transformations should emit logs through the standard logging pipeline where useful, especially for:

- parser diagnostics
- export validation failures
- screenshot mismatch classification
- AI-assisted analysis modules

## Maintenance rule

Whenever a new Rust module is added, its testing entry points should be planned at the same time as the module API. A module without a clear testing attachment should be treated as incomplete design.
