# How To Build a Rich Text Canvas in egui

## Scope of this synthesis

This document is a synthesis of the Markdown and source material present in this repository, with the code as the primary source of truth. At the time of writing, the workspace contains only one existing Markdown file, `README.md`, so the detailed explanation below is derived mainly from:

- `README.md`
- `src/main.rs`
- `src/rich_textbox.rs`
- `src/odt_pipeline.rs`

The goal is not to restate every function line by line, but to explain the actual rendering system and text manipulation model implemented here, so that the same approach can be rebuilt, extended, or ported.

---

## 1. High-level architecture

The editor is built around a deliberate choice: treat the document as a sequence of styled characters, then compute a custom visual layout on top of that sequence.

That means the system does not rely on `egui::TextEdit` for rich text editing. Instead, it implements its own:

- document model
- selection model
- keyboard editing logic
- line wrapping
- page layout
- image embedding
- cursor positioning
- hit testing
- painting
- ODT import/export

At the application level, `src/main.rs` wires the editor into the UI like this:

- a top menu bar handles file and insert actions
- a dedicated toolbar mutates editor formatting state
- a central panel hosts a custom `RichTextBox` widget
- side panels expose document stats and image properties

So the actual editor is not "a text box with formatting"; it is a document canvas widget with a document serializer attached to it.

---

## 2. Core design decision: the document is `Vec<StyledChar>`

The most important structural choice is the `StyledChar` model:

```rust
pub struct StyledChar {
    pub value: char,
    pub style: InlineStyle,
    pub paragraph_style: ParagraphStyle,
}
```

Each character carries:

- its Unicode scalar value
- its inline style
- the paragraph style active at that character

This is simple, expensive, and extremely practical.

### Why this works well

This model makes many editing operations trivial:

- insert one character at cursor
- delete one character
- apply inline formatting to a selection by mutating chars in a range
- preserve paragraph metadata while editing text
- serialize directly into styled runs later

It also removes the need for a more abstract tree model early in the project.

### Tradeoffs

This representation duplicates paragraph data across all chars in the paragraph. That is not ideal for memory use or normalization, but it simplifies:

- import
- export
- painting
- selection formatting
- list metadata propagation

For a small or medium editor prototype in egui, this tradeoff is defensible.

---

## 3. The three document layers

Even though the runtime storage is character-based, the system really has three conceptual layers.

### 3.1 Inline layer

`InlineStyle` contains:

- `font_size`
- `bold`
- `italic`
- `underline`
- `color`
- `background_color`

This is the style used when painting glyphs, painting text background/highlight fills, and exporting `<text:span>` runs to ODT.

### 3.2 Paragraph layer

`ParagraphStyle` contains:

- block kind: body or heading
- style name for export/import identity
- alignment
- margins
- line height percentage
- list metadata:
  - `list_style_name`
  - `list_marker`
  - `list_number`

This layer determines block semantics and ODT paragraph/list behavior.

### 3.3 Embedded object layer

Images are not stored inline as arbitrary spans. Instead:

- the text stream contains `EMBEDDED_IMAGE_OBJECT_CHAR` (`U+FFFC`)
- actual image payload lives in `Vec<DocumentImage>`

This is a standard editor technique: the text stream keeps a placeholder token, while side storage holds the object data.

The same pattern is used for soft page breaks:

- the text stream stores `SOFT_PAGE_BREAK_CHAR`
- the export layer turns it into `<text:soft-page-break/>`

---

## 4. What state the editor actually needs

`RichTextBoxState` is the real editor state machine. The critical fields are:

- `chars: Vec<StyledChar>`
- `images: Vec<DocumentImage>`
- `cursor_index: usize`
- `typing_style: InlineStyle`
- `page_scale: f32`
- `selection_anchor: Option<usize>`
- `selection_focus: Option<usize>`
- `edit_revision: u64`

### 4.1 `cursor_index`

The cursor is stored as an insertion index between characters. This is why layout computes `cursor_points` with length `chars.len() + 1`.

