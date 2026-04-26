use eframe::egui::{
    self,
    text::{LayoutJob, TextFormat},
    Color32, ColorImage, FontFamily, FontId, Galley, Id, Pos2, Rect, RichText, Sense, Stroke,
    TextureHandle, TextureOptions, Ui, Vec2, Widget,
};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const CHAR_SKIP: f32 = 1.5;
const PT_TO_PX: f32 = 4.0 / 3.0;
const FONT_RENDER_SCALE: f32 = 0.85;
const CURSOR_SCROLL_MARGIN_X: f32 = 24.0;
const CURSOR_SCROLL_MARGIN_Y: f32 = 32.0;
const CANVAS_BOTTOM_VIEWPORT_PADDING: f32 = 0.0;
const DOCUMENT_BOTTOM_PADDING: f32 = 24.0;
const A4_PAGE_WIDTH: f32 = 794.0;
const A4_PAGE_HEIGHT: f32 = 1123.0;
const A4_PAGE_WIDTH_CM: f32 = 21.0;
const A4_PAGE_HEIGHT_CM: f32 = 29.7;
const PAGE_HORIZONTAL_MARGIN_CM: f32 = 1.25;
const PAGE_TOP_MARGIN_CM: f32 = 1.5;
const PAGE_BOTTOM_MARGIN_CM: f32 = 2.0;
const PAGE_GAP: f32 = 28.0;
const PAGE_SIDE_MARGIN: f32 = 24.0;
const MIN_PAGE_SCALE: f32 = 0.5;
const MAX_PAGE_SCALE: f32 = 2.0;
const PAGE_SCALE_STEP: f32 = 0.1;
const RULER_BAR_HEIGHT: f32 = 24.0;
const RULER_BAR_BOTTOM_GAP: f32 = 6.0;
const STATUS_BAR_TOP_GAP: f32 = 6.0;
const STATUS_BAR_HEIGHT: f32 = 28.0;
const TOOLBAR_HOVER_EXPAND: f32 = 2.0;
const LINE_BOTTOM_PADDING: f32 = 3.0;
const LIST_ITEM_BOTTOM_PADDING: f32 = 8.0;
const ROW_Y_EPSILON: f32 = 2.0;
const EMBEDDED_IMAGE_GAP_Y: f32 = 18.0;
const IMAGE_SELECTION_STROKE_WIDTH: f32 = 1.8;
const IMAGE_SELECTION_HANDLE_RADIUS: f32 = 4.5;
const IMAGE_SELECTION_HANDLE_HIT_RADIUS: f32 = 14.0;
const IMAGE_MIN_SIZE: f32 = 24.0;
const MIN_FONT_SIZE_PT: f32 = 9.0;
const MAX_FONT_SIZE_PT: f32 = 27.0;
pub const EMBEDDED_IMAGE_OBJECT_CHAR: char = '\u{FFFC}';
pub const SOFT_PAGE_BREAK_CHAR: char = '\u{000C}';
pub const TEXT_COLOR_PALETTE: [Color32; 5] = [
    Color32::BLACK,
    Color32::RED,
    Color32::BLUE,
    Color32::GREEN,
    Color32::from_rgb(255, 165, 0),
];
pub const HIGHLIGHT_COLOR_PALETTE: [Option<Color32>; 5] = [
    None,
    Some(Color32::YELLOW),
    Some(Color32::from_rgb(255, 210, 120)),
    Some(Color32::from_rgb(180, 230, 180)),
    Some(Color32::from_rgb(190, 220, 255)),
];
pub const EDITOR_CANVAS_ID_SOURCE: &str = "libe_rust_office_editor_canvas";
const GENERATED_BULLET_LIST_STYLE_NAME: &str = "LroBulletList";
const GENERATED_BULLET_MARKER: char = '•';
const GENERATED_NUMBERED_LIST_STYLE_NAME: &str = "LroNumberedList";
const GENERATED_NUMBERED_LIST_MARKER: char = '.';
const GENERATED_NUMBERED_LIST_START: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InlineStyle {
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: Color32,
    pub background_color: Option<Color32>,
}

