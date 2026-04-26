# Document Pipeline

## Purpose

This document will track how `.odt` files are opened, parsed, normalized, validated, and prepared for rendering.

## Planned module coverage

- archive reading
- XML loading
- namespace resolution
- style parsing
- content parsing
- metadata parsing
- normalized model assembly
- validation

## Documentation expectations for later updates

Once implementation starts, this page should be expanded with:

- supported ODT structures
- unsupported structures and fallback behavior
- parser diagnostics format
- model mapping examples
- round-trip assumptions for export

## Current planning note

The pipeline should support a conservative subset first and treat unsupported constructs as explicit diagnostics.

## Current implementation snapshot

The first importer/exporter is implemented in `src/odt_pipeline.rs` and is currently wired into application startup, the `File` menu, and the `Insert` -> `Image` action from `src/main.rs`.

### Current startup flow

1. open `sample_docs/sample_text_base.odt`
2. extract `styles.xml` and `content.xml` by running `unzip -p`
3. parse a small style registry from default styles, named styles, and automatic styles
4. resolve parent-style inheritance, including span text properties layered over the surrounding paragraph/heading inline style
5. convert ODT text properties into `InlineStyle`
6. parse text blocks, spans, and first-pass bullet/numbered-list metadata into a `Vec<StyledChar>`
7. parse embedded image frames into `DocumentImage` objects and insert `U+FFFC` placeholders into the text stream
8. preserve `<text:soft-page-break/>` as a dedicated `SOFT_PAGE_BREAK_CHAR`
9. preserve self-closing empty paragraphs and headings as explicit newline markers, including trailing empty blocks
10. initialize the editor with `RichTextBoxState::from_styled_document()`
11. fall back to a built-in text buffer if loading fails

### Currently supported import subset

- `<text:h>` headings
- `<text:p>` paragraphs
- self-closing empty `<text:h .../>` / `<text:p .../>` blocks
- `<text:span>` inline spans, with text style properties applied as overrides on the current inherited inline style rather than as a reset to defaults
- first-pass `<text:list text:style-name="...">` bullet lists using the level-1 `text:bullet-char` from `<text:list-style>`
- first-pass `<text:list-level-style-number>` numbered lists using `text:start-value` and `style:num-suffix`, rendered as incrementing `\t1.\t`-style prefixes
- `<text:line-break/>`
- `<text:tab/>`
- `<text:soft-page-break/>` as a dedicated page-break marker
- `style:parent-style-name` inheritance
- paragraph style names and paragraph kind (`<text:p>` vs `<text:h>`)
- paragraph alignment, margins, and line-height from `<style:paragraph-properties>`
- `fo:font-size` in `pt`
- `fo:font-weight="bold"`
- `fo:font-style="italic"`
- `style:text-underline-style`
- `fo:color="#rrggbb"`
- `fo:background-color="#rrggbb"` and transparent/none text backgrounds as inline highlight state
- `<draw:frame>` and `<draw:image>`
- `xlink:href` for image package paths
- `svg:width` and `svg:height` for frame sizing
- `fo:margin-left`, `fo:margin-right`, `fo:margin-top`, `fo:margin-bottom` for graphic margins
- `style:horizontal-pos="center"` for first-pass horizontal centering

### Current paragraph spacing rule

Non-empty paragraph and heading closes are mapped to `\n\n` so visible paragraph gaps are preserved in the character canvas.
Bullet-list paragraphs are currently mapped to a `\t•\t` prefix and numbered-list paragraphs to an incrementing `\t1.\t`-style prefix plus a single trailing newline, so list items remain on consecutive lines while the editor adds a small extra visual row gap after each item.
Empty paragraphs remain a single newline marker, which preserves blank spacer paragraphs without generating unbounded spacing.
Trailing empty paragraphs are preserved, and a final non-empty paragraph only drops its synthetic closing newlines at EOF.

### Currently supported export subset

- `File` -> `Save` overwrites the current document path when available and falls back to `Save as...` for new unsaved files
- `File` -> `Save as...` writes a first-pass `.odt`
- exports text paragraphs as `<text:p>`
- exports one-level bullet and numbered lists as `<text:list>` / `<text:list-item>` blocks with generated `<text:list-style>` definitions
- converts plain tab-prefixed bullet/numbered lines into real ODT list blocks at save time, without patching those lines into list metadata during import
- rewrites a trailing whitespace-only generated list item into a plain empty paragraph after the list closes, so a deliberate blank line after the whole list is preserved without remaining inside `<text:list>`
- exports inferred 16 pt bold headings as `<text:h text:outline-level="2">`
- exports intra-paragraph line breaks as `<text:line-break/>`
- exports tabs as `<text:tab/>`
- exports soft page breaks as `<text:soft-page-break/>`
- exports empty paragraphs, including style-correct empty blocks after soft page breaks
- exports non-default inline styles as generated `<text:span text:style-name="T...">` styles
- exports font size, bold, italic, underline, RGB font color, and RGB text background/highlight color
- preserves imported paragraph style names on `<text:p>` and `<text:h>`
- preserves heading/body node kind and `text:outline-level`
- exports paragraph alignment, margins, and line-height through paragraph style definitions
- exports page margins through a generated page layout when using the application save path or `save_document_to_odt_with_page_margins`
- emits synthetic no-bottom-margin paragraph styles before generated lists and no-margin paragraph styles inside generated list items, so save/reopen does not add extra blank lines around those lists
- exports embedded images as `Pictures/liberustoffice-image-N.png`
- emits `<draw:frame>` / `<draw:image>` entries for image placeholders
- writes `text:use-soft-page-breaks="true"` on `<office:text>`
- writes `mimetype`, `content.xml`, `styles.xml`, and `META-INF/manifest.xml`
- resolves relative target paths to absolute filesystem paths before invoking `zip`, so saving a file opened from a relative startup path writes to the intended project file instead of a path relative to the temporary export folder

### Current round-trip test

The default document save/reload check is implemented as `examples/roundtrip_default.rs` and documented in `appdata/unit_tests/default_odt_round_trip.md`.

Run it with:

```bash
cargo run --example roundtrip_default
```

The test loads `sample_docs/sample_text_base.odt`, saves `sample_docs/sample_text_base_test.odt`, reloads the saved file, and compares the supported internal editor model. The saved package is currently not expected to be byte-identical to the source because export rebuilds a reduced ODT package and re-encodes embedded images as PNG files. The current success condition is `semantic reload comparison: ok`.

### Current limitations

- the importer is string-scanner based rather than a namespace-aware XML parser
- unsupported ODT nodes are skipped without structured diagnostics
- bullet and numbered lists are still represented in the editor buffer as prefixed text plus paragraph metadata rather than a dedicated list block tree; save export writes one-level ODT list blocks, but nested list levels are not modeled yet
- paragraph alignment, margins, and line-height are preserved for ODT export and partially consumed by the editor's on-screen layout engine
- embedded images are imported and rendered, but frame wrapping/anchoring behavior is only approximated
- image selection, resize handles, and drag positioning are not implemented
- no temporary extraction folder is created yet because `unzip -p` streams XML directly
- export currently shells out to `zip` and serializes a narrow text/style subset
- export rebuilds a reduced ODT package and does not preserve `meta.xml`, `settings.xml`, `manifest.rdf`, thumbnails, or the original style/master-page graph verbatim
- embedded images are decoded into `DocumentImage` and exported as generated PNG files, so the original JPEG encoding/container metadata is not preserved