That is the correct model for an editor canvas. It avoids ambiguity:

- index `0` means before the first char
- index `chars.len()` means after the last char

### 4.2 Selection as anchor/focus

Selection is represented by:

- `selection_anchor`
- `selection_focus`

Then `selected_range()` normalizes that into `min..max`.

This is better than storing only a range, because anchor/focus preserves drag direction and matches how pointer selection naturally works.

### 4.3 `typing_style`

This is crucial. The editor distinguishes:

- style of existing chars
- style to apply to newly inserted chars

Toolbar changes always update `typing_style`, and if a selection exists they also mutate the selected chars. That gives expected word-processor behavior:

- no selection: future typing changes style
- with selection: selected content changes style too

### 4.4 `edit_revision`

This acts as a cheap dirty marker for save status and document change tracking. It is incremented after mutating operations.

---

## 5. Editing model: simple operations over a flat buffer

The editor implements editing directly over the `Vec<StyledChar>`.

### 5.1 Insertion

`insert_char` does four things:

1. handles list-aware newline insertion specially
2. deletes selected content first
3. inserts a new `StyledChar` at `cursor_index`
4. advances the cursor and bumps revision

The inserted char uses:

- `typing_style`
- paragraph style borrowed from the adjacent cursor context

That adjacent paragraph style behavior matters because it keeps paragraph continuity during typing.

### 5.2 Deletion

`backspace` and `delete_forward` are intentionally low-level:

- delete selection first if one exists
- otherwise delete one char before or after the cursor

There is also special handling for empty generated list lines, which keeps list UX usable instead of leaving corrupted list prefixes behind.

### 5.3 Movement

Horizontal motion is index-based:

- `move_left`
- `move_right`
- `move_to_line_start`
- `move_to_line_end`

Vertical motion is layout-based:

- up/down arrows call `nearest_vertical_cursor_index`
- that uses actual rendered cursor positions

This separation is exactly right for a custom text canvas:

- left/right can be buffer-local
- up/down must depend on visual layout

---

## 6. Lists are implemented as text prefixes plus paragraph metadata

This is one of the most important design ideas in the codebase.

The editor does not maintain a separate list-node tree. Instead, a list item is represented by:

- visible prefix chars in the document stream
- paragraph metadata declaring that line as a list item

For example, a bullet item is stored in the text stream roughly as:

```text
\t•\tItem text
```

A numbered item looks like:

```text
\t1.\tItem text
```

### Why this is clever

This gives the user a plain character stream that is easy to edit and display, while still preserving enough metadata to export real ODT lists.

### What the list code does

The list system includes:

- detect whether current line is bullet or numbered
- insert list prefix on current line
- replace one list type with another
- remove list prefix and clear list metadata
- continue numbering when Enter is pressed at line end
- exit a list when Enter is pressed on an empty list item

### Important implication

The canvas renderer does not need special list-item drawing logic beyond normal text layout. Lists are visually ordinary characters plus paragraph spacing rules. The complexity is shifted into:

- editing rules
- export normalization
- import reconstruction

This is a good fit for egui, where building a full block layout engine would be much heavier.

---

## 7. Images are first-class document objects

`DocumentImage` stores:

- source path
- logical size
- margins
- centering flag
- decoded `ColorImage`
- lazily initialized `TextureHandle`

### 7.1 The placeholder pattern

The char stream stores an object replacement character and the image array stores the payload. The mapping is positional:

- count image object chars before cursor
- use that count as the image index

This is used both for insertion and selection.

### 7.2 Why lazy textures matter

The image bytes are decoded into `ColorImage`, but GPU texture upload is deferred until paint time via `texture_handle()`.

That is the correct split for egui:

- model owns CPU image data
- render path requests texture only when needed

### 7.3 Layout behavior for images

Images are block-like in this implementation:

- pending text line is flushed before the image
- image gets width fitting against available page content width
- image can force a move to the next page if it would overflow page content height
- after painting the image, layout resumes on a new line

So the current system treats embedded images more like anchored block objects than true inline glyph-sized objects.