impl Default for InlineStyle {
    fn default() -> Self {
        Self {
            font_size: 18.0,
            bold: false,
            italic: false,
            underline: false,
            color: Color32::BLACK,
            background_color: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParagraphKind {
    Body,
    Heading { outline_level: u8 },
}

impl Default for ParagraphKind {
    fn default() -> Self {
        Self::Body
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParagraphAlignment {
    Start,
    Center,
    End,
    Justify,
}

impl Default for ParagraphAlignment {
    fn default() -> Self {
        Self::Start
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageMargins {
    pub left_cm: f32,
    pub right_cm: f32,
    pub top_cm: f32,
    pub bottom_cm: f32,
}

impl Default for PageMargins {
    fn default() -> Self {
        Self {
            left_cm: PAGE_HORIZONTAL_MARGIN_CM,
            right_cm: PAGE_HORIZONTAL_MARGIN_CM,
            top_cm: PAGE_TOP_MARGIN_CM,
            bottom_cm: PAGE_BOTTOM_MARGIN_CM,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphStyle {
    pub kind: ParagraphKind,
    pub style_name: String,
    pub alignment: ParagraphAlignment,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub line_height_percent: Option<f32>,
    pub list_style_name: Option<String>,
    pub list_marker: Option<char>,
    pub list_number: Option<u32>,
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            kind: ParagraphKind::Body,
            style_name: "LroBody".to_owned(),
            alignment: ParagraphAlignment::Start,
            margin_top: 0.0,
            margin_bottom: 9.3312,
            margin_left: 0.0,
            margin_right: 0.0,
            line_height_percent: Some(115.0),
            list_style_name: None,
            list_marker: None,
            list_number: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyledChar {
    pub value: char,
    pub style: InlineStyle,
    pub paragraph_style: ParagraphStyle,
}

impl StyledChar {
    pub fn new(value: char, style: InlineStyle, paragraph_style: ParagraphStyle) -> Self {
        Self {
            value,
            style,
            paragraph_style,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutOptions {
    pub honor_paragraph_alignment: bool,
    pub honor_paragraph_spacing: bool,
    pub show_cursor_debug: bool,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            honor_paragraph_alignment: true,
            honor_paragraph_spacing: true,
            show_cursor_debug: false,
        }
    }
}

#[derive(Clone)]
pub struct DocumentImage {
    pub path: PathBuf,
    pub size: Vec2,
    pub margin_left: f32,
    pub margin_right: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub center_horizontally: bool,
    pub color_image: ColorImage,
    texture: Option<TextureHandle>,
}

#[derive(Clone)]
pub struct RichTextBoxState {
    pub chars: Vec<StyledChar>,
    pub images: Vec<DocumentImage>,
    pub cursor_index: usize,
    pub typing_style: InlineStyle,
    pub page_scale: f32,
    pub page_margins: PageMargins,
    pub editor_active: bool,
    pub selection_anchor: Option<usize>,
    pub selection_focus: Option<usize>,
    pub edit_revision: u64,
    pub layout_options: LayoutOptions,
    image_resize_drag: Option<ImageResizeDrag>,
    open_image_tab_requested: bool,
}

#[derive(Debug, Clone, Copy)]
struct ImageResizeDrag {
    image_index: usize,
    handle: ImageResizeHandle,
    start_pointer: Pos2,
    start_size: Vec2,
    page_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageResizeHandle {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

#[derive(Debug, Clone, Copy)]
struct ImageResizeHandleHit {
    image_index: usize,
    handle: ImageResizeHandle,
}

impl RichTextBoxState {
    pub fn new(text: impl AsRef<str>) -> Self {
        let typing_style = InlineStyle::default();
        let chars = text
            .as_ref()
            .chars()
            .map(|value| StyledChar::new(value, typing_style, ParagraphStyle::default()))
            .collect::<Vec<_>>();

        Self::from_styled_chars(chars)
    }

    pub fn from_styled_chars(chars: Vec<StyledChar>) -> Self {
        Self::from_styled_document(chars, Vec::new())
    }

    pub fn from_styled_document(chars: Vec<StyledChar>, images: Vec<DocumentImage>) -> Self {
        Self::from_styled_document_with_page_margins(chars, images, PageMargins::default())
    }

    pub fn from_styled_document_with_page_margins(
        chars: Vec<StyledChar>,
        images: Vec<DocumentImage>,
        page_margins: PageMargins,
    ) -> Self {
        let typing_style = chars
            .last()
            .map(|entry| entry.style)
            .unwrap_or_else(InlineStyle::default);

        Self {
            cursor_index: chars.len(),
            chars,
            images,
            typing_style,
            page_scale: 1.0,
            page_margins,
            editor_active: true,
            selection_anchor: None,
            selection_focus: None,
            edit_revision: 0,
            layout_options: LayoutOptions::default(),
            image_resize_drag: None,
            open_image_tab_requested: false,
        }
    }

    pub fn with_embedded_image(mut self, path: impl AsRef<Path>) -> Self {
        if let Ok(image) = DocumentImage::load_from_path(path) {
            self.images.push(image);
            self.bump_edit_revision();
            self.ensure_newline_after_embedded_image();
        }
        self
    }

    pub fn insert_embedded_image(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<(), image::ImageError> {
        self.delete_selection_if_any();

        let image_index = self
            .chars
            .iter()
            .take(self.cursor_index)
            .filter(|entry| entry.value == EMBEDDED_IMAGE_OBJECT_CHAR)
            .count();
        let image = DocumentImage::load_from_path(path)?;

        self.images.insert(image_index, image);
        self.chars.insert(
            self.cursor_index,
            StyledChar::new(
                EMBEDDED_IMAGE_OBJECT_CHAR,
                self.typing_style,
                self.cursor_adjacent_paragraph_style(),
            ),
        );
        self.select_image_object(self.cursor_index);
        self.bump_edit_revision();
        Ok(())
    }

    pub fn ensure_newline_after_embedded_image(&mut self) {
        if self.images.is_empty() {
            return;
        }

        if !self
            .chars
            .iter()
            .any(|entry| entry.value == EMBEDDED_IMAGE_OBJECT_CHAR)
        {
            self.chars.push(StyledChar::new(
                EMBEDDED_IMAGE_OBJECT_CHAR,
                self.typing_style,
                self.cursor_adjacent_paragraph_style(),
            ));
            self.bump_edit_revision();
        }

        if self.chars.last().is_some_and(|entry| entry.value == '\n') {
            self.cursor_index = self.chars.len();
            self.clear_selection();
            return;
        }

        self.chars.push(StyledChar::new(
            '\n',
            self.typing_style,
            self.cursor_adjacent_paragraph_style(),
        ));
        self.cursor_index = self.chars.len();
        self.clear_selection();
        self.bump_edit_revision();
    }

    pub fn plain_text(&self) -> String {
        self.chars
            .iter()
            .filter_map(|entry| match entry.value {
                EMBEDDED_IMAGE_OBJECT_CHAR => None,
                SOFT_PAGE_BREAK_CHAR => Some('\n'),
                value => Some(value),
            })
            .collect()
    }

    pub fn word_count(&self) -> usize {
        self.plain_text().split_whitespace().count()
    }

    pub fn insert_char(&mut self, value: char) {
        if value == '\n' && self.insert_list_line_break_if_any() {
            return;
        }

        self.delete_selection_if_any();
        self.chars.insert(
            self.cursor_index,
            StyledChar::new(
                value,
                self.typing_style,
                self.cursor_adjacent_paragraph_style(),
            ),
        );
        self.cursor_index += 1;
        self.bump_edit_revision();
    }

    pub fn insert_text(&mut self, text: &str) {
        for value in text.chars() {
            if value != '\r' {
                self.insert_char(value);
            }
        }
    }

    pub fn move_left(&mut self) {
        self.clear_selection();
        self.cursor_index = self.cursor_index.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.clear_selection();
        self.cursor_index = (self.cursor_index + 1).min(self.chars.len());
    }

    pub fn move_to_line_start(&mut self) {
        self.clear_selection();
        while self.cursor_index > 0 && self.chars[self.cursor_index - 1].value != '\n' {
            self.cursor_index -= 1;
        }
    }

    pub fn move_to_line_end(&mut self) {
        self.clear_selection();
        while self.cursor_index < self.chars.len() && self.chars[self.cursor_index].value != '\n' {
            self.cursor_index += 1;
        }
    }

    pub fn backspace(&mut self) {
        if self.delete_selection_if_any() {
            return;
        }

        if self.cursor_index == 0 {
            return;
        }

        self.cursor_index -= 1;
        self.chars.remove(self.cursor_index);
        self.bump_edit_revision();
    }

    pub fn delete_forward(&mut self) {
        if self.delete_selection_if_any() {
            return;
        }

        if self.delete_empty_list_line_if_any() {
            return;
        }

        if self.cursor_index < self.chars.len() {
            self.chars.remove(self.cursor_index);
            self.bump_edit_revision();
        }
    }

    pub fn clear(&mut self) {
        if !self.chars.is_empty() {
            self.bump_edit_revision();
        }
        self.chars.clear();
        self.cursor_index = 0;
        self.clear_selection();
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_focus = None;
    }

    pub fn set_selection_point(&mut self, index: usize) {
        let index = index.min(self.chars.len());
        self.selection_anchor = Some(index);
        self.selection_focus = Some(index);
        self.cursor_index = index;
    }

    pub fn drag_selection_to(&mut self, index: usize) {
        let index = index.min(self.chars.len());
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_index);
        }
        self.selection_focus = Some(index);
        self.cursor_index = index;
    }

    pub fn selected_range(&self) -> Option<Range<usize>> {
        let anchor = self.selection_anchor?;
        let focus = self.selection_focus?;
        if anchor == focus {
            return None;
        }
        Some(anchor.min(focus)..anchor.max(focus))
    }

    pub fn select_image_object(&mut self, image_char_index: usize) {
        let start = image_char_index.min(self.chars.len());
        let end = (start + 1).min(self.chars.len());
        self.selection_anchor = Some(start);
        self.selection_focus = Some(end);
        self.cursor_index = end;
    }

    pub fn selected_image_index(&self) -> Option<usize> {
        let selection_range = self.selected_range()?;
        if selection_range.len() != 1 {
            return None;
        }

        let char_index = selection_range.start;
        if self.chars.get(char_index)?.value != EMBEDDED_IMAGE_OBJECT_CHAR {
            return None;
        }

        let image_index = self
            .chars
            .iter()
            .take(char_index + 1)
            .filter(|entry| entry.value == EMBEDDED_IMAGE_OBJECT_CHAR)
            .count()
            .saturating_sub(1);

        (image_index < self.images.len()).then_some(image_index)
    }

    pub fn selected_image(&self) -> Option<(usize, &DocumentImage)> {
        let image_index = self.selected_image_index()?;
        self.images
            .get(image_index)
            .map(|image| (image_index, image))
    }

    pub fn mark_image_edited(&mut self) {
        self.bump_edit_revision();
    }

    fn start_image_resize_drag(
        &mut self,
        image_index: usize,
        handle: ImageResizeHandle,
        pointer_pos: Pos2,
        page_scale: f32,
    ) {
        let Some(image) = self.images.get(image_index) else {
            return;
        };
        self.open_image_tab_requested = true;
        self.image_resize_drag = Some(ImageResizeDrag {
            image_index,
            handle,
            start_pointer: pointer_pos,
            start_size: image.size,
            page_scale,
        });
    }

    fn drag_image_resize_to(&mut self, pointer_pos: Pos2) -> bool {
        let Some(drag) = self.image_resize_drag else {
            return false;
        };
        let Some(image) = self.images.get_mut(drag.image_index) else {
            self.image_resize_drag = None;
            return false;
        };

        let delta = (pointer_pos - drag.start_pointer) / drag.page_scale.max(0.1);
        let mut new_size = drag.start_size;
        match drag.handle {
            ImageResizeHandle::Left => {
                new_size.x = drag.start_size.x - delta.x;
            }
            ImageResizeHandle::Right => {
                new_size.x = drag.start_size.x + delta.x;
            }
            ImageResizeHandle::Top => {
                new_size.y = drag.start_size.y - delta.y;
            }
            ImageResizeHandle::Bottom => {
                new_size.y = drag.start_size.y + delta.y;
            }
            ImageResizeHandle::TopLeft
            | ImageResizeHandle::TopRight
            | ImageResizeHandle::BottomRight
            | ImageResizeHandle::BottomLeft => {
                new_size = diagonal_image_resize_size(drag.start_size, delta, drag.handle);
            }
        }

        image.size = egui::vec2(
            new_size.x.max(IMAGE_MIN_SIZE),
            new_size.y.max(IMAGE_MIN_SIZE),
        );
        self.bump_edit_revision();
        true
    }

    fn stop_image_resize_drag(&mut self) {
        self.image_resize_drag = None;
    }

    pub fn take_open_image_tab_request(&mut self) -> bool {
        let requested = self.open_image_tab_requested;
        self.open_image_tab_requested = false;
        requested
    }

    pub fn toggle_bold(&mut self) {
        let new_value = !self.active_bold();
        self.typing_style.bold = new_value;
        self.apply_to_selection(|style| style.bold = new_value);
    }

    pub fn toggle_italic(&mut self) {
        let new_value = !self.active_italic();
        self.typing_style.italic = new_value;
        self.apply_to_selection(|style| style.italic = new_value);
    }

    pub fn toggle_underline(&mut self) {
        let new_value = !self.active_underline();
        self.typing_style.underline = new_value;
        self.apply_to_selection(|style| style.underline = new_value);
    }

    pub fn increase_font_size(&mut self) {
        let new_size = (self.active_font_size() + PT_TO_PX).min(MAX_FONT_SIZE_PT * PT_TO_PX);
        self.typing_style.font_size = new_size;
        self.apply_to_selection(|style| style.font_size = new_size);
    }

    pub fn decrease_font_size(&mut self) {
        let new_size = (self.active_font_size() - PT_TO_PX).max(MIN_FONT_SIZE_PT * PT_TO_PX);
        self.typing_style.font_size = new_size;
        self.apply_to_selection(|style| style.font_size = new_size);
    }

    pub fn cycle_text_color(&mut self) {
        let next_color = next_palette_color(self.active_color());
        self.set_text_color(next_color);
    }

    pub fn set_text_color(&mut self, color: Color32) {
        self.typing_style.color = color;
        self.apply_to_selection(|style| style.color = color);
    }

    pub fn set_highlight_color(&mut self, color: Option<Color32>) {
        self.typing_style.background_color = color;
        self.apply_to_selection(|style| style.background_color = color);
    }

    pub fn set_paragraph_alignment(&mut self, alignment: ParagraphAlignment) {
        let target_range = self.selected_range().unwrap_or_else(|| {
            let line_start = self.current_line_start_index();
            line_start..self.current_line_end_index(line_start)
        });
        self.apply_paragraph_alignment_to_range(target_range, alignment);
    }

    pub fn set_active_paragraph_horizontal_margins(&mut self, margin_left: f32, margin_right: f32) {
        let target_range = self.selected_range().unwrap_or_else(|| {
            let line_start = self.current_line_start_index();
            line_start..self.current_line_end_index(line_start)
        });
        self.apply_paragraph_horizontal_margins_to_range(
            target_range,
            margin_left.max(0.0),
            margin_right.max(0.0),
        );
    }

    pub fn zoom_in_page(&mut self) {
        self.page_scale = ((self.page_scale + PAGE_SCALE_STEP) * 100.0).round() / 100.0;
        self.page_scale = self.page_scale.min(MAX_PAGE_SCALE);
    }

    pub fn zoom_out_page(&mut self) {
        self.page_scale = ((self.page_scale - PAGE_SCALE_STEP) * 100.0).round() / 100.0;
        self.page_scale = self.page_scale.max(MIN_PAGE_SCALE);
    }

    pub fn reset_page_zoom(&mut self) {
        self.page_scale = 1.0;
    }

    pub fn active_bold(&self) -> bool {
        self.selected_range()
            .map(|range| self.chars[range].iter().all(|entry| entry.style.bold))
            .unwrap_or_else(|| self.cursor_adjacent_style().bold)
    }

    pub fn active_italic(&self) -> bool {
        self.selected_range()
            .map(|range| self.chars[range].iter().all(|entry| entry.style.italic))
            .unwrap_or_else(|| self.cursor_adjacent_style().italic)
    }

    pub fn active_underline(&self) -> bool {
        self.selected_range()
            .map(|range| self.chars[range].iter().all(|entry| entry.style.underline))
            .unwrap_or_else(|| self.cursor_adjacent_style().underline)
    }

    pub fn active_font_size(&self) -> f32 {
        self.selected_range()
            .map(|range| self.chars[range.start].style.font_size)
            .unwrap_or_else(|| self.cursor_adjacent_style().font_size)
    }

    pub fn active_color(&self) -> Color32 {
        self.selected_range()
            .map(|range| self.chars[range.start].style.color)
            .unwrap_or_else(|| self.cursor_adjacent_style().color)
    }

    pub fn active_highlight_color(&self) -> Option<Color32> {
        self.selected_range()
            .map(|range| self.chars[range.start].style.background_color)
            .unwrap_or_else(|| self.cursor_adjacent_style().background_color)
    }

    pub fn active_paragraph_alignment(&self) -> ParagraphAlignment {
        self.selected_range()
            .and_then(|range| {
                self.chars
                    .get(range.start)
                    .map(|entry| entry.paragraph_style.alignment)
            })
            .unwrap_or_else(|| self.cursor_adjacent_paragraph_style().alignment)
    }

    pub fn active_paragraph_style(&self) -> ParagraphStyle {
        self.selected_range()
            .and_then(|range| {
                self.chars
                    .get(range.start)
                    .map(|entry| entry.paragraph_style.clone())
            })
            .unwrap_or_else(|| self.cursor_adjacent_paragraph_style())
    }

    pub fn active_bullet_list(&self) -> bool {
        let line_start = self.current_line_start_index();
        let line_end = self.current_line_end_index(line_start);
        self.current_line_list_style(line_start, line_end)
            .is_some_and(|style| {
                style.list_marker == Some(GENERATED_BULLET_MARKER) && style.list_number.is_none()
            })
    }

    pub fn active_numbered_list(&self) -> bool {
        let line_start = self.current_line_start_index();
        let line_end = self.current_line_end_index(line_start);
        self.current_line_list_style(line_start, line_end)
            .is_some_and(|style| {
                style.list_marker == Some(GENERATED_NUMBERED_LIST_MARKER)
                    && style.list_number.is_some()
            })
    }

    pub fn toggle_bullet_list(&mut self) {
        let line_start = self.current_line_start_index();
        let line_end = self.current_line_end_index(line_start);

        if let Some(list_style) = self.current_line_list_style(line_start, line_end) {
            if list_style.list_marker == Some(GENERATED_BULLET_MARKER)
                && list_style.list_number.is_none()
            {
                self.remove_current_line_list_prefix(line_start, line_end, &list_style);
            } else {
                self.replace_current_line_list_prefix_with_bullet(
                    line_start,
                    line_end,
                    &list_style,
                );
            }
            return;
        }

        self.insert_current_line_bullet_prefix(line_start, line_end);
    }

    pub fn toggle_numbered_list(&mut self) {
        let line_start = self.current_line_start_index();
        let line_end = self.current_line_end_index(line_start);

        if let Some(list_style) = self.current_line_list_style(line_start, line_end) {
            if list_style.list_marker == Some(GENERATED_NUMBERED_LIST_MARKER)
                && list_style.list_number.is_some()
            {
                self.remove_current_line_list_prefix(line_start, line_end, &list_style);
            } else {
                self.replace_current_line_list_prefix_with_numbered(
                    line_start,
                    line_end,
                    &list_style,
                );
            }
            return;
        }

        self.insert_current_line_numbered_prefix(line_start, line_end);
    }

    fn delete_selection_if_any(&mut self) -> bool {
        let Some(range) = self.selected_range() else {
            return false;
        };
        self.chars.drain(range.clone());
        self.cursor_index = range.start;
        self.clear_selection();
        self.bump_edit_revision();
        true
    }

    fn apply_to_selection(&mut self, mut update: impl FnMut(&mut InlineStyle)) {
        if let Some(range) = self.selected_range() {
            for entry in &mut self.chars[range] {
                update(&mut entry.style);
            }
            self.bump_edit_revision();
        }
    }

    fn bump_edit_revision(&mut self) {
        self.edit_revision = self.edit_revision.wrapping_add(1);
    }

    fn cursor_adjacent_style(&self) -> InlineStyle {
        if self.cursor_index > 0 {
            return self.chars[self.cursor_index - 1].style;
        }

        self.chars
            .first()
            .map(|entry| entry.style)
            .unwrap_or(self.typing_style)
    }

    fn cursor_adjacent_paragraph_style(&self) -> ParagraphStyle {
        if self.cursor_index > 0 {
            return self.chars[self.cursor_index - 1].paragraph_style.clone();
        }

        self.chars
            .first()
            .map(|entry| entry.paragraph_style.clone())
            .unwrap_or_else(ParagraphStyle::default)
    }

    fn delete_empty_list_line_if_any(&mut self) -> bool {
        let line_start = self.current_line_start_index();
        let line_end = self.current_line_end_index(line_start);
        let Some(list_style) = self.current_line_list_style(line_start, line_end) else {
            return false;
        };

        if !line_has_only_list_prefix(&self.chars[line_start..line_end], &list_style) {
            return false;
        }

        let mut delete_end = line_end;
        if self
            .chars
            .get(delete_end)
            .is_some_and(|entry| entry.value == '\n')
        {
            delete_end += 1;
        }
        if self
            .chars
            .get(delete_end)
            .is_some_and(|entry| entry.value == '\n')
        {
            delete_end += 1;
        }

        self.chars.drain(line_start..delete_end);
        self.cursor_index = line_start.min(self.chars.len());
        if self.cursor_index > 0
            && self
                .chars
                .get(self.cursor_index - 1)
                .is_some_and(|entry| entry.value == '\n')
        {
            clear_paragraph_list_style(&mut self.chars[self.cursor_index - 1].paragraph_style);
        }
        self.clear_selection();
        self.bump_edit_revision();
        true
    }

    fn insert_list_line_break_if_any(&mut self) -> bool {
        if self.delete_selection_if_any() {
            return false;
        }

        let line_start = self.current_line_start_index();
        let line_end = self.current_line_end_index(line_start);
        if self.cursor_index != line_end {
            return false;
        }

        let Some(current_style) = self
            .chars
            .get(line_start)
            .map(|entry| entry.paragraph_style.clone())
        else {
            return false;
        };
        let Some(list_marker) = current_style.list_marker else {
            return false;
        };

        if line_has_only_list_prefix(&self.chars[line_start..line_end], &current_style) {
            let mut plain_style = current_style;
            clear_paragraph_list_style(&mut plain_style);
            self.chars.splice(
                line_start..self.cursor_index,
                [StyledChar::new('\n', self.typing_style, plain_style)],
            );
            self.cursor_index = line_start + 1;
            self.clear_selection();
            self.bump_edit_revision();
            return true;
        }

        let next_list_number = current_style
            .list_number
            .map(|number| number.saturating_add(1));
        let mut next_style = current_style.clone();
        next_style.list_number = next_list_number;

        let mut insert_chars = vec![StyledChar::new('\n', self.typing_style, current_style)];
        insert_chars.push(StyledChar::new('\t', self.typing_style, next_style.clone()));
        if let Some(number) = next_list_number {
            insert_chars.extend(
                number
                    .to_string()
                    .chars()
                    .map(|value| StyledChar::new(value, self.typing_style, next_style.clone())),
            );
        }
        insert_chars.push(StyledChar::new(
            list_marker,
            self.typing_style,
            next_style.clone(),
        ));
        insert_chars.push(StyledChar::new('\t', self.typing_style, next_style));

        let insert_len = insert_chars.len();
        self.chars
            .splice(self.cursor_index..self.cursor_index, insert_chars);
        self.cursor_index += insert_len;
        self.clear_selection();
        self.bump_edit_revision();
        true
    }

    fn current_line_start_index(&self) -> usize {
        let mut line_start = self.cursor_index.min(self.chars.len());
        while line_start > 0 && self.chars[line_start - 1].value != '\n' {
            line_start -= 1;
        }
        line_start
    }

    fn current_line_end_index(&self, line_start: usize) -> usize {
        let mut line_end = line_start.min(self.chars.len());
        while line_end < self.chars.len() && self.chars[line_end].value != '\n' {
            line_end += 1;
        }
        line_end
    }

    fn current_line_list_style(
        &self,
        line_start: usize,
        line_end: usize,
    ) -> Option<ParagraphStyle> {
        self.chars
            .get(line_start..line_end)?
            .iter()
            .find(|entry| entry.paragraph_style.list_marker.is_some())
            .map(|entry| entry.paragraph_style.clone())
    }

    fn insert_current_line_bullet_prefix(&mut self, line_start: usize, line_end: usize) {
        let mut paragraph_style = self
            .chars
            .get(line_start)
            .map(|entry| entry.paragraph_style.clone())
            .unwrap_or_else(|| self.cursor_adjacent_paragraph_style());
        paragraph_style.list_style_name = Some(GENERATED_BULLET_LIST_STYLE_NAME.to_owned());
        paragraph_style.list_marker = Some(GENERATED_BULLET_MARKER);
        paragraph_style.list_number = None;

        self.apply_paragraph_style_to_line(line_start, line_end, paragraph_style.clone());
        self.chars.splice(
            line_start..line_start,
            [
                StyledChar::new('\t', self.typing_style, paragraph_style.clone()),
                StyledChar::new(
                    GENERATED_BULLET_MARKER,
                    self.typing_style,
                    paragraph_style.clone(),
                ),
                StyledChar::new('\t', self.typing_style, paragraph_style),
            ],
        );
        if self.cursor_index >= line_start {
            self.cursor_index += 3;
        }
        self.clear_selection();
        self.bump_edit_revision();
    }

    fn insert_current_line_numbered_prefix(&mut self, line_start: usize, line_end: usize) {
        let mut paragraph_style = self
            .chars
            .get(line_start)
            .map(|entry| entry.paragraph_style.clone())
            .unwrap_or_else(|| self.cursor_adjacent_paragraph_style());
        paragraph_style.list_style_name = Some(GENERATED_NUMBERED_LIST_STYLE_NAME.to_owned());
        paragraph_style.list_marker = Some(GENERATED_NUMBERED_LIST_MARKER);
        paragraph_style.list_number = Some(GENERATED_NUMBERED_LIST_START);

        self.apply_paragraph_style_to_line(line_start, line_end, paragraph_style.clone());
        self.chars.splice(
            line_start..line_start,
            [
                StyledChar::new('\t', self.typing_style, paragraph_style.clone()),
                StyledChar::new('1', self.typing_style, paragraph_style.clone()),
                StyledChar::new(
                    GENERATED_NUMBERED_LIST_MARKER,
                    self.typing_style,
                    paragraph_style.clone(),
                ),
                StyledChar::new('\t', self.typing_style, paragraph_style),
            ],
        );
        if self.cursor_index >= line_start {
            self.cursor_index += 4;
        }
        self.clear_selection();
        self.bump_edit_revision();
    }

    fn replace_current_line_list_prefix_with_bullet(
        &mut self,
        line_start: usize,
        line_end: usize,
        list_style: &ParagraphStyle,
    ) {
        let Some(prefix_len) = list_prefix_len(&self.chars[line_start..line_end], list_style)
        else {
            return;
        };
        let mut paragraph_style = list_style.clone();
        paragraph_style.list_style_name = Some(GENERATED_BULLET_LIST_STYLE_NAME.to_owned());
        paragraph_style.list_marker = Some(GENERATED_BULLET_MARKER);
        paragraph_style.list_number = None;

        self.apply_paragraph_style_to_line(line_start, line_end, paragraph_style.clone());
        self.chars.splice(
            line_start..line_start + prefix_len,
            [
                StyledChar::new('\t', self.typing_style, paragraph_style.clone()),
                StyledChar::new(
                    GENERATED_BULLET_MARKER,
                    self.typing_style,
                    paragraph_style.clone(),
                ),
                StyledChar::new('\t', self.typing_style, paragraph_style),
            ],
        );
        self.cursor_index = if self.cursor_index <= line_start {
            self.cursor_index
        } else {
            line_start + 3 + self.cursor_index.saturating_sub(line_start + prefix_len)
        }
        .min(self.chars.len());
        self.clear_selection();
        self.bump_edit_revision();
    }

    fn replace_current_line_list_prefix_with_numbered(
        &mut self,
        line_start: usize,
        line_end: usize,
        list_style: &ParagraphStyle,
    ) {
        let Some(prefix_len) = list_prefix_len(&self.chars[line_start..line_end], list_style)
        else {
            return;
        };
        let mut paragraph_style = list_style.clone();
        paragraph_style.list_style_name = Some(GENERATED_NUMBERED_LIST_STYLE_NAME.to_owned());
        paragraph_style.list_marker = Some(GENERATED_NUMBERED_LIST_MARKER);
        paragraph_style.list_number = Some(GENERATED_NUMBERED_LIST_START);

        self.apply_paragraph_style_to_line(line_start, line_end, paragraph_style.clone());
        self.chars.splice(
            line_start..line_start + prefix_len,
            [
                StyledChar::new('\t', self.typing_style, paragraph_style.clone()),
                StyledChar::new('1', self.typing_style, paragraph_style.clone()),
                StyledChar::new(
                    GENERATED_NUMBERED_LIST_MARKER,
                    self.typing_style,
                    paragraph_style.clone(),
                ),
                StyledChar::new('\t', self.typing_style, paragraph_style),
            ],
        );
        self.cursor_index = if self.cursor_index <= line_start {
            self.cursor_index
        } else {
            line_start + 4 + self.cursor_index.saturating_sub(line_start + prefix_len)
        }
        .min(self.chars.len());
        self.clear_selection();
        self.bump_edit_revision();
    }

    fn remove_current_line_list_prefix(
        &mut self,
        line_start: usize,
        line_end: usize,
        list_style: &ParagraphStyle,
    ) {
        let Some(prefix_len) = list_prefix_len(&self.chars[line_start..line_end], list_style)
        else {
            return;
        };

        self.chars.drain(line_start..line_start + prefix_len);
        let updated_line_end = self.current_line_end_index(line_start);
        self.clear_paragraph_style_from_line(line_start, updated_line_end);
        self.cursor_index = if self.cursor_index <= line_start {
            self.cursor_index
        } else {
            self.cursor_index.saturating_sub(prefix_len).max(line_start)
        }
        .min(self.chars.len());
        self.clear_selection();
        self.bump_edit_revision();
    }

    fn apply_paragraph_style_to_line(
        &mut self,
        line_start: usize,
        line_end: usize,
        paragraph_style: ParagraphStyle,
    ) {
        for entry in &mut self.chars[line_start..line_end] {
            entry.paragraph_style = paragraph_style.clone();
        }
        if let Some(entry) = self
            .chars
            .get_mut(line_end)
            .filter(|entry| entry.value == '\n')
        {
            entry.paragraph_style = paragraph_style;
        }
    }

    fn apply_paragraph_alignment_to_range(
        &mut self,
        range: Range<usize>,
        alignment: ParagraphAlignment,
    ) {
        if self.chars.is_empty() {
            return;
        }

        let mut line_start = range.start.min(self.chars.len());
        while line_start > 0 && self.chars[line_start - 1].value != '\n' {
            line_start -= 1;
        }

        let range_end = range.end.min(self.chars.len());
        loop {
            let line_end = self.current_line_end_index(line_start);
            let mut paragraph_style = self
                .chars
                .get(line_start)
                .or_else(|| self.chars.get(line_start.saturating_sub(1)))
                .map(|entry| entry.paragraph_style.clone())
                .unwrap_or_else(|| self.cursor_adjacent_paragraph_style());
            paragraph_style.alignment = alignment;
            self.apply_paragraph_style_to_line(line_start, line_end, paragraph_style);

            if line_end >= range_end || line_end >= self.chars.len() {
                break;
            }
            line_start = line_end + 1;
        }

        self.bump_edit_revision();
    }

    fn apply_paragraph_horizontal_margins_to_range(
        &mut self,
        range: Range<usize>,
        margin_left: f32,
        margin_right: f32,
    ) {
        if self.chars.is_empty() {
            return;
        }

        let mut line_start = range.start.min(self.chars.len());
        while line_start > 0 && self.chars[line_start - 1].value != '\n' {
            line_start -= 1;
        }

        let range_end = range.end.min(self.chars.len());
        loop {
            let line_end = self.current_line_end_index(line_start);
            let mut paragraph_style = self
                .chars
                .get(line_start)
                .or_else(|| self.chars.get(line_start.saturating_sub(1)))
                .map(|entry| entry.paragraph_style.clone())
                .unwrap_or_else(|| self.cursor_adjacent_paragraph_style());
            paragraph_style.margin_left = margin_left;
            paragraph_style.margin_right = margin_right;
            self.apply_paragraph_style_to_line(line_start, line_end, paragraph_style);

            if line_end >= range_end || line_end >= self.chars.len() {
                break;
            }
            line_start = line_end + 1;
        }

        self.bump_edit_revision();
    }

    fn clear_paragraph_style_from_line(&mut self, line_start: usize, line_end: usize) {
        for entry in &mut self.chars[line_start..line_end] {
            clear_paragraph_list_style(&mut entry.paragraph_style);
        }
        if let Some(entry) = self
            .chars
            .get_mut(line_end)
            .filter(|entry| entry.value == '\n')
        {
            clear_paragraph_list_style(&mut entry.paragraph_style);
        }
    }
}

fn clear_paragraph_list_style(paragraph_style: &mut ParagraphStyle) {
    paragraph_style.list_style_name = None;
    paragraph_style.list_marker = None;
    paragraph_style.list_number = None;
}

fn line_has_only_list_prefix(line_chars: &[StyledChar], list_style: &ParagraphStyle) -> bool {
    let Some(marker) = list_style.list_marker else {
        return false;
    };

    line_chars.iter().all(|entry| {
        entry.value == '\t'
            || entry.value == ' '
            || entry.value == marker
            || (list_style.list_number.is_some() && entry.value.is_ascii_digit())
    })
}

fn list_prefix_len(line_chars: &[StyledChar], list_style: &ParagraphStyle) -> Option<usize> {
    let mut index = 0;
    if line_chars.get(index)?.value != '\t' {
        return None;
    }
    index += 1;

    if list_style.list_number.is_some() {
        let digits_start = index;
        while line_chars
            .get(index)
            .is_some_and(|entry| entry.value.is_ascii_digit())
        {
            index += 1;
        }
        if index == digits_start {
            return None;
        }
    }

    if line_chars.get(index)?.value != list_style.list_marker? {
        return None;
    }
    index += 1;

    if line_chars.get(index)?.value != '\t' {
        return None;
    }

    Some(index + 1)
}

impl DocumentImage {
    pub fn from_encoded_bytes(
        path: PathBuf,
        bytes: &[u8],
        requested_size: Option<Vec2>,
        margin_left: f32,
        margin_right: f32,
        margin_top: f32,
        margin_bottom: f32,
        center_horizontally: bool,
    ) -> Result<Self, image::ImageError> {
        let rgba_image = image::load_from_memory(bytes)?.to_rgba8();
        let pixel_size = egui::vec2(rgba_image.width() as f32, rgba_image.height() as f32);
        let color_image = ColorImage::from_rgba_unmultiplied(
            [rgba_image.width() as usize, rgba_image.height() as usize],
            rgba_image.as_raw(),
        );

        Ok(Self {
            path,
            size: requested_size.unwrap_or(pixel_size),
            margin_left,
            margin_right,
            margin_top,
            margin_bottom,
            center_horizontally,
            color_image,
            texture: None,
        })
    }

    fn load_from_path(path: impl AsRef<Path>) -> Result<Self, image::ImageError> {
        let path = path.as_ref().to_path_buf();
        let bytes = std::fs::read(&path)?;
        Self::from_encoded_bytes(
            path,
            &bytes,
            None,
            0.0,
            0.0,
            EMBEDDED_IMAGE_GAP_Y,
            0.0,
            false,
        )
    }

    pub fn reload_from_path(&mut self, path: impl AsRef<Path>) -> Result<(), image::ImageError> {
        let replacement = Self::from_encoded_bytes(
            path.as_ref().to_path_buf(),
            &std::fs::read(path.as_ref())?,
            Some(self.size),
            self.margin_left,
            self.margin_right,
            self.margin_top,
            self.margin_bottom,
            self.center_horizontally,
        )?;

        self.path = replacement.path;
        self.color_image = replacement.color_image;
        self.texture = None;
        Ok(())
    }

    fn texture_handle(&mut self, ui: &Ui, image_index: usize) -> TextureHandle {
        if self.texture.is_none() {
            let texture_name = format!("embedded-document-image-{image_index}");
            self.texture = Some(ui.ctx().load_texture(
                texture_name,
                self.color_image.clone(),
                TextureOptions::LINEAR,
            ));
        }

        self.texture
            .as_ref()
            .expect("texture is initialized above")
            .clone()
    }
}

#[derive(Debug, Clone, Copy)]
struct RenderTransform {
    scale: Vec2,
}

impl Default for RenderTransform {
    fn default() -> Self {
        Self {
            scale: egui::vec2(1.0, 1.0),
        }
    }
}

impl RenderTransform {
    fn apply_to_rect(self, rect: Rect) -> Rect {
        Rect::from_min_size(
            Pos2::new(rect.min.x * self.scale.x, rect.min.y * self.scale.y),
            egui::vec2(rect.width() * self.scale.x, rect.height() * self.scale.y),
        )
    }

    fn apply_to_style(self, mut style: InlineStyle) -> InlineStyle {
        style.font_size *= self.scale.y.max(0.1);
        style = rendered_inline_style(style);
        style
    }
}

#[derive(Debug, Clone, Copy)]
enum RenderBoxKind {
    TextChar {
        char_index: usize,
    },
    LineBreak {
        char_index: usize,
    },
    Image {
        char_index: usize,
        image_index: usize,
    },
}

#[derive(Debug, Clone, Copy)]
struct RenderBox {
    kind: RenderBoxKind,
    local_rect: Rect,
    transform: RenderTransform,
}

impl RenderBox {
    fn char_index(self) -> Option<usize> {
        match self.kind {
            RenderBoxKind::TextChar { char_index }
            | RenderBoxKind::LineBreak { char_index }
            | RenderBoxKind::Image { char_index, .. } => Some(char_index),
        }
    }

    fn visual_rect(self) -> Rect {
        self.transform.apply_to_rect(self.local_rect)
    }

    fn is_line_break(self) -> bool {
        matches!(self.kind, RenderBoxKind::LineBreak { .. })
    }

    fn is_text_char(self) -> bool {
        matches!(self.kind, RenderBoxKind::TextChar { .. })
    }

    fn is_image(self) -> bool {
        matches!(self.kind, RenderBoxKind::Image { .. })
    }

    fn paint_text_background(self, ui: &Ui, entry: &StyledChar) {
        if !self.is_text_char() {
            return;
        }

        let paint_rect = self.visual_rect();
        let paint_style = self.transform.apply_to_style(entry.style);
        if let Some(background_color) = paint_style.background_color {
            ui.painter().rect_filled(
                paint_rect.expand2(egui::vec2(
                    rendered_char_skip() * self.transform.scale.x * 0.5,
                    1.0 * self.transform.scale.y,
                )),
                0.0,
                background_color,
            );
        }
    }

    fn paint(self, ui: &Ui, entry: &StyledChar) {
        if self.is_line_break() {
            return;
        }

        let paint_rect = self.visual_rect();
        let paint_style = self.transform.apply_to_style(entry.style);
        let galley = glyph_galley(ui, entry.value, paint_style);
        ui.painter()
            .galley(paint_rect.left_top(), galley.clone(), paint_style.color);

        if paint_style.bold {
            ui.painter().galley(
                paint_rect.left_top()
                    + egui::vec2(rendered_bold_width_offset() * self.transform.scale.x, 0.0),
                galley,
                paint_style.color,
            );
        }

        if paint_style.underline {
            let y = paint_rect.bottom() - 2.0 * self.transform.scale.y;
            ui.painter().line_segment(
                [
                    Pos2::new(paint_rect.left(), y),
                    Pos2::new(paint_rect.right(), y),
                ],
                Stroke::new(1.2 * self.transform.scale.y, paint_style.color),
            );
        }
    }

    fn paint_image(self, ui: &Ui, image: &mut DocumentImage, image_index: usize) {
        let texture = image.texture_handle(ui, image_index);
        ui.painter().image(
            texture.id(),
            self.visual_rect(),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

#[derive(Debug, Clone)]
struct LaidOutDocument {
    render_boxes: Vec<RenderBox>,
    cursor_points: Vec<Pos2>,
    content_height: f32,
}

#[derive(Debug, Clone, Copy)]
struct PendingGlyph {
    index: usize,
    x: f32,
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, Copy)]
struct ParagraphLayoutSpec {
    left_x: f32,
    width: f32,
    alignment: ParagraphAlignment,
    line_height_percent: Option<f32>,
}

impl ParagraphLayoutSpec {
    fn plain(origin_x: f32, max_width: f32) -> Self {
        Self {
            left_x: origin_x,
            width: max_width,
            alignment: ParagraphAlignment::Start,
            line_height_percent: None,
        }
    }
}

pub struct RichTextBox<'a> {
    state: &'a mut RichTextBoxState,
    desired_rows: usize,
}

impl<'a> RichTextBox<'a> {
    pub fn new(state: &'a mut RichTextBoxState) -> Self {
        Self {
            state,
            desired_rows: 12,
        }
    }

    pub fn desired_rows(mut self, rows: usize) -> Self {
        self.desired_rows = rows;
        self
    }
}

impl Widget for RichTextBox<'_> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let frame = egui::Frame::none()
            .fill(Color32::from_rgb(35, 35, 35))
            .stroke(Stroke::new(1.0, Color32::from_rgb(55, 55, 55)))
            .rounding(8.0)
            .inner_margin(12.0);

        frame
            .show(ui, |ui| {
                let viewport_width = ui.available_width();
                let page_width = scaled_page_width(self.state.page_scale);
                let canvas_width = editor_canvas_width(viewport_width, page_width);

                let reserved_fixed_height = RULER_BAR_HEIGHT
                    + RULER_BAR_BOTTOM_GAP
                    + STATUS_BAR_TOP_GAP
                    + STATUS_BAR_HEIGHT
                    + CANVAS_BOTTOM_VIEWPORT_PADDING;
                let viewport_height = (ui.available_height() - reserved_fixed_height).max(1.0);
                let keyboard_layout = layout_document(
                    ui,
                    self.state,
                    logical_page_content_origin(
                        Pos2::ZERO,
                        self.state.page_scale,
                        self.state.page_margins,
                    ),
                    page_text_width(A4_PAGE_WIDTH, self.state.page_margins),
                    self.state.page_scale,
                );
                let zoom_shortcut_changed = consume_page_zoom_shortcut(ui, self.state);
                if self.state.editor_active {
                    focus_editor_canvas(ui);
                }
                let editor_canvas_has_focus =
                    ui.memory(|memory| memory.has_focus(editor_canvas_id()));
                let keyboard_changed = self.state.editor_active
                    && editor_canvas_has_focus
                    && consume_keyboard_input(ui, self.state, &keyboard_layout);

                draw_editor_ruler_bar(ui, self.state, viewport_width, canvas_width, page_width);
                ui.add_space(RULER_BAR_BOTTOM_GAP);

                let scroll_output = egui::ScrollArea::both()
                    .id_source(editor_scroll_area_id())
                    .auto_shrink([false, false])
                    .max_height(viewport_height)
                    .show(ui, |ui| {
                        ui.set_min_width(canvas_width);

                        let sizing_layout = layout_document(
                            ui,
                            self.state,
                            logical_page_content_origin(
                                Pos2::ZERO,
                                self.state.page_scale,
                                self.state.page_margins,
                            ),
                            page_text_width(A4_PAGE_WIDTH, self.state.page_margins),
                            self.state.page_scale,
                        );
                        let canvas_size = egui::vec2(
                            canvas_width,
                            sizing_layout
                                .content_height
                                .max(scaled_page_height(self.state.page_scale))
                                .max(viewport_height),
                        );
                        let (canvas_rect, _canvas_allocation) =
                            ui.allocate_exact_size(canvas_size, Sense::hover());
                        let page_left = canvas_rect.left()
                            + ((canvas_rect.width() - page_width) * 0.5).max(0.0);
                        let page_rect = Rect::from_min_size(
                            Pos2::new(page_left, canvas_rect.top()),
                            egui::vec2(page_width, canvas_rect.height()),
                        );
                        let response =
                            ui.interact(page_rect, editor_canvas_id(), Sense::click_and_drag());

                        if response.clicked() {
                            self.state.editor_active = true;
                            response.request_focus();
                        }

                        let hit_test_layout = layout_document(
                            ui,
                            self.state,
                            logical_page_content_origin(
                                page_rect.left_top(),
                                self.state.page_scale,
                                self.state.page_margins,
                            ),
                            page_text_width(A4_PAGE_WIDTH, self.state.page_margins),
                            self.state.page_scale,
                        );

                        if let Some(pointer_pos) = response.interact_pointer_pos() {
                            let hit_resize_handle = hit_test_selected_image_resize_handle(
                                self.state,
                                &hit_test_layout,
                                pointer_pos,
                            );
                            let hit_image_char_index =
                                hit_test_image_char_index(&hit_test_layout, pointer_pos);
                            let hit_index = hit_image_char_index.unwrap_or_else(|| {
                                nearest_cursor_index(&hit_test_layout, pointer_pos)
                            });
                            if response.drag_started() {
                                if let Some(hit) = hit_resize_handle {
                                    self.state.open_image_tab_requested = true;
                                    self.state.start_image_resize_drag(
                                        hit.image_index,
                                        hit.handle,
                                        pointer_pos,
                                        self.state.page_scale,
                                    );
                                } else if let Some(image_char_index) = hit_image_char_index {
                                    self.state.select_image_object(image_char_index);
                                } else {
                                    self.state.set_selection_point(hit_index);
                                }
                            } else if response.dragged() && self.state.image_resize_drag.is_some() {
                                if self.state.drag_image_resize_to(pointer_pos) {
                                    ui.ctx().request_repaint();
                                }
                            } else if response.dragged() {
                                self.state.drag_selection_to(hit_index);
                            } else if response.clicked() {
                                if hit_resize_handle.is_some() {
                                    self.state.open_image_tab_requested = true;
                                } else if let Some(image_char_index) = hit_image_char_index {
                                    self.state.select_image_object(image_char_index);
                                } else {
                                    self.state.cursor_index = hit_index;
                                    self.state.clear_selection();
                                }
                            } else if response.secondary_clicked() {
                                if let Some(image_char_index) = hit_image_char_index {
                                    self.state.select_image_object(image_char_index);
                                    self.state.open_image_tab_requested = true;
                                }
                            }
                        }

                        if response.drag_stopped() {
                            if self.state.image_resize_drag.is_some() {
                                self.state.stop_image_resize_drag();
                            } else if self.state.selected_range().is_none() {
                                self.state.clear_selection();
                            }
                        }

                        let paint_layout = layout_document(
                            ui,
                            self.state,
                            logical_page_content_origin(
                                page_rect.left_top(),
                                self.state.page_scale,
                                self.state.page_margins,
                            ),
                            page_text_width(A4_PAGE_WIDTH, self.state.page_margins),
                            self.state.page_scale,
                        );
                        if zoom_shortcut_changed
                            || keyboard_changed
                            || response.clicked()
                            || response.dragged()
                        {
                            scroll_cursor_into_view(ui, self.state, &paint_layout);
                        }
                        paint_document(ui, self.state, &paint_layout, page_rect);
                        (response, paint_layout.content_height)
                    });

                let (response, content_height) = scroll_output.inner;
                let total_pages =
                    page_count_for_content_height(content_height, self.state.page_scale);
                let current_page = current_viewed_page(
                    scroll_output.state.offset.y,
                    viewport_height,
                    total_pages,
                    self.state.page_scale,
                );

                ui.add_space(STATUS_BAR_TOP_GAP);
                draw_status_bar(
                    ui,
                    self.state.word_count(),
                    self.state.chars.len(),
                    self.state.page_scale,
                    current_page,
                    total_pages,
                );

                ui.add_space(CANVAS_BOTTOM_VIEWPORT_PADDING);

                response
            })
            .response
    }
}

pub fn draw_editor_toolbar(ui: &mut Ui, state: &mut RichTextBoxState) {
    egui::Frame::none()
        .fill(Color32::from_rgb(35, 35, 35))
        .stroke(Stroke::new(1.0, Color32::BLACK))
        .rounding(4.0)
        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("Text style")
                        .size(13.0)
                        .color(Color32::from_rgb(240, 240, 240)),
                );
                ui.separator();

                if toolbar_toggle(ui, "Bold", state.active_bold(), true, false, false) {
                    state.toggle_bold();
                    focus_editor_canvas(ui);
                }

                if toolbar_toggle(ui, "Italic", state.active_italic(), false, true, false) {
                    state.toggle_italic();
                    focus_editor_canvas(ui);
                }

                if toolbar_toggle(
                    ui,
                    "Underline",
                    state.active_underline(),
                    false,
                    false,
                    true,
                ) {
                    state.toggle_underline();
                    focus_editor_canvas(ui);
                }

                ui.separator();
                ui.label(
                    RichText::new("Size")
                        .size(13.0)
                        .color(Color32::from_rgb(240, 240, 240)),
                );
                if toolbar_size_step(
                    ui,
                    "-",
                    state.active_font_size() > MIN_FONT_SIZE_PT * PT_TO_PX,
                ) {
                    state.decrease_font_size();
                    focus_editor_canvas(ui);
                }
                ui.label(
                    RichText::new(format!("{:.0} pt", state.active_font_size() / PT_TO_PX))
                        .size(13.0)
                        .color(Color32::from_rgb(240, 240, 240)),
                );
                if toolbar_size_step(
                    ui,
                    "+",
                    state.active_font_size() < MAX_FONT_SIZE_PT * PT_TO_PX,
                ) {
                    state.increase_font_size();
                    focus_editor_canvas(ui);
                }

                ui.separator();
                draw_color_menu(ui, state);
                draw_highlight_menu(ui, state);

                ui.separator();
                ui.label(
                    RichText::new("Zoom")
                        .size(13.0)
                        .color(Color32::from_rgb(240, 240, 240)),
                );
                if toolbar_size_step(ui, "-", state.page_scale > MIN_PAGE_SCALE) {
                    state.zoom_out_page();
                    focus_editor_canvas(ui);
                }
                let zoom_response = ui
                    .add(
                        egui::Label::new(
                            RichText::new(format!("{:.0}%", state.page_scale * 100.0))
                                .size(13.0)
                                .color(Color32::from_rgb(240, 240, 240)),
                        )
                        .sense(Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::Default);
                paint_toolbar_hover_box(ui, &zoom_response, false);
                if zoom_response.clicked() {
                    state.reset_page_zoom();
                    focus_editor_canvas(ui);
                }
                if toolbar_size_step(ui, "+", state.page_scale < MAX_PAGE_SCALE) {
                    state.zoom_in_page();
                    focus_editor_canvas(ui);
                }

                ui.separator();
                let active_alignment = state.active_paragraph_alignment();
                if toolbar_alignment_button(
                    ui,
                    "|← ",
                    active_alignment == ParagraphAlignment::Start,
                ) {
                    state.set_paragraph_alignment(ParagraphAlignment::Start);
                    focus_editor_canvas(ui);
                }
                if toolbar_alignment_button(
                    ui,
                    " ↔ ",
                    active_alignment == ParagraphAlignment::Center,
                ) {
                    state.set_paragraph_alignment(ParagraphAlignment::Center);
                    focus_editor_canvas(ui);
                }
                if toolbar_alignment_button(ui, " →|", active_alignment == ParagraphAlignment::End)
                {
                    state.set_paragraph_alignment(ParagraphAlignment::End);
                    focus_editor_canvas(ui);
                }
                if toolbar_alignment_button(
                    ui,
                    "|↔|",
                    active_alignment == ParagraphAlignment::Justify,
                ) {
                    state.set_paragraph_alignment(ParagraphAlignment::Justify);
                    focus_editor_canvas(ui);
                }

                ui.separator();
                if toolbar_toggle(ui, "• ◦ ▪", state.active_bullet_list(), false, false, false)
                {
                    state.toggle_bullet_list();
                    focus_editor_canvas(ui);
                }
                if toolbar_toggle(
                    ui,
                    "1. 2. 3.",
                    state.active_numbered_list(),
                    false,
                    false,
                    false,
                ) {
                    state.toggle_numbered_list();
                    focus_editor_canvas(ui);
                }
            });
        });
}

fn draw_editor_ruler_bar(
    ui: &mut Ui,
    state: &mut RichTextBoxState,
    viewport_width: f32,
    canvas_width: f32,
    page_width: f32,
) {
    egui::Frame::none()
        .fill(Color32::from_rgb(35, 35, 35))
        .show(ui, |ui| {
            let scroll_offset_x = egui::scroll_area::State::load(ui.ctx(), editor_scroll_area_id())
                .map(|scroll_state| scroll_state.offset.x)
                .unwrap_or(0.0);

            draw_ruler_bar(
                ui,
                viewport_width,
                canvas_width,
                page_width,
                state,
                scroll_offset_x,
            );
        });
}

fn toolbar_toggle(
    ui: &mut Ui,
    label: &str,
    active: bool,
    bold: bool,
    italic: bool,
    underline: bool,
) -> bool {
    let mut text = RichText::new(label).size(14.0).color(if active {
        Color32::from_rgb(120, 190, 255)
    } else {
        Color32::from_rgb(240, 240, 240)
    });
    if bold {
        text = text.strong();
    }
    if italic {
        text = text.italics();
    }
    if underline {
        text = text.underline();
    }

    let response = ui
        .add(egui::Label::new(text).sense(Sense::click()))
        .on_hover_cursor(egui::CursorIcon::Default);
    paint_toolbar_hover_box(ui, &response, active);
    response.clicked()
}

fn toolbar_alignment_button(ui: &mut Ui, label: &str, active: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(34.0, 20.0), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::Default);
    let color = if active {
        Color32::from_rgb(120, 190, 255)
    } else {
        Color32::from_rgb(240, 240, 240)
    };

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::monospace(14.0),
        color,
    );
    paint_toolbar_hover_box(ui, &response, active);
    response.clicked()
}

fn toolbar_size_step(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let color = if enabled {
        Color32::from_rgb(120, 190, 255)
    } else {
        Color32::from_gray(130)
    };

    if !enabled {
        return false;
    }

    let response = ui
        .add(
            egui::Label::new(RichText::new(label).size(16.0).strong().color(color))
                .sense(Sense::click()),
        )
        .on_hover_cursor(egui::CursorIcon::Default);
    paint_toolbar_hover_box(ui, &response, false);
    response.clicked()
}

fn paint_toolbar_hover_box(ui: &Ui, response: &egui::Response, active: bool) {
    if !response.hovered() {
        return;
    }

    let stroke_color = if active {
        Color32::from_rgb(120, 190, 255)
    } else {
        Color32::from_rgb(240, 240, 240)
    };
    ui.painter().rect_stroke(
        response.rect.expand(TOOLBAR_HOVER_EXPAND),
        3.0,
        Stroke::new(1.0, stroke_color),
    );
}

fn draw_color_menu(ui: &mut Ui, state: &mut RichTextBoxState) {
    let active_color = state.active_color();

    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(12.0, 2.0);
        ui.visuals_mut().widgets.inactive.weak_bg_fill = active_color;
        ui.visuals_mut().widgets.hovered.weak_bg_fill = active_color;
        ui.visuals_mut().widgets.active.weak_bg_fill = active_color;
        ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.5, Color32::WHITE);
        ui.visuals_mut().widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::WHITE);
        ui.visuals_mut().widgets.active.bg_stroke = Stroke::new(1.5, Color32::WHITE);

        ui.menu_button("   ", |ui| {
            ui.horizontal(|ui| {
                for color in TEXT_COLOR_PALETTE {
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::click());
                    ui.painter().rect_filled(rect.shrink(2.0), 4.0, color);
                    ui.painter().rect_stroke(
                        rect.shrink(2.0),
                        4.0,
                        Stroke::new(
                            if color == active_color { 2.0 } else { 1.0 },
                            Color32::from_gray(90),
                        ),
                    );

                    if response.clicked() {
                        state.set_text_color(color);
                        focus_editor_canvas(ui);
                        ui.close_menu();
                    }
                }
            });
        });
    });
}

fn draw_highlight_menu(ui: &mut Ui, state: &mut RichTextBoxState) {
    let active_color = state.active_highlight_color();
    let swatch_color = active_color.unwrap_or(Color32::from_gray(35));

    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(12.0, 2.0);
        ui.visuals_mut().widgets.inactive.weak_bg_fill = swatch_color;
        ui.visuals_mut().widgets.hovered.weak_bg_fill = swatch_color;
        ui.visuals_mut().widgets.active.weak_bg_fill = swatch_color;
        ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.5, Color32::WHITE);
        ui.visuals_mut().widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::WHITE);
        ui.visuals_mut().widgets.active.bg_stroke = Stroke::new(1.5, Color32::WHITE);

        ui.menu_button("HL", |ui| {
            ui.horizontal(|ui| {
                for color in HIGHLIGHT_COLOR_PALETTE {
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(24.0, 22.0), Sense::click());
                    let swatch_rect = rect.shrink(2.0);
                    if let Some(color) = color {
                        ui.painter().rect_filled(swatch_rect, 4.0, color);
                    } else {
                        ui.painter()
                            .rect_filled(swatch_rect, 4.0, Color32::from_gray(245));
                        ui.painter().line_segment(
                            [swatch_rect.left_bottom(), swatch_rect.right_top()],
                            Stroke::new(1.5, Color32::RED),
                        );
                    }
                    ui.painter().rect_stroke(
                        swatch_rect,
                        4.0,
                        Stroke::new(
                            if color == active_color { 2.0 } else { 1.0 },
                            Color32::from_gray(90),
                        ),
                    );

                    if response.clicked() {
                        state.set_highlight_color(color);
                        focus_editor_canvas(ui);
                        ui.close_menu();
                    }
                }
            });
        });
    });
}

