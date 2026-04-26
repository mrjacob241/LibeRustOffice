# Default ODT Round-Trip Test

## Purpose

This test checks whether the default sample document can be loaded through the real ODT importer, saved through the real ODT exporter, and loaded again without changing the supported internal document model.

The test intentionally uses `sample_docs/sample_text_base.odt` because that is also the startup document used by the application.

## Command

```bash
cargo run --example roundtrip_default
```

## Files

- Source document: `sample_docs/sample_text_base.odt`
- Saved test document: `sample_docs/sample_text_base_test.odt`
- Test harness: `examples/roundtrip_default.rs`

## What The Test Does

1. Loads `sample_docs/sample_text_base.odt` with `load_document_from_odt`.
2. Saves it as `sample_docs/sample_text_base_test.odt` with `save_document_to_odt_with_page_margins`.
3. Reloads the saved file with `load_document_from_odt`.
4. Reports whether the two ODT packages are byte-identical.
5. Compares the supported semantic model after reload:
   - character values
   - paragraph style metadata
   - visible inline styles
   - embedded image count, size, margins, centering, and pixels
   - page margins, with a small numeric tolerance

## Current Expected Result

The files are **not expected to be byte-identical**.

The exporter rebuilds a reduced ODT package and currently re-encodes embedded images as generated PNG files. As a result, the saved file can be larger and structurally different at the package/XML level while still preserving the supported editor model.

The expected passing line is:

```text
semantic reload comparison: ok
```

## Current Known Output Shape

The latest observed run reported:

```text
byte-identical: false
source bytes: 285226, saved bytes: 2288954
semantic reload comparison: ok
```

The exact byte count of the saved file may change when export serialization changes.

## Failure Interpretation

If byte identity is `false` but semantic reload comparison is `ok`, the current supported import/export model is preserved.

If semantic reload comparison fails, the failure message includes the first mismatching character index and nearby text context. Treat that as a real round-trip regression unless the mismatch is intentionally outside the supported model and the test needs to be updated with a documented reason.

## Important Caveat

This is a semantic round-trip test, not a full LibreOffice compatibility or byte-preservation test. It does not prove that unsupported ODT metadata, settings, thumbnails, original image encoding, or the original full style/master-page graph are preserved.