---

## 8. Rendering architecture: layout first, paint second

The renderer is built on an explicit layout pass that generates intermediate geometry.

### 8.1 The layout output

`layout_document()` returns `LaidOutDocument`:

- `render_boxes: Vec<RenderBox>`
- `cursor_points: Vec<Pos2>`
- `content_height: f32`

This is the heart of the canvas.

### 8.2 `RenderBox`

Each render box is one of:

- text char
- line break
- image

Each one carries:

- a logical `Rect`
- a `RenderTransform`
- a kind pointing back to char index and, for images, image index

The key idea is that the document is laid out in logical page coordinates first, then scaled for zoom through the transform.

### 8.3 Why this matters

Separating logical geometry from visual geometry makes zoom substantially easier:

- cursor model remains stable
- page size remains conceptually A4
- same layout logic can be reused for hit testing and painting

This is one of the strongest parts of the implementation.

---

## 9. `RenderBox` and precise inline character stacking

If you want to understand how characters are stacked precisely inline in this editor, `RenderBox` is the key object.

The important point is that the editor does not paint text directly while iterating through the character buffer. Instead, it does this in two stages:

1. measure characters and accumulate inline positions
2. convert those measurements into stable rectangles (`RenderBox`)

That separation is what makes the inline layout precise and reusable.

### 9.1 The bounding-box approach

Each visible character is measured before it is painted.

The measurement path is:

- `glyph_galley(ui, value, style)`
- `glyph_cell_size(ui, value, style)`

`glyph_cell_size()` returns a `Vec2` that acts as the glyph's bounding box in the editor's logical coordinate space.

In practice, this means every character gets:

- a width
- a height

and the layout engine uses those dimensions to determine exactly where that character should sit on the line.

### 9.2 `PendingGlyph` is the pre-render inline record

Before a character becomes a `RenderBox`, it is stored as:

```rust
struct PendingGlyph {
    index: usize,
    x: f32,
    width: f32,
    height: f32,
}
```

This is the inline stacking record for one character.

It says:

- which character this is
- where its left edge should start on the line
- how wide its bounding box is
- how tall its bounding box is

That is enough information to defer final y-positioning until the line baseline is known.

### 9.3 How characters are stacked horizontally

The line builder maintains a running pen position:

- `pen_x`
- `pen_y`

For each visible character:

1. measure the glyph bounding box
2. store a `PendingGlyph` with `x = pen_x`
3. advance `pen_x` by `glyph_width + CHAR_SKIP`

So inline stacking is fundamentally:

```text
next_x = current_x + measured_width + inter_glyph_spacing
```

This is simple, but it is also exact relative to the measured bounding boxes returned by egui for each glyph.

In other words, characters are not spaced using a guessed monospace cell and not spaced by string length. They are spaced by measured glyph width.

### 9.4 How characters are aligned vertically on the same line

Horizontal stacking alone is not enough. Characters with different font sizes must still sit on a common line in a visually coherent way.

That is why the system waits until line flush time to finalize rectangles.

When `flush_pending_line()` runs, it computes:

- resolved line height
- caret height
- line baseline
- caret y position

The critical formula is:

```text
baseline_y = line_top_y + (line_height - LINE_BOTTOM_PADDING)
glyph_y    = baseline_y - glyph_height
```

Then each glyph becomes a `RenderBox` with:

- `min.x = pending.x`
- `min.y = glyph_y`
- `width = pending.width`
- `height = pending.height`

So each character rectangle is positioned by:

- left edge from accumulated inline width
- top edge from baseline minus its own measured height

This is the core of precise inline stacking in this codebase.

### 9.5 Why `RenderBox` is the right abstraction

Once each glyph has a `RenderBox`, the editor no longer needs to think in terms of raw characters for rendering.

It now has stable geometry that can be reused for:

- text painting
- selection highlight painting
- cursor hit testing
- image hit testing
- scrolling the cursor into view

That is why `RenderBox` matters. It is not just a paint helper. It is the geometry contract between layout and every later visual operation.