fn draw_status_bar(
    ui: &mut Ui,
    word_count: usize,
    char_count: usize,
    page_scale: f32,
    current_page: usize,
    total_pages: usize,
) {
    let (bar_rect, _response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), STATUS_BAR_HEIGHT),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(bar_rect, 4.0, Color32::from_rgb(58, 58, 58));
    ui.painter()
        .rect_stroke(bar_rect, 4.0, Stroke::new(1.0, Color32::BLACK));
    ui.painter().text(
        bar_rect.left_center() + egui::vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("Words: {word_count}   Chars: {char_count}"),
        FontId::new(13.0, FontFamily::Proportional),
        Color32::from_rgb(240, 240, 240),
    );
    ui.painter().text(
        bar_rect.right_center() - egui::vec2(12.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        format!(
            "Zoom: {:.0}%   Page: {current_page}/{total_pages}",
            page_scale * 100.0
        ),
        FontId::new(13.0, FontFamily::Proportional),
        Color32::from_rgb(240, 240, 240),
    );
}

fn draw_ruler_bar(
    ui: &mut Ui,
    viewport_width: f32,
    canvas_width: f32,
    page_width: f32,
    state: &mut RichTextBoxState,
    scroll_offset_x: f32,
) {
    let (ruler_rect, _response) =
        ui.allocate_exact_size(egui::vec2(viewport_width, RULER_BAR_HEIGHT), Sense::hover());
    let page_left =
        ruler_rect.left() - scroll_offset_x + ((canvas_width - page_width) * 0.5).max(0.0);
    let page_rect = Rect::from_min_size(
        Pos2::new(page_left, ruler_rect.top()),
        egui::vec2(page_width, ruler_rect.height()),
    );
    let content_rect = Rect::from_min_max(
        Pos2::new(
            page_rect.left() + page_text_margin_left(page_width, state.page_margins),
            page_rect.top(),
        ),
        Pos2::new(
            page_rect.right() - page_text_margin_right(page_width, state.page_margins),
            page_rect.bottom(),
        ),
    );
    let page_scale = (page_width / A4_PAGE_WIDTH).clamp(MIN_PAGE_SCALE, MAX_PAGE_SCALE);
    let active_paragraph_style = state.active_paragraph_style();
    let max_paragraph_margin = (content_rect.width() / page_scale - 80.0).max(0.0);
    let paragraph_left_margin = active_paragraph_style
        .margin_left
        .clamp(0.0, max_paragraph_margin);
    let paragraph_right_margin = active_paragraph_style
        .margin_right
        .clamp(0.0, max_paragraph_margin);
    let paragraph_left_x =
        (content_rect.left() + paragraph_left_margin * page_scale).min(content_rect.right() - 16.0);
    let paragraph_right_x =
        (content_rect.right() - paragraph_right_margin * page_scale).max(paragraph_left_x + 16.0);
    let paragraph_rect = Rect::from_min_max(
        Pos2::new(paragraph_left_x, content_rect.top()),
        Pos2::new(paragraph_right_x, content_rect.bottom()),
    );

    ui.painter()
        .rect_filled(ruler_rect, 0.0, Color32::from_rgb(35, 35, 35));

    let ruler_painter = ui.painter().with_clip_rect(ruler_rect);
    ruler_painter.rect_filled(page_rect, 4.0, Color32::from_rgb(210, 210, 210));
    ruler_painter.rect_filled(content_rect, 0.0, Color32::from_rgb(242, 242, 242));
    ruler_painter.rect_filled(paragraph_rect, 0.0, Color32::from_rgb(225, 238, 250));
    ruler_painter.rect_stroke(
        page_rect,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(140, 140, 140)),
    );

    let cm_width = page_rect.width() / A4_PAGE_WIDTH_CM;
    for cm_index in 0..=A4_PAGE_WIDTH_CM as usize {
        let x = page_rect.left() + cm_index as f32 * cm_width;
        let tick_top = if cm_index % 5 == 0 {
            page_rect.top() + 4.0
        } else {
            page_rect.top() + 10.0
        };
        ruler_painter.line_segment(
            [
                Pos2::new(x, tick_top),
                Pos2::new(x, page_rect.bottom() - 4.0),
            ],
            Stroke::new(1.0, Color32::from_rgb(90, 90, 90)),
        );
    }

    for marker_x in [content_rect.left(), content_rect.right()] {
        ruler_painter.line_segment(
            [
                Pos2::new(marker_x, page_rect.top() + 2.0),
                Pos2::new(marker_x, page_rect.bottom() - 2.0),
            ],
            Stroke::new(2.0, Color32::from_rgb(20, 96, 160)),
        );
    }

    for marker_x in [paragraph_rect.left(), paragraph_rect.right()] {
        ruler_painter.line_segment(
            [
                Pos2::new(marker_x, page_rect.top() + 2.0),
                Pos2::new(marker_x, page_rect.bottom() - 2.0),
            ],
            Stroke::new(2.0, Color32::from_rgb(32, 128, 208)),
        );
    }

    let left_handle_rect = Rect::from_center_size(
        Pos2::new(paragraph_rect.left(), page_rect.center().y),
        egui::vec2(14.0, RULER_BAR_HEIGHT),
    );
    let right_handle_rect = Rect::from_center_size(
        Pos2::new(paragraph_rect.right(), page_rect.center().y),
        egui::vec2(14.0, RULER_BAR_HEIGHT),
    );

    let left_response = ui
        .interact(
            left_handle_rect,
            ui.id().with("paragraph_margin_left_handle"),
            Sense::drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    let right_response = ui
        .interact(
            right_handle_rect,
            ui.id().with("paragraph_margin_right_handle"),
            Sense::drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);

    paint_ruler_margin_handle(
        &ruler_painter,
        paragraph_rect.left(),
        page_rect,
        true,
        left_response.hovered() || left_response.dragged(),
    );
    paint_ruler_margin_handle(
        &ruler_painter,
        paragraph_rect.right(),
        page_rect,
        false,
        right_response.hovered() || right_response.dragged(),
    );

    if left_response.dragged() {
        if let Some(pointer_pos) = left_response.interact_pointer_pos() {
            let max_left_margin =
                ((paragraph_right_x - content_rect.left()) / page_scale - 80.0).max(0.0);
            let margin_left =
                ((pointer_pos.x - content_rect.left()) / page_scale).clamp(0.0, max_left_margin);
            state.set_active_paragraph_horizontal_margins(margin_left, paragraph_right_margin);
            focus_editor_canvas(ui);
        }
    }

    if right_response.dragged() {
        if let Some(pointer_pos) = right_response.interact_pointer_pos() {
            let max_right_margin =
                ((content_rect.right() - paragraph_left_x) / page_scale - 80.0).max(0.0);
            let margin_right =
                ((content_rect.right() - pointer_pos.x) / page_scale).clamp(0.0, max_right_margin);
            state.set_active_paragraph_horizontal_margins(paragraph_left_margin, margin_right);
            focus_editor_canvas(ui);
        }
    }
}