### 9.6 Why bounding boxes are better than painting on the fly

If the editor tried to paint each character immediately while measuring it, several things would become harder:

- selection highlight would not have reusable geometry
- cursor placement would need a separate inference pass
- hit testing would be approximate
- vertical movement would be much harder
- zoom would be less coherent

By storing the bounding box explicitly in a `RenderBox`, the editor can treat text like a set of positioned visual objects.

### 9.7 What is precise and what is still approximate

This inline model is precise in the sense that it uses measured glyph dimensions and explicit rectangles.

But it is still approximate in a few ways:

- it lays out per character, not per grapheme cluster
- it uses a fixed `CHAR_SKIP`
- bold width is simulated by adding a small width offset
- it does not use full text shaping for run-level placement

So the system is precise at the bounding-box level implemented here, but it is not yet a full typographic engine.

---

## 10. Canvas scaling and movement based on `RenderBox`

The canvas movement system is also built on the same geometry model. In this editor, scaling, hit testing, and scrolling all depend on the fact that each laid out object has a `RenderBox` and each cursor slot has a concrete screen-space position.

The important design split is:

- layout computes document geometry in logical page coordinates
- `RenderBox` applies scaling through `RenderTransform`
- egui `ScrollArea` moves the viewport over the already scaled document

So the editor is not using a separate camera system. It is using a scaled document surface inside a scrollable viewport.

### 10.1 Logical space versus visual space

Each `RenderBox` stores:

- `local_rect`
- `transform`

`local_rect` is the rectangle in document/page coordinates.

`transform` contains the current scale:

```rust
struct RenderTransform {
    scale: Vec2,
}
```

The conversion from document space to visible canvas space happens here:

```rust
fn visual_rect(self) -> Rect {
    self.transform.apply_to_rect(self.local_rect)
}
```

That means the document model and layout model can stay stable in logical page units while the actual rendered canvas grows or shrinks with zoom.

### 10.2 Why scaling is attached to `RenderBox`

Attaching scale to each `RenderBox` makes the rendering model coherent.

The same box can provide:

- logical geometry for layout reasoning
- visual geometry for painting
- visual geometry for hit testing

Without that, scaling would have to be recomputed separately in multiple places, which would make cursoring and selection much easier to desynchronize from painting.

### 10.3 What actually moves when the user zooms

When zoom changes:

- page width and page height are scaled
- page gap is scaled
- glyph painting uses scaled styles
- `RenderBox::visual_rect()` gets larger or smaller
- cursor positions are also stored in scaled coordinates

So zoom changes the size of the document surface itself, not just the appearance of the final painted pixels.

This is why scroll behavior remains consistent after zooming: the scroll area is still looking at a real, resized document extent.

### 10.4 What actually moves when the user scrolls

Scrolling is handled by egui's `ScrollArea::both()`.

The editor computes:

- a canvas width
- a canvas height based on `layout.content_height`

Then it allocates that size inside the scroll area. In effect:

- the document is a large scrollable surface
- page backgrounds, text boxes, image boxes, and cursor all live on that surface
- the viewport moves over it

So the movement system is not changing document coordinates. It is changing which portion of the scaled canvas is visible.

### 10.5 How `RenderBox` participates in hit testing during movement

Because hit testing uses scaled geometry, scrolling and zooming do not require a separate interaction model.

For example:

- image hit testing checks `render_box.visual_rect().contains(pointer_pos)`
- selection highlights are painted using `render_box.visual_rect()`

That means once a box is transformed into visual space, the same rect can be reused regardless of how the viewport has moved.

This is a strong property of the design: paint-space and hit-test-space are aligned.

### 10.6 How cursor movement and scroll movement are tied together

The cursor points computed by layout are stored already multiplied by `page_scale`.

That matters because the scroll helper:

```rust
ui.scroll_to_rect(cursor_rect, None);
```

expects a rectangle in the visible canvas coordinate system.

So the movement chain is:

1. layout computes cursor point for an insertion index
2. that cursor point is stored in scaled coordinates
3. the editor builds a cursor rect around that point
4. `ScrollArea` scrolls until that rect is visible

This is why keyboard editing, zooming, and scrolling still feel synchronized. They all meet in the same scaled coordinate space.

### 10.7 Page movement is also geometry-driven

Even the multi-page illusion follows the same rule.

The renderer computes:

- scaled page width
- scaled page height
- scaled page gap

Then `paint_page_backgrounds()` draws page sheets at those scaled positions.

So when the user scrolls vertically, they are really moving across a stacked sequence of scaled page rectangles, while the text and images remain positioned by the same underlying layout geometry.

### 10.8 Why this is better than ad hoc offset math

Many custom canvas editors end up mixing:

- logical coordinates
- zoomed coordinates
- scroll offsets
- local widget coordinates

in an ad hoc way.

This implementation avoids a lot of that confusion by using a clearer chain:

- layout in logical page space
- transform into visual space through `RenderBox`
- let `ScrollArea` own viewport movement

That makes the system easier to debug and easier to extend.

---

## 11. How line layout works

The line layout engine is intentionally manual.

### 9.1 Inputs

The function iterates through `state.chars` in order and maintains:

- `pen_x`
- `pen_y`
- current line height
- current caret height
- pending glyphs for the current line
- pending cursor slots

### 9.2 Wrapping

For normal chars:

- measure glyph width with `glyph_cell_size`
- if the next glyph would exceed `max_width`, flush current line
- continue on next line

This is simple greedy wrapping.

It does not do:

- word wrapping by token
- shaping-aware cluster wrapping
- bidi
- ligature-safe cursoring

But for a prototype editor, greedy char wrapping is enough to prove the architecture.

### 9.3 Newlines

A newline causes:

- line break box to be recorded
- cursor slots for next line to be registered
- current line to be flushed
- pen to move to the next line

The newline itself is not painted, but it remains addressable in cursor/index space.

### 9.4 Soft page breaks

`SOFT_PAGE_BREAK_CHAR` is treated specially:

- flush current line
- compute next page origin
- move pen directly to the next page top
- keep cursor points aligned at that break

This is a good example of representing structural layout control inside the text stream.

### 9.5 Images

When an image placeholder is encountered:

- flush pending text line
- compute fitted image size
- place image box
- advance vertical pen by image height plus margins
- continue from line start

Again, the image behaves as a block object in layout terms.

---

## 12. Cursor placement is a first-class output of layout

Many editors try to infer cursor positions during paint. This code does not. It computes `cursor_points` during layout.

That is the right approach.

Each cursor slot is associated with a text insertion index. When a line is flushed:

- cursor x positions are assigned
- caret y is aligned using baseline and caret height

This supports:

- direct painting of the caret
- click hit testing
- up/down navigation by row
- scroll-to-cursor behavior

Without this array, the editor would be much harder to reason about.

---

## 13. Vertical navigation uses visual rows, not text rows

The function `nearest_vertical_cursor_index()` is important.

It works by:

1. reading the current cursor point
2. grouping all cursor points into rows via `collect_cursor_rows()`
3. finding the adjacent row
4. choosing the cursor index on that row with nearest x coordinate

That is the correct mental model for arrow-up and arrow-down in a wrapped rich text editor.

The implementation is lightweight, but the idea scales well.

One limitation is that rows are grouped by y with a small epsilon. That works because layout is deterministic and mostly linear, but a more advanced engine might want explicit row IDs instead of float grouping.

---

## 14. Painting is intentionally dumb

After layout, painting is straightforward.

### 12.1 Text background and selection paint

The renderer first paints per-character text background/highlight fills, then paints selection backgrounds for selected text boxes.

That means both persistent text highlights and transient selection highlights are geometry-driven, not text-run-driven, and the active selection color remains visible above normal text background fills.
The current text selection fill is `rgba(140, 194, 255, 166)`: it composites close to the previous light-blue selection color on a white page, but remains translucent enough for persistent text backgrounds to influence the final color.