fn paint_ruler_margin_handle(
    painter: &egui::Painter,
    x: f32,
    page_rect: Rect,
    points_right: bool,
    active: bool,
) {
    let center_y = page_rect.center().y;
    let color = if active {
        Color32::from_rgb(45, 145, 235)
    } else {
        Color32::from_rgb(20, 96, 160)
    };
    let points = if points_right {
        vec![
            Pos2::new(x - 5.0, center_y - 6.0),
            Pos2::new(x + 5.0, center_y),
            Pos2::new(x - 5.0, center_y + 6.0),
        ]
    } else {
        vec![
            Pos2::new(x + 5.0, center_y - 6.0),
            Pos2::new(x - 5.0, center_y),
            Pos2::new(x + 5.0, center_y + 6.0),
        ]
    };
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        Stroke::new(1.0, Color32::from_rgb(12, 58, 98)),
    ));
}

fn scaled_page_width(page_scale: f32) -> f32 {
    A4_PAGE_WIDTH * page_scale.clamp(MIN_PAGE_SCALE, MAX_PAGE_SCALE)
}

fn editor_canvas_width(viewport_width: f32, page_width: f32) -> f32 {
    (page_width + PAGE_SIDE_MARGIN * 2.0).max(viewport_width)
}

fn scaled_page_height(page_scale: f32) -> f32 {
    A4_PAGE_HEIGHT * page_scale.clamp(MIN_PAGE_SCALE, MAX_PAGE_SCALE)
}