### 12.2 Glyph paint

Each text render box paints itself by:

- building a one-char `LayoutJob`
- asking egui fonts to shape/layout it
- painting the resulting galley at the box position

Bold is simulated by painting the glyph twice with a tiny x offset.

Underline is drawn manually as a line segment.

Italic relies on `TextFormat.italics`.

This is pragmatic. It avoids needing a separate font stack or custom path rendering.

### 12.3 Image paint

Image boxes simply paint the texture into their visual rect.

### 12.4 Caret paint

The caret is painted from `layout.cursor_points[state.cursor_index]`.

That means caret placement is fully layout-driven and consistent with hit testing.

### 12.5 Page paint

The canvas also paints:

- page backgrounds
- page borders
- ruler
- status bar

So the widget behaves like a paginated document surface, not a plain text region.

---

## 15. Zoom is handled by scaling the page model

Zoom is not implemented by scaling the whole UI externally. The editor keeps a `page_scale` in state and applies it consistently to:

- page width and height
- page gap
- render transforms
- caret size
- ruler and page visuals
- scrolling calculations

The document is laid out in logical A4 coordinates and transformed to screen space. This keeps the page metaphor coherent.

One nice detail is that zoom can be changed via:

- trackpad pinch through `zoom_delta`
- Ctrl/Cmd + scroll
- toolbar buttons

That makes the widget behave closer to a desktop document editor than a normal egui form control.

---

## 16. Hit testing is geometry-based

Pointer interaction is implemented through the canvas response rect and the computed layout.

### 14.1 Text hit testing

Text clicks use `nearest_cursor_index()`, which chooses the closest cursor point by weighted Manhattan distance.

This is not typographically perfect, but it is robust and simple.

### 14.2 Image hit testing

Images are detected separately via `hit_test_image_char_index()` against image render box rects.

This allows:

- single-click image selection
- drag behavior
- right-click opening the image properties tab

### 14.3 Selection behavior

The interaction model is:

- drag start on image: select that image object
- drag start on text: set selection anchor
- drag move: extend selection focus
- click on text: collapse selection and move cursor

This is exactly the kind of explicit logic you need once you stop using stock text widgets.

---

## 17. Toolbar logic is state mutation, not widget composition

The toolbar is not deeply coupled to layout. It simply reads and mutates `RichTextBoxState`.

That is a clean split.

Examples:

- `toggle_bold()` updates `typing_style.bold` and selected chars
- font size buttons update `typing_style.font_size` and selected chars
- color picker updates `typing_style.color` and selected chars
- list buttons call line-based transformations
- zoom buttons mutate `page_scale`

Because focus is explicitly returned to the editor canvas after toolbar actions, the user can keep typing immediately.

That is a small but important desktop-editor detail.

---

## 18. Paragraph semantics are carried all the way to ODT

The document is not just painted richly; it is serialized as a real ODT structure.

`ParagraphKind` distinguishes:

- `Body`
- `Heading { outline_level }`

During export:

- body paragraphs become `<text:p>`
- headings become `<text:h text:outline-level="...">`

During import:

- heading tags are reconstructed into `ParagraphKind::Heading`
- paragraph style identity is preserved when possible

This makes the editor more than a visual rich text toy. It preserves actual document semantics.

---

## 19. ODT import design: parse enough, normalize into the editor model

The ODT import path is implemented in `src/odt_pipeline.rs`.

### 17.1 Package reading

The loader reads:

- `styles.xml`
- `content.xml`
- image bytes via `unzip -p`

This is simple shell-driven archive access rather than a pure Rust ZIP parser. Pragmatic for a prototype.

### 17.2 Style registry

`StyleRegistry` resolves:

- default paragraph/text/graphic styles
- named inherited styles
- span text styles applied as overrides over the current inherited inline style
- list style definitions

This allows the importer to reconstruct editor-facing style objects from ODT style references.

### 17.3 Content extraction

`extract_document_content()` scans XML as text and reconstructs:

- paragraph boundaries
- heading levels
- nested span styles
- lists
- tabs
- line breaks
- soft page breaks
- image placeholders plus image payloads

This is not a full XML DOM parser. It is a targeted stream parser for the subset of ODT the project currently cares about.

That is a valid engineering choice as long as the input scope is understood.

---

## 20. Importing lists: from real ODT lists to editable prefix text

The importer converts `<text:list>` and `<text:list-item>` structures into editable prefix chars.

When a paragraph begins inside a list block, the importer injects:

- leading tab
- list marker or digits plus marker
- trailing tab

and applies paragraph list metadata to the chars in that line.

This is the bridge between:

- semantic list structure in ODT
- character-oriented editing model in the canvas

It is one of the most important normalization steps in the whole system.

---

## 21. Export design: from flat chars back to structured ODT

Export does the reverse.

### 19.1 Export normalization

Before generating XML, the exporter runs:

- `normalize_tab_prefixed_list_metadata_for_export`

This detects lines that merely look like list prefixes and annotates them with proper list metadata if needed.

That is a very useful defensive step, because it lets the editor save plain typed prefixes as real list blocks.

### 19.2 Inline style export

`ExportStyleRegistry` collects distinct non-default `InlineStyle` values and assigns generated names like `T1`, `T2`, and so on.

Then text runs are emitted as:

- raw text if style is default
- `<text:span text:style-name="...">...</text:span>` otherwise

This is a clean and compact mapping from per-char styling to span-based ODT markup.

### 19.3 Paragraph export

`export_paragraphs()` scans the char stream and groups content into paragraphs based mainly on newline structure.

It also handles:

- line breaks inside a paragraph
- soft page break markers
- image frames
- tabs
- empty paragraphs
- transitions into and out of generated list blocks

### 19.4 List export

This is where the text-prefix/list-metadata dual representation pays off.

The exporter:

- detects list-style paragraphs
- strips visible prefix chars from payload when generating list items
- opens and closes `<text:list>` blocks as style changes
- emits `<text:list-item><text:p>...</text:p></text:list-item>`

So the internal editing representation stays easy to manipulate, while the saved ODT remains semantically correct.

---

## 22. Exporting images

Images are exported as:

- PNG files under `Pictures/`
- manifest entries in `META-INF/manifest.xml`
- graphic styles in automatic styles
- `<draw:frame><draw:image .../></draw:frame>` inside content

Notable implementation details:

- all exported images are normalized to PNG
- width and height are exported in inches
- margin and horizontal position are preserved via graphic styles

This is a practical portability choice. It avoids needing to preserve every original image encoding format.

---

## 23. What the tests reveal about the intended contract

The test suite in both `rich_textbox.rs` and `odt_pipeline.rs` is valuable because it makes the editor contract explicit.

The tests verify:

- insertion and deletion semantics
- home/end movement behavior
- selection replacement on typing
- styled span import/export
- heading detection and preservation
- soft page break round-tripping
- image round-tripping
- bullet and numbered list round-tripping
- generated list spacing behavior
- preservation of paragraph style identity

That tells you the project’s real abstraction boundary:

- the char stream is the editing truth
- ODT import/export must round-trip back to that truth

---

## 24. Strengths of this approach

### 22.1 Very understandable data flow

The flow is easy to reason about:

1. ODT imports into `Vec<StyledChar> + Vec<DocumentImage>`
2. layout turns that into render boxes and cursor points
3. paint draws render boxes
4. input mutates the char buffer
5. export reconstructs semantic ODT

### 22.2 Good match for egui

egui is immediate-mode. This design embraces that by:

- recomputing layout cheaply from current state
- storing only essential persistent editor state
- keeping painting and interaction deterministic

### 22.3 Practical feature growth path

Because the layout engine is custom, the project can keep adding:

- alignment
- headings
- tables
- hyperlinks
- richer image controls
- comments
- selection overlays

without fighting a stock text widget.

---

## 25. Current limitations and likely extension points