fn scaled_page_gap(page_scale: f32) -> f32 {
    PAGE_GAP * page_scale.clamp(MIN_PAGE_SCALE, MAX_PAGE_SCALE)
}

fn page_text_margin_left(page_width: f32, margins: PageMargins) -> f32 {
    page_width * (margins.left_cm / A4_PAGE_WIDTH_CM)
}

fn page_text_margin_right(page_width: f32, margins: PageMargins) -> f32 {
    page_width * (margins.right_cm / A4_PAGE_WIDTH_CM)
}

fn page_text_width(page_width: f32, margins: PageMargins) -> f32 {
    (page_width
        - page_text_margin_left(page_width, margins)
        - page_text_margin_right(page_width, margins))
    .max(120.0)
}

fn page_text_margin_top(page_scale: f32, margins: PageMargins) -> f32 {
    scaled_page_height(page_scale) * (margins.top_cm / A4_PAGE_HEIGHT_CM)
}

fn page_text_margin_bottom(page_scale: f32, margins: PageMargins) -> f32 {
    scaled_page_height(page_scale) * (margins.bottom_cm / A4_PAGE_HEIGHT_CM)
}

fn page_text_height(page_scale: f32, margins: PageMargins) -> f32 {
    (scaled_page_height(page_scale)
        - page_text_margin_top(page_scale, margins)
        - page_text_margin_bottom(page_scale, margins))
    .max(120.0)
}

fn logical_page_content_origin(page_top_left: Pos2, page_scale: f32, margins: PageMargins) -> Pos2 {
    Pos2::new(
        page_top_left.x / page_scale + page_text_margin_left(A4_PAGE_WIDTH, margins),
        page_top_left.y / page_scale + page_text_margin_top(1.0, margins),
    )
}

fn next_palette_color(color: Color32) -> Color32 {
    let index = TEXT_COLOR_PALETTE
        .iter()
        .position(|candidate| *candidate == color)
        .unwrap_or(0);
    TEXT_COLOR_PALETTE[(index + 1) % TEXT_COLOR_PALETTE.len()]
}

pub fn editor_canvas_id() -> Id {
    Id::new(EDITOR_CANVAS_ID_SOURCE)
}

fn editor_scroll_area_id() -> Id {
    editor_canvas_id().with("scroll_area")
}

pub fn focus_editor_canvas(ui: &mut Ui) {
    ui.memory_mut(|memory| memory.request_focus(editor_canvas_id()));
}

fn consume_keyboard_input(ui: &Ui, state: &mut RichTextBoxState, layout: &LaidOutDocument) -> bool {
    let mut changed = false;

    ui.input_mut(|input| {
        if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
            state.move_left();
            changed = true;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
            state.move_right();
            changed = true;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
            state.clear_selection();
            state.cursor_index = nearest_vertical_cursor_index(layout, state.cursor_index, -1);
            changed = true;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
            state.clear_selection();
            state.cursor_index = nearest_vertical_cursor_index(layout, state.cursor_index, 1);
            changed = true;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
            state.move_to_line_start();
            changed = true;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
            state.move_to_line_end();
            changed = true;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace) {
            state.backspace();
            changed = true;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::Delete) {
            state.delete_forward();
            changed = true;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
            state.insert_char('\n');
            changed = true;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
            state.insert_text("    ");
            changed = true;
        }

        let events = input.events.clone();
        for event in events {
            match event {
                egui::Event::Text(text) => {
                    state.insert_text(&text);
                    changed = true;
                }
                egui::Event::CompositionEnd(text) => {
                    if text != "\n" && text != "\r" {
                        state.insert_text(&text);
                        changed = true;
                    }
                }
                egui::Event::Paste(text) => {
                    state.insert_text(&text);
                    changed = true;
                }
                _ => {}
            }
        }
    });

    changed
}

fn consume_page_zoom_shortcut(ui: &Ui, state: &mut RichTextBoxState) -> bool {
    let zoom_factor = ui.input(|input| input.zoom_delta());
    if zoom_factor > 1.0 {
        state.zoom_in_page();
        return true;
    }
    if zoom_factor < 1.0 {
        state.zoom_out_page();
        return true;
    }

    let mut zoom_changed = false;

    ui.input_mut(|input| {
        if !(input.modifiers.ctrl || input.modifiers.command) {
            return;
        }

        let scroll_delta_y = input.smooth_scroll_delta.y + input.raw_scroll_delta.y;
        if scroll_delta_y > 0.0 {
            state.zoom_in_page();
            input.smooth_scroll_delta = Vec2::ZERO;
            input.raw_scroll_delta = Vec2::ZERO;
            zoom_changed = true;
        } else if scroll_delta_y < 0.0 {
            state.zoom_out_page();
            input.smooth_scroll_delta = Vec2::ZERO;
            input.raw_scroll_delta = Vec2::ZERO;
            zoom_changed = true;
        }
    });

    zoom_changed
}

fn layout_document(
    ui: &Ui,
    state: &RichTextBoxState,
    origin: Pos2,
    max_width: f32,
    page_scale: f32,
) -> LaidOutDocument {
    let mut render_boxes = Vec::with_capacity(state.chars.len());
    let mut cursor_points = vec![origin; state.chars.len() + 1];
    let mut pending_glyphs: Vec<PendingGlyph> = Vec::new();
    let mut pending_cursor_slots: Vec<(usize, f32)> = vec![(0, origin.x)];
    let mut pen_x = origin.x;
    let mut pen_y = origin.y;
    let mut line_spec = ParagraphLayoutSpec::plain(origin.x, max_width);
    let fallback_line_height =
        rendered_font_size(state.typing_style.font_size) + LINE_BOTTOM_PADDING;
    let fallback_caret_height = rendered_font_size(state.typing_style.font_size);
    let page_content_height = page_text_height(1.0, state.page_margins);
    let mut line_height = 0.0_f32;
    let mut line_caret_height = 0.0_f32;
    let mut image_object_index = 0;
    let mut at_paragraph_start = true;

    for (index, entry) in state.chars.iter().enumerate() {
        if is_synthetic_paragraph_separator(state, index) {
            cursor_points[index] = Pos2::new(pen_x * page_scale, pen_y * page_scale);
            cursor_points[index + 1] = cursor_points[index];
            continue;
        }

        if at_paragraph_start {
            let next_spec =
                paragraph_layout_spec(entry, origin.x, max_width, &state.layout_options);
            if state.layout_options.honor_paragraph_spacing {
                pen_y += entry.paragraph_style.margin_top.max(0.0);
            }
            line_spec = next_spec;
            pen_x = line_spec.left_x;
            if pending_cursor_slots.is_empty() {
                pending_cursor_slots.push((index, pen_x));
            } else if let Some(slot) = pending_cursor_slots.last_mut() {
                if slot.0 == index {
                    slot.1 = pen_x;
                }
            }
            at_paragraph_start = false;
        }

        if entry.value == EMBEDDED_IMAGE_OBJECT_CHAR {
            if !pending_glyphs.is_empty() {
                flush_pending_line(
                    state,
                    &mut render_boxes,
                    &mut cursor_points,
                    &pending_glyphs,
                    &pending_cursor_slots,
                    pen_y,
                    line_height,
                    line_caret_height,
                    fallback_line_height,
                    fallback_caret_height,
                    line_spec,
                    page_scale,
                    false,
                );
                pending_glyphs.clear();
                pending_cursor_slots.clear();

                pen_y = advance_to_next_line_y(
                    pen_y,
                    resolved_line_height_for_spec(line_height, fallback_line_height, line_spec),
                    fallback_line_height,
                    origin.y,
                    page_content_height,
                );
                line_height = 0.0;
                line_caret_height = 0.0;
            }
            pending_cursor_slots.clear();

            if let Some(image) = state.images.get(image_object_index) {
                pen_y += image.margin_top;
                let available_width =
                    (line_spec.width - image.margin_left - image.margin_right).max(1.0);
                let image_size = fit_image_size_to_width(image.size, available_width);
                let image_x = if image.center_horizontally {
                    line_spec.left_x + ((line_spec.width - image_size.x) * 0.5).max(0.0)
                } else {
                    line_spec.left_x + image.margin_left
                };

                if pen_y + image_size.y
                    > current_page_content_bottom(pen_y, origin.y, page_content_height)
                {
                    let page_stride = A4_PAGE_HEIGHT + PAGE_GAP;
                    let next_page_index = ((pen_y - origin.y) / page_stride).floor().max(0.0) + 1.0;
                    pen_y = origin.y + next_page_index * page_stride;
                }

                cursor_points[index] = Pos2::new(image_x * page_scale, pen_y * page_scale);

                render_boxes.push(RenderBox {
                    kind: RenderBoxKind::Image {
                        char_index: index,
                        image_index: image_object_index,
                    },
                    local_rect: Rect::from_min_size(
                        Pos2::new(image_x, pen_y),
                        egui::vec2(image_size.x, image_size.y),
                    ),
                    transform: RenderTransform {
                        scale: egui::vec2(page_scale, page_scale),
                    },
                });

                pen_y += image_size.y + image.margin_bottom;
                cursor_points[index + 1] = Pos2::new(image_x * page_scale, pen_y * page_scale);
                image_object_index += 1;
            }

            pen_x = line_spec.left_x;
            pending_cursor_slots.push((index + 1, pen_x));
            continue;
        }

        let glyph_size = glyph_cell_size(ui, entry.value, entry.style);
        let glyph_width = glyph_size.x;
        let glyph_height = if entry.value == '\n' {
            line_break_height(entry)
        } else {
            glyph_size.y
        };
        let glyph_caret_height = rendered_font_size(entry.style.font_size);

        if entry.value != '\n'
            && pen_x > line_spec.left_x
            && pen_x + glyph_width > line_spec.left_x + line_spec.width
        {
            flush_pending_line(
                state,
                &mut render_boxes,
                &mut cursor_points,
                &pending_glyphs,
                &pending_cursor_slots,
                pen_y,
                line_height,
                line_caret_height,
                fallback_line_height,
                fallback_caret_height,
                line_spec,
                page_scale,
                false,
            );
            pending_glyphs.clear();
            pending_cursor_slots.clear();

            pen_x = line_spec.left_x;
            pen_y = advance_to_next_line_y(
                pen_y,
                resolved_line_height_for_spec(line_height, fallback_line_height, line_spec),
                fallback_line_height,
                origin.y,
                page_content_height,
            );
            line_height = 0.0;
            line_caret_height = 0.0;
            pending_cursor_slots.push((index, pen_x));
        }

        line_height = line_height.max(glyph_height + LINE_BOTTOM_PADDING);
        line_caret_height = line_caret_height.max(glyph_caret_height);

        if entry.value == '\n' {
            pending_glyphs.push(PendingGlyph {
                index,
                x: pen_x,
                width: 0.0,
                height: glyph_height,
            });
            pending_cursor_slots.push((index + 1, line_spec.left_x));
            flush_pending_line(
                state,
                &mut render_boxes,
                &mut cursor_points,
                &pending_glyphs,
                &pending_cursor_slots,
                pen_y,
                line_height,
                line_caret_height,
                fallback_line_height,
                fallback_caret_height,
                line_spec,
                page_scale,
                true,
            );
            pending_glyphs.clear();
            pending_cursor_slots.clear();

            pen_x = line_spec.left_x;
            pen_y = advance_to_next_line_y(
                pen_y,
                resolved_line_height_for_spec(line_height, fallback_line_height, line_spec),
                fallback_line_height,
                origin.y,
                page_content_height,
            );
            if state.layout_options.honor_paragraph_spacing {
                pen_y += entry.paragraph_style.margin_bottom.max(0.0);
            }
            line_height = 0.0;
            line_caret_height = 0.0;
            at_paragraph_start = true;
            pending_cursor_slots.push((index + 1, pen_x));
            continue;
        }

        if entry.value == SOFT_PAGE_BREAK_CHAR {
            cursor_points[index] = Pos2::new(pen_x * page_scale, pen_y * page_scale);
            cursor_points[index + 1] = Pos2::new(pen_x * page_scale, pen_y * page_scale);
            pending_cursor_slots.push((index + 1, pen_x));
            continue;
        }

        pending_glyphs.push(PendingGlyph {
            index,
            x: pen_x,
            width: glyph_width,
            height: glyph_height,
        });
        pen_x += glyph_width + rendered_char_skip();
        pending_cursor_slots.push((index + 1, pen_x));
    }

    flush_pending_line(
        state,
        &mut render_boxes,
        &mut cursor_points,
        &pending_glyphs,
        &pending_cursor_slots,
        pen_y,
        line_height,
        line_caret_height,
        fallback_line_height,
        fallback_caret_height,
        line_spec,
        page_scale,
        true,
    );

    let logical_content_bottom = document_visual_bottom(
        &render_boxes,
        &cursor_points,
        pen_y,
        line_height,
        fallback_line_height,
        fallback_caret_height,
        origin.y,
        page_scale,
    );

    LaidOutDocument {
        render_boxes,
        cursor_points,
        content_height: paginated_document_height(
            logical_content_bottom + DOCUMENT_BOTTOM_PADDING,
            page_scale,
        ),
    }
}

fn document_visual_bottom(
    render_boxes: &[RenderBox],
    cursor_points: &[Pos2],
    line_top_y: f32,
    line_height: f32,
    fallback_line_height: f32,
    fallback_caret_height: f32,
    document_origin_y: f32,
    page_scale: f32,
) -> f32 {
    let mut visual_bottom = document_origin_y;

    for render_box in render_boxes {
        visual_bottom = visual_bottom.max(render_box.local_rect.bottom());
    }

    if line_height > 0.0 || render_boxes.is_empty() {
        visual_bottom =
            visual_bottom.max(line_top_y + resolved_line_height(line_height, fallback_line_height));
    }

    if render_boxes.is_empty() {
        let caret_scale = page_scale.max(0.1);
        for cursor_point in cursor_points {
            visual_bottom =
                visual_bottom.max((cursor_point.y / caret_scale) + fallback_caret_height);
        }
    }

    visual_bottom - document_origin_y
}

fn is_synthetic_paragraph_separator(state: &RichTextBoxState, index: usize) -> bool {
    if !state.layout_options.honor_paragraph_spacing {
        return false;
    }

    let Some(entry) = state.chars.get(index) else {
        return false;
    };
    if entry.value != '\n' {
        return false;
    }

    let previous_is_newline = index
        .checked_sub(1)
        .and_then(|previous_index| state.chars.get(previous_index))
        .is_some_and(|previous| previous.value == '\n');
    let before_previous_is_newline = index
        .checked_sub(2)
        .and_then(|previous_index| state.chars.get(previous_index))
        .is_some_and(|previous| previous.value == '\n');

    previous_is_newline
        && !before_previous_is_newline
        && entry.paragraph_style.list_marker.is_none()
}

fn paragraph_layout_spec(
    entry: &StyledChar,
    origin_x: f32,
    max_width: f32,
    options: &LayoutOptions,
) -> ParagraphLayoutSpec {
    if !options.honor_paragraph_spacing && !options.honor_paragraph_alignment {
        return ParagraphLayoutSpec::plain(origin_x, max_width);
    }

    let paragraph_style = &entry.paragraph_style;
    let margin_left = if options.honor_paragraph_spacing {
        paragraph_style.margin_left.max(0.0)
    } else {
        0.0
    };
    let margin_right = if options.honor_paragraph_spacing {
        paragraph_style.margin_right.max(0.0)
    } else {
        0.0
    };
    let alignment = if options.honor_paragraph_alignment {
        paragraph_style.alignment
    } else {
        ParagraphAlignment::Start
    };
    let line_height_percent = if options.honor_paragraph_spacing {
        paragraph_style.line_height_percent
    } else {
        None
    };

    ParagraphLayoutSpec {
        left_x: origin_x + margin_left,
        width: (max_width - margin_left - margin_right).max(80.0),
        alignment,
        line_height_percent,
    }
}

fn resolved_line_height_for_spec(
    line_height: f32,
    fallback_line_height: f32,
    line_spec: ParagraphLayoutSpec,
) -> f32 {
    let resolved = resolved_line_height(line_height, fallback_line_height);
    if let Some(percent) = line_spec.line_height_percent {
        resolved * (percent.max(40.0) / 100.0)
    } else {
        resolved
    }
}

fn flush_pending_line(
    state: &RichTextBoxState,
    render_boxes: &mut Vec<RenderBox>,
    cursor_points: &mut [Pos2],
    pending_glyphs: &[PendingGlyph],
    pending_cursor_slots: &[(usize, f32)],
    line_top_y: f32,
    line_height: f32,
    caret_height: f32,
    fallback_line_height: f32,
    fallback_caret_height: f32,
    line_spec: ParagraphLayoutSpec,
    page_scale: f32,
    paragraph_end: bool,
) {
    let line_height = resolved_line_height_for_spec(line_height, fallback_line_height, line_spec);
    let caret_height = if caret_height > 0.0 {
        caret_height
    } else {
        fallback_caret_height
    };
    let baseline_y = line_top_y + (line_height - LINE_BOTTOM_PADDING).max(0.0);
    let caret_y = baseline_y - caret_height;
    let (alignment_shift, justify_space_extra) =
        line_alignment_adjustments(state, pending_glyphs, line_spec, paragraph_end);

    for (cursor_index, cursor_x) in pending_cursor_slots {
        if let Some(cursor_point) = cursor_points.get_mut(*cursor_index) {
            let adjusted_x = *cursor_x
                + alignment_shift
                + justify_extra_before_x(state, pending_glyphs, *cursor_x, justify_space_extra);
            *cursor_point = Pos2::new(adjusted_x * page_scale, caret_y * page_scale);
        }
    }

    for pending in pending_glyphs {
        let glyph_y = baseline_y - pending.height;
        let glyph_x = pending.x
            + alignment_shift
            + justify_extra_before_x(state, pending_glyphs, pending.x, justify_space_extra);
        let width = if state
            .chars
            .get(pending.index)
            .is_some_and(|entry| entry.value == ' ')
        {
            pending.width + justify_space_extra
        } else {
            pending.width
        };
        let local_rect = Rect::from_min_size(
            Pos2::new(glyph_x, glyph_y),
            egui::vec2(width, pending.height),
        );
        let kind = if pending.width <= 0.0 {
            RenderBoxKind::LineBreak {
                char_index: pending.index,
            }
        } else {
            RenderBoxKind::TextChar {
                char_index: pending.index,
            }
        };
        render_boxes.push(RenderBox {
            kind,
            local_rect,
            transform: RenderTransform {
                scale: egui::vec2(page_scale, page_scale),
            },
        });
    }
}

fn line_alignment_adjustments(
    state: &RichTextBoxState,
    pending_glyphs: &[PendingGlyph],
    line_spec: ParagraphLayoutSpec,
    paragraph_end: bool,
) -> (f32, f32) {
    let content_width = pending_line_content_width(pending_glyphs);
    let remaining = (line_spec.width - content_width).max(0.0);
    let shift = match line_spec.alignment {
        ParagraphAlignment::Start | ParagraphAlignment::Justify => 0.0,
        ParagraphAlignment::Center => remaining * 0.5,
        ParagraphAlignment::End => remaining,
    };

    let justify_space_extra = if line_spec.alignment == ParagraphAlignment::Justify
        && !paragraph_end
        && remaining > 0.0
    {
        let spaces = pending_glyphs
            .iter()
            .filter(|pending| {
                state
                    .chars
                    .get(pending.index)
                    .is_some_and(|entry| entry.value == ' ')
            })
            .count();
        if spaces > 0 {
            remaining / spaces as f32
        } else {
            0.0
        }
    } else {
        0.0
    };

    (shift, justify_space_extra)
}

fn pending_line_content_width(pending_glyphs: &[PendingGlyph]) -> f32 {
    let Some(first_visible) = pending_glyphs.iter().find(|pending| pending.width > 0.0) else {
        return 0.0;
    };
    let right = pending_glyphs
        .iter()
        .filter(|pending| pending.width > 0.0)
        .map(|pending| pending.x + pending.width)
        .fold(first_visible.x, f32::max);
    (right - first_visible.x).max(0.0)
}

fn justify_extra_before_x(
    state: &RichTextBoxState,
    pending_glyphs: &[PendingGlyph],
    x: f32,
    space_extra: f32,
) -> f32 {
    if space_extra <= 0.0 {
        return 0.0;
    }

    let spaces_before = pending_glyphs
        .iter()
        .filter(|pending| pending.x + pending.width <= x)
        .filter(|pending| {
            state
                .chars
                .get(pending.index)
                .is_some_and(|entry| entry.value == ' ')
        })
        .count();
    spaces_before as f32 * space_extra
}

fn resolved_line_height(line_height: f32, fallback_line_height: f32) -> f32 {
    if line_height > 0.0 {
        line_height
    } else {
        fallback_line_height
    }
}

fn advance_to_next_line_y(
    current_line_top_y: f32,
    current_line_height: f32,
    fallback_line_height: f32,
    document_origin_y: f32,
    page_content_height: f32,
) -> f32 {
    let next_line_y =
        current_line_top_y + resolved_line_height(current_line_height, fallback_line_height);
    let page_stride = A4_PAGE_HEIGHT + PAGE_GAP;
    let current_page_index = ((current_line_top_y - document_origin_y) / page_stride)
        .floor()
        .max(0.0);
    let current_page_bottom =
        current_page_content_bottom(current_line_top_y, document_origin_y, page_content_height);

    if next_line_y + fallback_line_height > current_page_bottom {
        document_origin_y + (current_page_index + 1.0) * page_stride
    } else {
        next_line_y
    }
}

fn current_page_content_bottom(
    current_y: f32,
    document_origin_y: f32,
    page_content_height: f32,
) -> f32 {
    let page_stride = A4_PAGE_HEIGHT + PAGE_GAP;
    let current_page_index = ((current_y - document_origin_y) / page_stride)
        .floor()
        .max(0.0);
    document_origin_y + current_page_index * page_stride + page_content_height
}

fn fit_image_size_to_width(image_size: Vec2, max_width: f32) -> Vec2 {
    if image_size.x <= max_width {
        return image_size;
    }

    let scale = max_width / image_size.x.max(1.0);
    egui::vec2(max_width, image_size.y * scale)
}

fn rendered_font_size(font_size: f32) -> f32 {
    font_size * FONT_RENDER_SCALE
}

fn rendered_inline_style(mut style: InlineStyle) -> InlineStyle {
    style.font_size = rendered_font_size(style.font_size);
    style
}

fn rendered_char_skip() -> f32 {
    CHAR_SKIP * FONT_RENDER_SCALE
}

fn rendered_bold_width_offset() -> f32 {
    0.8 * FONT_RENDER_SCALE
}