This architecture is solid for a prototype, but there are real limitations.

### 23.1 Character-by-character layout

Current layout is per char, so it lacks:

- grapheme awareness
- proper word wrapping
- complex script shaping semantics
- efficient large-document performance

If this editor grows, a next step would be grouping into styled text runs and then into shaped line fragments.

### 23.2 Paragraph layout is still partial

`ParagraphAlignment` is imported/exported and now affects the character-canvas layout for start, center, end, and basic justify alignment. Paragraph margins and line-height are also consumed by the canvas path, but this is still a pragmatic approximation rather than a full Writer-compatible paragraph layout engine.

### 23.3 Paragraph margins and line-height are only partially reflected visually

The paragraph style stores margins and line-height metadata for both ODT fidelity and first-pass on-screen layout. The canvas still relies on per-character measurement, line padding, list spacing, page margins, and image margins, so complex Writer spacing rules are not fully modeled.

### 23.4 XML parsing is subset-based

The importer is a manual scanner over XML-like strings. That is fast to evolve, but brittle compared with a true XML parser if document variety expands.

### 23.5 Object mapping is positional

Images are mapped by counting object placeholder chars. That is workable now, but an object ID system would scale better if the editor adds:

- undo/redo
- drag reordering
- copy/paste of mixed objects
- collaborative edits

---

## 26. If you wanted to build this system from scratch

A pragmatic build order in egui would be:

### Phase 1: document model

- define `InlineStyle`
- define `ParagraphStyle`
- define `StyledChar`
- store document as `Vec<StyledChar>`
- add cursor index and selection anchor/focus

### Phase 2: custom layout

- iterate chars with `pen_x` and `pen_y`
- measure glyphs through egui fonts
- produce:
  - text render boxes
  - line break placeholders
  - cursor positions

### Phase 3: painting

- draw text background/highlight fills
- draw translucent selection backgrounds
- draw text boxes
- draw caret from cursor positions

### Phase 4: editing

- insert text
- delete selection
- backspace/delete
- left/right/home/end
- click-to-cursor
- drag selection
- up/down based on layout rows

### Phase 5: document canvas behavior

- page background
- ruler
- zoom
- scroll-to-cursor

### Phase 6: structured rich text features

- toolbar formatting
- list toggles
- heading metadata
- image placeholders and object storage

### Phase 7: persistence

- import/export to a semantic format like ODT

This repository is already in phases 6 and 7, with some rendering and semantic gaps still open.

---

## 27. Recommended next improvements for this codebase

If the goal is to make this editor materially stronger without rewriting it, the most valuable next steps are:

1. Make layout paragraph-aware instead of purely char-aware.
2. Replace the first-pass paragraph alignment/margin handling with a fuller paragraph layout model.
3. Refine paragraph top/bottom margin collapse and contextual spacing rules.
4. Replace char wrapping with word wrapping.
5. Introduce grapheme-aware cursor movement.
6. Give embedded objects stable IDs instead of only positional mapping.
7. Add undo/redo around `RichTextBoxState` mutations.
8. Move ODT parsing from ad hoc string scanning toward a proper XML parser when compatibility needs rise.

None of those invalidate the current design. They are natural evolutions of it.

---

## 28. Final synthesis

The rich text canvas in this repository works because it is built around one strong simplification:

> keep the editor state as a flat stream of styled characters and special object markers, then derive layout, interaction, and serialization from that stream.

Everything else follows from that:

- formatting is range mutation
- cursoring is index-based plus layout geometry
- selection is anchor/focus over the stream
- images are object markers plus side storage
- lists are visible prefix text plus paragraph metadata
- pages are a logical layout convention, not a separate document tree
- ODT is a semantic import/export layer wrapped around the same stream

For egui specifically, this is a practical and well-matched architecture. It avoids fighting the framework, keeps the state model explicit, and leaves room for progressive refinement. The code is still prototype-grade in several areas, but the underlying approach is coherent: this is not a hacked `TextEdit`, it is the beginning of a real custom document editor.