fn glyph_cell_size(ui: &Ui, value: char, style: InlineStyle) -> Vec2 {
    if value == '\n' {
        return egui::vec2(
            0.0,
            rendered_font_size(style.font_size) + LINE_BOTTOM_PADDING,
        );
    }

    let style = rendered_inline_style(style);
    let galley = glyph_galley(ui, value, style);
    let mut size = galley.size();
    if style.bold {
        size.x += rendered_bold_width_offset();
    }
    size
}

fn line_break_height(entry: &StyledChar) -> f32 {
    let list_gap = if entry.paragraph_style.list_marker.is_some() {
        LIST_ITEM_BOTTOM_PADDING
    } else {
        0.0
    };
    rendered_font_size(entry.style.font_size) + LINE_BOTTOM_PADDING + list_gap
}

fn glyph_galley(ui: &Ui, value: char, style: InlineStyle) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(
        &value.to_string(),
        0.0,
        TextFormat {
            font_id: FontId::new(style.font_size, FontFamily::Proportional),
            color: style.color,
            italics: style.italic,
            ..Default::default()
        },
    );
    ui.fonts(|fonts| fonts.layout_job(job))
}

fn nearest_cursor_index(layout: &LaidOutDocument, pointer_pos: Pos2) -> usize {
    let mut best_index = 0;
    let mut best_distance = f32::INFINITY;

    for (index, cursor_pos) in layout.cursor_points.iter().enumerate() {
        let distance =
            (cursor_pos.x - pointer_pos.x).abs() + (cursor_pos.y - pointer_pos.y).abs() * 2.0;
        if distance < best_distance {
            best_distance = distance;
            best_index = index;
        }
    }

    best_index
}

fn hit_test_image_char_index(layout: &LaidOutDocument, pointer_pos: Pos2) -> Option<usize> {
    layout
        .render_boxes
        .iter()
        .find_map(|render_box| match render_box.kind {
            RenderBoxKind::Image { char_index, .. }
                if render_box.visual_rect().contains(pointer_pos) =>
            {
                Some(char_index)
            }
            _ => None,
        })
}

fn hit_test_selected_image_resize_handle(
    state: &RichTextBoxState,
    layout: &LaidOutDocument,
    pointer_pos: Pos2,
) -> Option<ImageResizeHandleHit> {
    let selected_image_index = state.selected_image_index()?;
    let selection_range = state.selected_range()?;

    layout
        .render_boxes
        .iter()
        .find_map(|render_box| match render_box.kind {
            RenderBoxKind::Image {
                char_index,
                image_index,
            } if image_index == selected_image_index && selection_range.contains(&char_index) => {
                hit_test_image_resize_handle(render_box.visual_rect(), pointer_pos).map(|handle| {
                    ImageResizeHandleHit {
                        image_index,
                        handle,
                    }
                })
            }
            _ => None,
        })
}

fn hit_test_image_resize_handle(image_rect: Rect, pointer_pos: Pos2) -> Option<ImageResizeHandle> {
    image_resize_handle_points(image_rect)
        .into_iter()
        .find_map(|(handle, point)| {
            (point.distance(pointer_pos) <= IMAGE_SELECTION_HANDLE_HIT_RADIUS).then_some(handle)
        })
}

fn nearest_vertical_cursor_index(
    layout: &LaidOutDocument,
    cursor_index: usize,
    direction: i32,
) -> usize {
    let cursor_index = cursor_index.min(layout.cursor_points.len().saturating_sub(1));
    let Some(current_pos) = layout.cursor_points.get(cursor_index).copied() else {
        return 0;
    };

    let rows = collect_cursor_rows(layout);
    let Some(current_row_index) = rows
        .iter()
        .position(|row| row.iter().any(|index| *index == cursor_index))
    else {
        return cursor_index;
    };

    let target_row_index = if direction < 0 {
        current_row_index.checked_sub(1)
    } else {
        let next_index = current_row_index + 1;
        (next_index < rows.len()).then_some(next_index)
    };

    let Some(target_row_index) = target_row_index else {
        return cursor_index;
    };

    rows[target_row_index]
        .iter()
        .copied()
        .min_by(|left_index, right_index| {
            let left_x = layout.cursor_points[*left_index].x;
            let right_x = layout.cursor_points[*right_index].x;
            (left_x - current_pos.x)
                .abs()
                .total_cmp(&(right_x - current_pos.x).abs())
        })
        .unwrap_or(cursor_index)
}

fn collect_cursor_rows(layout: &LaidOutDocument) -> Vec<Vec<usize>> {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut current_row_y: Option<f32> = None;

    for (index, point) in layout.cursor_points.iter().enumerate() {
        let same_row = current_row_y
            .map(|row_y| (point.y - row_y).abs() <= ROW_Y_EPSILON)
            .unwrap_or(false);

        if !same_row {
            rows.push(Vec::new());
            current_row_y = Some(point.y);
        }

        if let Some(row) = rows.last_mut() {
            row.push(index);
        }
    }

    rows
}

fn paint_document(
    ui: &Ui,
    state: &mut RichTextBoxState,
    layout: &LaidOutDocument,
    canvas_rect: Rect,
) {
    paint_page_backgrounds(ui, layout, canvas_rect);

    for render_box in &layout.render_boxes {
        if let Some(char_index) = render_box.char_index() {
            let entry = &state.chars[char_index];
            render_box.paint_text_background(ui, entry);
        }
    }

    if let Some(selection_range) = state.selected_range() {
        for render_box in &layout.render_boxes {
            let Some(char_index) = render_box.char_index() else {
                continue;
            };
            if selection_range.contains(&char_index) && render_box.is_text_char() {
                ui.painter().rect_filled(
                    render_box.visual_rect().expand2(egui::vec2(
                        rendered_char_skip() * state.page_scale * 0.5,
                        0.0,
                    )),
                    1.5,
                    selection_highlight_color(),
                );
            }
        }
    }

    for render_box in &layout.render_boxes {
        match render_box.kind {
            RenderBoxKind::TextChar { char_index } | RenderBoxKind::LineBreak { char_index } => {
                let entry = &state.chars[char_index];
                render_box.paint(ui, entry);
            }
            RenderBoxKind::Image { image_index, .. } => {
                if let Some(image) = state.images.get_mut(image_index) {
                    render_box.paint_image(ui, image, image_index);
                }
            }
        }
    }

    if let Some(selection_range) = state.selected_range() {
        for render_box in &layout.render_boxes {
            let Some(char_index) = render_box.char_index() else {
                continue;
            };
            if selection_range.contains(&char_index) && render_box.is_image() {
                paint_image_selection_overlay(ui, render_box.visual_rect());
            }
        }
    }

    let cursor_pos = layout
        .cursor_points
        .get(state.cursor_index)
        .copied()
        .unwrap_or(canvas_rect.left_top());
    let cursor_height = rendered_font_size(state.typing_style.font_size) * state.page_scale * 1.35;
    ui.painter().line_segment(
        [cursor_pos, cursor_pos + egui::vec2(0.0, cursor_height)],
        Stroke::new(1.5, Color32::from_rgb(20, 96, 160)),
    );

    if state.layout_options.show_cursor_debug {
        let info_pos = canvas_rect.left_bottom() + egui::vec2(0.0, 6.0);
        ui.painter().text(
            info_pos,
            egui::Align2::LEFT_TOP,
            format!("cursor {}", state.cursor_index),
            FontId::new(12.0, FontFamily::Proportional),
            Color32::GRAY,
        );
    }
}

fn selection_highlight_color() -> Color32 {
    Color32::from_rgba_unmultiplied(140, 194, 255, 166)
}

fn diagonal_image_resize_size(start_size: Vec2, delta: Vec2, handle: ImageResizeHandle) -> Vec2 {
    let aspect_ratio = (start_size.x / start_size.y.max(1.0)).max(0.01);
    let signed_delta_x = match handle {
        ImageResizeHandle::TopLeft | ImageResizeHandle::BottomLeft => -delta.x,
        ImageResizeHandle::TopRight | ImageResizeHandle::BottomRight => delta.x,
        ImageResizeHandle::Left
        | ImageResizeHandle::Right
        | ImageResizeHandle::Top
        | ImageResizeHandle::Bottom => 0.0,
    };
    let signed_delta_y = match handle {
        ImageResizeHandle::TopLeft | ImageResizeHandle::TopRight => -delta.y,
        ImageResizeHandle::BottomRight | ImageResizeHandle::BottomLeft => delta.y,
        ImageResizeHandle::Left
        | ImageResizeHandle::Right
        | ImageResizeHandle::Top
        | ImageResizeHandle::Bottom => 0.0,
    };

    let width_from_x = start_size.x + signed_delta_x;
    let width_from_y = (start_size.y + signed_delta_y) * aspect_ratio;
    let new_width = if signed_delta_x.abs() >= signed_delta_y.abs() {
        width_from_x
    } else {
        width_from_y
    }
    .max(IMAGE_MIN_SIZE);

    egui::vec2(new_width, (new_width / aspect_ratio).max(IMAGE_MIN_SIZE))
}

fn image_resize_handle_points(image_rect: Rect) -> [(ImageResizeHandle, Pos2); 8] {
    [
        (ImageResizeHandle::TopLeft, image_rect.left_top()),
        (ImageResizeHandle::Top, image_rect.center_top()),
        (ImageResizeHandle::TopRight, image_rect.right_top()),
        (ImageResizeHandle::Right, image_rect.right_center()),
        (ImageResizeHandle::BottomRight, image_rect.right_bottom()),
        (ImageResizeHandle::Bottom, image_rect.center_bottom()),
        (ImageResizeHandle::BottomLeft, image_rect.left_bottom()),
        (ImageResizeHandle::Left, image_rect.left_center()),
    ]
}

fn paint_image_selection_overlay(ui: &Ui, image_rect: Rect) {
    let stroke_color = Color32::from_rgb(20, 120, 220);
    ui.painter().rect_stroke(
        image_rect,
        0.0,
        Stroke::new(IMAGE_SELECTION_STROKE_WIDTH, stroke_color),
    );

    for (_handle, handle_point) in image_resize_handle_points(image_rect) {
        ui.painter()
            .circle_filled(handle_point, IMAGE_SELECTION_HANDLE_RADIUS, Color32::WHITE);
        ui.painter().circle_stroke(
            handle_point,
            IMAGE_SELECTION_HANDLE_RADIUS,
            Stroke::new(1.4, stroke_color),
        );
    }

    ui.painter().circle_filled(
        image_rect.center(),
        IMAGE_SELECTION_HANDLE_RADIUS,
        Color32::WHITE,
    );
    ui.painter().circle_stroke(
        image_rect.center(),
        IMAGE_SELECTION_HANDLE_RADIUS,
        Stroke::new(1.4, stroke_color),
    );
}

fn paint_page_backgrounds(ui: &Ui, layout: &LaidOutDocument, canvas_rect: Rect) {
    let page_scale = (canvas_rect.width() / A4_PAGE_WIDTH).clamp(MIN_PAGE_SCALE, MAX_PAGE_SCALE);
    let page_count = page_count_for_content_height(layout.content_height, page_scale);

    for page_index in 0..page_count {
        let page_top = canvas_rect.top()
            + page_index as f32 * (scaled_page_height(page_scale) + scaled_page_gap(page_scale));
        let page_sheet_rect = Rect::from_min_size(
            Pos2::new(canvas_rect.left(), page_top),
            egui::vec2(canvas_rect.width(), scaled_page_height(page_scale)),
        );
        ui.painter()
            .rect_filled(page_sheet_rect, 4.0, Color32::WHITE);
        ui.painter().rect_stroke(
            page_sheet_rect,
            4.0,
            Stroke::new(1.0, Color32::from_rgb(210, 206, 198)),
        );
    }
}

fn page_count_for_content_height(content_height: f32, page_scale: f32) -> usize {
    let content_height = (content_height - DOCUMENT_BOTTOM_PADDING * page_scale).max(0.0);
    (((content_height + scaled_page_gap(page_scale))
        / (scaled_page_height(page_scale) + scaled_page_gap(page_scale)))
    .ceil() as usize)
        .max(1)
}

fn paginated_document_height(content_height: f32, page_scale: f32) -> f32 {
    let content_height = content_height * page_scale;
    let page_count = page_count_for_content_height(content_height, page_scale);
    page_count as f32 * scaled_page_height(page_scale)
        + page_count.saturating_sub(1) as f32 * scaled_page_gap(page_scale)
}

fn current_viewed_page(
    scroll_offset_y: f32,
    viewport_height: f32,
    total_pages: usize,
    page_scale: f32,
) -> usize {
    let page_stride = scaled_page_height(page_scale) + scaled_page_gap(page_scale);
    let viewport_center_y = scroll_offset_y + viewport_height * 0.5;
    let page_index = (viewport_center_y / page_stride).floor().max(0.0) as usize;
    (page_index + 1).clamp(1, total_pages.max(1))
}

fn scroll_cursor_into_view(ui: &mut Ui, state: &RichTextBoxState, layout: &LaidOutDocument) {
    let cursor_top = layout
        .cursor_points
        .get(state.cursor_index)
        .copied()
        .unwrap_or(Pos2::ZERO);
    let cursor_rect = Rect::from_min_size(
        cursor_top,
        egui::vec2(
            2.0,
            rendered_font_size(state.typing_style.font_size) * state.page_scale * 1.35,
        ),
    )
    .expand2(egui::vec2(CURSOR_SCROLL_MARGIN_X, CURSOR_SCROLL_MARGIN_Y));

    ui.scroll_to_rect(cursor_rect, None);
}

#[cfg(test)]
mod tests {
    use super::{
        is_synthetic_paragraph_separator, line_alignment_adjustments, paragraph_layout_spec,
        InlineStyle, LayoutOptions, ParagraphAlignment, ParagraphLayoutSpec, ParagraphStyle,
        PendingGlyph, RichTextBoxState, StyledChar, EMBEDDED_IMAGE_OBJECT_CHAR, PT_TO_PX,
    };
    use eframe::egui::Color32;

    #[test]
    fn insert_text_uses_cursor_and_advances_it() {
        let mut state = RichTextBoxState::new("abc");
        state.cursor_index = 1;

        state.insert_text("XY");

        assert_eq!(state.plain_text(), "aXYbc");
        assert_eq!(state.cursor_index, 3);
    }

    #[test]
    fn backspace_removes_char_before_cursor() {
        let mut state = RichTextBoxState::new("abc");
        state.cursor_index = 2;

        state.backspace();

        assert_eq!(state.plain_text(), "ac");
        assert_eq!(state.cursor_index, 1);
    }

    #[test]
    fn home_and_end_move_within_current_line() {
        let mut state = RichTextBoxState::new("ab\ncd\nef");
        state.cursor_index = 4;

        state.move_to_line_start();
        assert_eq!(state.cursor_index, 3);

        state.move_to_line_end();
        assert_eq!(state.cursor_index, 5);
    }

    #[test]
    fn typing_replaces_selected_text() {
        let mut state = RichTextBoxState::new("abcdef");
        state.selection_anchor = Some(2);
        state.selection_focus = Some(5);
        state.cursor_index = 5;

        state.insert_text("X");

        assert_eq!(state.plain_text(), "abXf");
        assert_eq!(state.cursor_index, 3);
        assert_eq!(state.selected_range(), None);
    }

    #[test]
    fn insert_embedded_image_places_image_object_at_cursor() {
        let mut state = RichTextBoxState::new("abcd");
        state.cursor_index = 2;

        state
            .insert_embedded_image("panels_figs/sample_image.png")
            .expect("sample image should load");

        assert_eq!(state.chars[2].value, EMBEDDED_IMAGE_OBJECT_CHAR);
        assert_eq!(state.images.len(), 1);
        assert_eq!(state.selected_image_index(), Some(0));
        assert_eq!(state.cursor_index, 3);
        assert_eq!(state.plain_text(), "abcd");
    }

    #[test]
    fn delete_forward_removes_an_empty_bullet_line() {
        let list_paragraph_style = ParagraphStyle {
            list_style_name: Some("L1".to_owned()),
            list_marker: Some('•'),
            ..ParagraphStyle::default()
        };
        let body_paragraph_style = ParagraphStyle::default();
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('•', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\n', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\n', InlineStyle::default(), list_paragraph_style),
            StyledChar::new('N', InlineStyle::default(), body_paragraph_style.clone()),
            StyledChar::new('e', InlineStyle::default(), body_paragraph_style.clone()),
            StyledChar::new('x', InlineStyle::default(), body_paragraph_style.clone()),
            StyledChar::new('t', InlineStyle::default(), body_paragraph_style),
        ]);
        state.cursor_index = 3;

        state.delete_forward();

        assert_eq!(state.plain_text(), "Next");
        assert_eq!(state.cursor_index, 0);
        assert_eq!(state.chars[0].paragraph_style.list_marker, None);
    }

    #[test]
    fn enter_at_end_of_bullet_line_creates_next_bullet_item() {
        let list_paragraph_style = ParagraphStyle {
            list_style_name: Some("L1".to_owned()),
            list_marker: Some('•'),
            ..ParagraphStyle::default()
        };
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('•', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('I', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('e', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('m', InlineStyle::default(), list_paragraph_style),
        ]);
        state.cursor_index = state.chars.len();

        state.insert_char('\n');

        assert_eq!(state.plain_text(), "\t•\tItem\n\t•\t");
        assert_eq!(state.cursor_index, state.chars.len());
        assert_eq!(
            state
                .chars
                .last()
                .and_then(|entry| entry.paragraph_style.list_marker),
            Some('•')
        );

        state.delete_forward();

        assert_eq!(state.plain_text(), "\t•\tItem\n");
        assert_eq!(state.cursor_index, 8);
        assert_eq!(state.chars[7].paragraph_style.list_marker, None);

        state.insert_text("Plain");
        state.insert_char('\n');

        assert_eq!(state.plain_text(), "\t•\tItem\nPlain\n");
    }

    #[test]
    fn enter_at_end_of_numbered_line_creates_incremented_next_item() {
        let list_paragraph_style = ParagraphStyle {
            list_style_name: Some("L2".to_owned()),
            list_marker: Some('.'),
            list_number: Some(3),
            ..ParagraphStyle::default()
        };
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('3', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('.', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('I', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('e', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('m', InlineStyle::default(), list_paragraph_style),
        ]);
        state.cursor_index = state.chars.len();

        state.insert_char('\n');

        assert_eq!(state.plain_text(), "\t3.\tItem\n\t4.\t");
        assert_eq!(state.cursor_index, state.chars.len());
        assert_eq!(
            state
                .chars
                .last()
                .and_then(|entry| entry.paragraph_style.list_number),
            Some(4)
        );
    }

    #[test]
    fn enter_at_end_of_empty_bullet_line_terminates_the_list() {
        let list_paragraph_style = ParagraphStyle {
            list_style_name: Some("L1".to_owned()),
            list_marker: Some('•'),
            ..ParagraphStyle::default()
        };
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('•', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style),
        ]);
        state.cursor_index = state.chars.len();

        state.insert_char('\n');
        state.insert_text("Plain");

        assert_eq!(state.plain_text(), "\nPlain");
        assert_eq!(state.chars[0].paragraph_style.list_marker, None);
    }

    #[test]
    fn enter_at_end_of_empty_numbered_line_terminates_the_list() {
        let list_paragraph_style = ParagraphStyle {
            list_style_name: Some("L2".to_owned()),
            list_marker: Some('.'),
            list_number: Some(7),
            ..ParagraphStyle::default()
        };
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('7', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('.', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style),
        ]);
        state.cursor_index = state.chars.len();

        state.insert_char('\n');
        state.insert_text("Plain");

        assert_eq!(state.plain_text(), "\nPlain");
        assert_eq!(state.chars[0].paragraph_style.list_number, None);
        assert_eq!(state.chars[0].paragraph_style.list_marker, None);
    }

    #[test]
    fn toggling_bullets_adds_prefix_to_current_plain_line() {
        let mut state = RichTextBoxState::new("Alpha\nBeta");
        state.cursor_index = 2;

        state.toggle_bullet_list();

        assert_eq!(state.plain_text(), "\t•\tAlpha\nBeta");
        assert!(state.active_bullet_list());
        assert_eq!(state.cursor_index, 5);
        assert_eq!(state.chars[0].paragraph_style.list_marker, Some('•'));
        assert_eq!(state.chars[4].paragraph_style.list_marker, Some('•'));
        assert_eq!(state.chars[9].paragraph_style.list_marker, None);
    }

    #[test]
    fn toggling_bullets_removes_prefix_from_current_bullet_line() {
        let list_paragraph_style = ParagraphStyle {
            list_style_name: Some("LroBulletList".to_owned()),
            list_marker: Some('•'),
            ..ParagraphStyle::default()
        };
        let body_paragraph_style = ParagraphStyle::default();
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('•', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('A', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('l', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('p', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('h', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('a', InlineStyle::default(), list_paragraph_style),
            StyledChar::new('\n', InlineStyle::default(), body_paragraph_style),
        ]);
        state.cursor_index = 5;

        state.toggle_bullet_list();

        assert_eq!(state.plain_text(), "Alpha\n");
        assert!(!state.active_bullet_list());
        assert_eq!(state.cursor_index, 2);
        assert!(state.chars[..5]
            .iter()
            .all(|entry| entry.paragraph_style.list_marker.is_none()));
    }

    #[test]
    fn toggling_bullets_converts_current_numbered_line_to_bullet() {
        let list_paragraph_style = ParagraphStyle {
            list_style_name: Some("L2".to_owned()),
            list_marker: Some('.'),
            list_number: Some(12),
            ..ParagraphStyle::default()
        };
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('1', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('2', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('.', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('I', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('e', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('m', InlineStyle::default(), list_paragraph_style),
        ]);
        state.cursor_index = 7;

        state.toggle_bullet_list();

        assert_eq!(state.plain_text(), "\t•\tItem");
        assert!(state.active_bullet_list());
        assert_eq!(state.cursor_index, 5);
        assert_eq!(state.chars[0].paragraph_style.list_marker, Some('•'));
        assert_eq!(state.chars[0].paragraph_style.list_number, None);
    }

    #[test]
    fn toggling_numbered_list_adds_prefix_to_current_plain_line() {
        let mut state = RichTextBoxState::new("Alpha\nBeta");
        state.cursor_index = 2;

        state.toggle_numbered_list();

        assert_eq!(state.plain_text(), "\t1.\tAlpha\nBeta");
        assert!(state.active_numbered_list());
        assert_eq!(state.cursor_index, 6);
        assert_eq!(state.chars[1].paragraph_style.list_marker, Some('.'));
        assert_eq!(state.chars[1].paragraph_style.list_number, Some(1));
        assert_eq!(state.chars[10].paragraph_style.list_marker, None);
    }

    #[test]
    fn toggling_numbered_list_removes_prefix_from_current_numbered_line() {
        let list_paragraph_style = ParagraphStyle {
            list_style_name: Some("LroNumberedList".to_owned()),
            list_marker: Some('.'),
            list_number: Some(1),
            ..ParagraphStyle::default()
        };
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('1', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('.', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('A', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('l', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('p', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('h', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('a', InlineStyle::default(), list_paragraph_style),
        ]);
        state.cursor_index = 6;

        state.toggle_numbered_list();

        assert_eq!(state.plain_text(), "Alpha");
        assert!(!state.active_numbered_list());
        assert_eq!(state.cursor_index, 2);
        assert!(state
            .chars
            .iter()
            .all(|entry| entry.paragraph_style.list_marker.is_none()));
    }

    #[test]
    fn toggling_numbered_list_converts_current_bullet_line_to_numbered() {
        let list_paragraph_style = ParagraphStyle {
            list_style_name: Some("LroBulletList".to_owned()),
            list_marker: Some('•'),
            ..ParagraphStyle::default()
        };
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('•', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('I', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('t', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('e', InlineStyle::default(), list_paragraph_style.clone()),
            StyledChar::new('m', InlineStyle::default(), list_paragraph_style),
        ]);
        state.cursor_index = 5;

        state.toggle_numbered_list();

        assert_eq!(state.plain_text(), "\t1.\tItem");
        assert!(state.active_numbered_list());
        assert_eq!(state.cursor_index, 6);
        assert_eq!(state.chars[0].paragraph_style.list_marker, Some('.'));
        assert_eq!(state.chars[0].paragraph_style.list_number, Some(1));
    }

    #[test]
    fn toggling_style_updates_selected_chars() {
        let mut state = RichTextBoxState::new("abcdef");
        state.selection_anchor = Some(1);
        state.selection_focus = Some(4);

        state.toggle_bold();
        state.increase_font_size();

        assert!(!state.chars[0].style.bold);
        assert!(state.chars[1..4].iter().all(|entry| entry.style.bold));
        assert!(!state.chars[4].style.bold);
        assert!(state.chars[1..4]
            .iter()
            .all(|entry| { (entry.style.font_size - (18.0 + PT_TO_PX)).abs() < f32::EPSILON }));
    }

    #[test]
    fn cycling_color_updates_selected_chars() {
        let mut state = RichTextBoxState::new("abcd");
        state.selection_anchor = Some(1);
        state.selection_focus = Some(3);

        state.cycle_text_color();

        assert_eq!(state.chars[0].style.color, Color32::BLACK);
        assert_eq!(state.chars[1].style.color, Color32::RED);
        assert_eq!(state.chars[2].style.color, Color32::RED);
        assert_eq!(state.chars[3].style.color, Color32::BLACK);
        assert_eq!(state.typing_style.color, Color32::RED);
    }

    #[test]
    fn highlight_color_updates_selected_chars() {
        let mut state = RichTextBoxState::new("abcd");
        state.selection_anchor = Some(1);
        state.selection_focus = Some(3);

        state.set_highlight_color(Some(Color32::YELLOW));

        assert_eq!(state.chars[0].style.background_color, None);
        assert_eq!(state.chars[1].style.background_color, Some(Color32::YELLOW));
        assert_eq!(state.chars[2].style.background_color, Some(Color32::YELLOW));
        assert_eq!(state.chars[3].style.background_color, None);
        assert_eq!(state.typing_style.background_color, Some(Color32::YELLOW));

        state.set_highlight_color(None);

        assert_eq!(state.chars[1].style.background_color, None);
        assert_eq!(state.chars[2].style.background_color, None);
        assert_eq!(state.typing_style.background_color, None);
    }

    #[test]
    fn active_font_size_follows_cursor_position_without_selection() {
        let state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new(
                'A',
                InlineStyle {
                    font_size: 12.0,
                    ..Default::default()
                },
                Default::default(),
            ),
            StyledChar::new(
                'B',
                InlineStyle {
                    font_size: 24.0,
                    ..Default::default()
                },
                Default::default(),
            ),
        ]);
        let mut state = state;

        state.cursor_index = 1;
        assert_eq!(state.active_font_size(), 12.0);

        state.cursor_index = 2;
        assert_eq!(state.active_font_size(), 24.0);
    }

    #[test]
    fn paragraph_layout_spec_uses_imported_margins_and_alignment() {
        let entry = StyledChar::new(
            'A',
            InlineStyle::default(),
            ParagraphStyle {
                alignment: ParagraphAlignment::End,
                margin_left: 10.0,
                margin_right: 20.0,
                line_height_percent: Some(130.0),
                ..ParagraphStyle::default()
            },
        );

        let spec = paragraph_layout_spec(&entry, 100.0, 500.0, &LayoutOptions::default());

        assert_eq!(spec.left_x, 110.0);
        assert_eq!(spec.width, 470.0);
        assert_eq!(spec.alignment, ParagraphAlignment::End);
        assert_eq!(spec.line_height_percent, Some(130.0));
    }

    #[test]
    fn active_paragraph_horizontal_margins_update_current_paragraph_only() {
        let first_style = ParagraphStyle {
            margin_left: 4.0,
            margin_right: 6.0,
            ..ParagraphStyle::default()
        };
        let second_style = ParagraphStyle {
            margin_left: 11.0,
            margin_right: 13.0,
            ..ParagraphStyle::default()
        };
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('A', InlineStyle::default(), first_style.clone()),
            StyledChar::new('\n', InlineStyle::default(), first_style),
            StyledChar::new('B', InlineStyle::default(), second_style.clone()),
            StyledChar::new('e', InlineStyle::default(), second_style.clone()),
            StyledChar::new('t', InlineStyle::default(), second_style.clone()),
            StyledChar::new('a', InlineStyle::default(), second_style),
        ]);
        state.cursor_index = 3;

        state.set_active_paragraph_horizontal_margins(24.0, 36.0);

        assert_eq!(state.chars[0].paragraph_style.margin_left, 4.0);
        assert_eq!(state.chars[0].paragraph_style.margin_right, 6.0);
        assert_eq!(state.chars[2].paragraph_style.margin_left, 24.0);
        assert_eq!(state.chars[5].paragraph_style.margin_right, 36.0);
    }

    #[test]
    fn active_paragraph_horizontal_margins_update_selected_paragraphs() {
        let first_style = ParagraphStyle::default();
        let second_style = ParagraphStyle {
            margin_left: 11.0,
            margin_right: 13.0,
            ..ParagraphStyle::default()
        };
        let mut state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('A', InlineStyle::default(), first_style.clone()),
            StyledChar::new('\n', InlineStyle::default(), first_style),
            StyledChar::new('B', InlineStyle::default(), second_style.clone()),
            StyledChar::new('e', InlineStyle::default(), second_style.clone()),
            StyledChar::new('t', InlineStyle::default(), second_style.clone()),
            StyledChar::new('a', InlineStyle::default(), second_style),
        ]);
        state.selection_anchor = Some(0);
        state.selection_focus = Some(4);

        state.set_active_paragraph_horizontal_margins(8.0, 10.0);

        assert!(state
            .chars
            .iter()
            .all(|entry| entry.paragraph_style.margin_left == 8.0));
        assert!(state
            .chars
            .iter()
            .all(|entry| entry.paragraph_style.margin_right == 10.0));
    }

    #[test]
    fn line_alignment_adjustment_right_aligns_remaining_width() {
        let state = RichTextBoxState::new("abc");
        let glyphs = [
            PendingGlyph {
                index: 0,
                x: 0.0,
                width: 10.0,
                height: 10.0,
            },
            PendingGlyph {
                index: 1,
                x: 12.0,
                width: 10.0,
                height: 10.0,
            },
        ];
        let spec = ParagraphLayoutSpec {
            left_x: 0.0,
            width: 50.0,
            alignment: ParagraphAlignment::End,
            line_height_percent: None,
        };

        let (shift, justify_extra) = line_alignment_adjustments(&state, &glyphs, spec, true);

        assert_eq!(shift, 28.0);
        assert_eq!(justify_extra, 0.0);
    }

    #[test]
    fn paragraph_spacing_collapses_only_the_imported_separator_newline() {
        let paragraph_style = ParagraphStyle::default();
        let state = RichTextBoxState::from_styled_chars(vec![
            StyledChar::new('A', InlineStyle::default(), paragraph_style.clone()),
            StyledChar::new('\n', InlineStyle::default(), paragraph_style.clone()),
            StyledChar::new('\n', InlineStyle::default(), paragraph_style.clone()),
            StyledChar::new('\n', InlineStyle::default(), paragraph_style),
        ]);

        assert!(!is_synthetic_paragraph_separator(&state, 1));
        assert!(is_synthetic_paragraph_separator(&state, 2));
        assert!(!is_synthetic_paragraph_separator(&state, 3));
    }
}
