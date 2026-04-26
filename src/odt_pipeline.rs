use crate::rich_textbox::{
    DocumentImage, InlineStyle, ParagraphAlignment, ParagraphKind, ParagraphStyle, StyledChar,
    EMBEDDED_IMAGE_OBJECT_CHAR, SOFT_PAGE_BREAK_CHAR,
};
use eframe::egui::{self, Color32, Vec2};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use std::{
    collections::HashMap,
    error::Error,
    fmt::{Display, Formatter},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    string::FromUtf8Error,
    time::{SystemTime, UNIX_EPOCH},
};

const ODT_MIMETYPE: &str = "application/vnd.oasis.opendocument.text";
const MANIFEST_XML_ENTRY: &str = "META-INF/manifest.xml";
const CONTENT_XML_ENTRY: &str = "content.xml";
const STYLES_XML_ENTRY: &str = "styles.xml";
const PICTURES_DIR: &str = "Pictures";
const EXPORT_PARAGRAPH_STYLE_NAME: &str = "LroBody";
const EXPORT_HEADING_STYLE_NAME: &str = "LroHeading2";
const GENERATED_BULLET_LIST_STYLE_NAME: &str = "LroBulletList";
const GENERATED_NUMBERED_LIST_STYLE_NAME: &str = "LroNumberedList";
const PARAGRAPH_OPEN_TAG: &str = "<text:p";
const PARAGRAPH_CLOSE_TAG: &str = "</text:p>";
const HEADING_OPEN_TAG: &str = "<text:h";
const HEADING_CLOSE_TAG: &str = "</text:h>";
const SPAN_OPEN_TAG: &str = "<text:span";
const SPAN_CLOSE_TAG: &str = "</text:span>";
const DRAW_FRAME_OPEN_TAG: &str = "<draw:frame";
const DRAW_FRAME_CLOSE_TAG: &str = "</draw:frame>";
const DRAW_IMAGE_TAG: &str = "<draw:image";
const LIST_OPEN_TAG: &str = "<text:list";
const LIST_CLOSE_TAG: &str = "</text:list>";
const LIST_STYLE_OPEN_TAG: &str = "<text:list-style";
const LIST_STYLE_CLOSE_TAG: &str = "</text:list-style>";
const LIST_LEVEL_BULLET_TAG: &str = "<text:list-level-style-bullet";
const LIST_LEVEL_NUMBER_TAG: &str = "<text:list-level-style-number";
const LINE_BREAK_TAG: &str = "<text:line-break/>";
const TAB_TAG: &str = "<text:tab/>";
const SOFT_PAGE_BREAK_TAG: &str = "<text:soft-page-break/>";
const STYLE_OPEN_TAG: &str = "<style:style";
const STYLE_CLOSE_TAG: &str = "</style:style>";
const DEFAULT_STYLE_OPEN_TAG: &str = "<style:default-style";
const DEFAULT_STYLE_CLOSE_TAG: &str = "</style:default-style>";
const TEXT_PROPERTIES_TAG: &str = "<style:text-properties";
const PARAGRAPH_PROPERTIES_TAG: &str = "<style:paragraph-properties";
const GRAPHIC_PROPERTIES_TAG: &str = "<style:graphic-properties";
const PT_TO_PX: f32 = 4.0 / 3.0;
const IN_TO_PX: f32 = 96.0;
const CM_TO_PX: f32 = IN_TO_PX / 2.54;
const MM_TO_PX: f32 = CM_TO_PX / 10.0;

#[derive(Debug)]
pub enum OdtLoadError {
    UnzipFailed(String),
    ContentXmlNotUtf8(FromUtf8Error),
    ImageDecodeFailed {
        entry_path: PathBuf,
        source: image::ImageError,
    },
}

impl Display for OdtLoadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnzipFailed(message) => write!(formatter, "{message}"),
            Self::ContentXmlNotUtf8(error) => {
                write!(formatter, "ODT XML is not valid UTF-8: {error}")
            }
            Self::ImageDecodeFailed { entry_path, source } => {
                write!(
                    formatter,
                    "failed to decode {}: {source}",
                    entry_path.display()
                )
            }
        }
    }
}

impl Error for OdtLoadError {}

#[derive(Debug)]
pub enum OdtSaveError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    ImageEncodeFailed {
        path: PathBuf,
        source: image::ImageError,
    },
    ZipFailed(String),
}

impl Display for OdtSaveError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to write {}: {source}", path.display())
            }
            Self::ImageEncodeFailed { path, source } => {
                write!(formatter, "failed to encode {}: {source}", path.display())
            }
            Self::ZipFailed(message) => write!(formatter, "{message}"),
        }
    }
}

impl Error for OdtSaveError {}

#[derive(Debug, Default, Clone)]
struct StyleDefinition {
    parent_name: Option<String>,
    font_size: Option<f32>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    color: Option<Color32>,
    background_color: Option<Option<Color32>>,
    alignment: Option<ParagraphAlignment>,
    margin_left: Option<f32>,
    margin_right: Option<f32>,
    margin_top: Option<f32>,
    margin_bottom: Option<f32>,
    line_height_percent: Option<f32>,
    center_horizontally: Option<bool>,
}

#[derive(Debug, Default)]
struct StyleRegistry {
    default_style: InlineStyle,
    default_paragraph_style: ParagraphStyle,
    default_graphic_style: GraphicStyle,
    styles: HashMap<String, StyleDefinition>,
    list_styles: HashMap<String, ListStyleDefinition>,
    resolved_styles: HashMap<String, InlineStyle>,
    resolved_graphic_styles: HashMap<String, GraphicStyle>,
}

#[derive(Debug, Clone, Copy, Default)]
struct GraphicStyle {
    margin_left: f32,
    margin_right: f32,
    margin_top: f32,
    margin_bottom: f32,
    center_horizontally: bool,
}

#[derive(Debug, Clone, Copy)]
struct ListStyleDefinition {
    marker: char,
    start_number: Option<u32>,
}

#[derive(Debug, Clone)]
struct ActiveListState {
    style_name: String,
    marker: char,
    next_number: Option<u32>,
}

pub struct OdtDocument {
    pub chars: Vec<StyledChar>,
    pub images: Vec<DocumentImage>,
}

#[derive(Debug, Clone, Copy)]
struct PendingFrame {
    size: Option<Vec2>,
    graphic_style: GraphicStyle,
}

pub fn load_document_from_odt(path: impl AsRef<Path>) -> Result<OdtDocument, OdtLoadError> {
    let path = path.as_ref();
    let styles_xml = read_zip_entry(path, STYLES_XML_ENTRY)?;
    let content_xml = read_zip_entry(path, CONTENT_XML_ENTRY)?;
    let mut styles = StyleRegistry::from_xml(&styles_xml, &content_xml);
    extract_document_content(path, &content_xml, &mut styles)
}

pub fn load_styled_text_from_odt(path: impl AsRef<Path>) -> Result<Vec<StyledChar>, OdtLoadError> {
    Ok(load_document_from_odt(path)?.chars)
}

pub fn load_plain_text_from_odt(path: impl AsRef<Path>) -> Result<String, OdtLoadError> {
    Ok(load_styled_text_from_odt(path)?
        .into_iter()
        .map(|entry| entry.value)
        .collect())
}

pub fn save_document_to_odt(
    path: impl AsRef<Path>,
    chars: &[StyledChar],
    images: &[DocumentImage],
) -> Result<(), OdtSaveError> {
    let path = resolve_export_target_path(path.as_ref())?;
    let temp_dir = create_export_temp_dir(&path)?;
    let mimetype_path = temp_dir.join("mimetype");
    let content_xml_path = temp_dir.join(CONTENT_XML_ENTRY);
    let styles_xml_path = temp_dir.join(STYLES_XML_ENTRY);
    let pictures_dir = temp_dir.join(PICTURES_DIR);
    let manifest_dir = temp_dir.join("META-INF");
    let manifest_xml_path = temp_dir.join(MANIFEST_XML_ENTRY);

    write_export_file(&mimetype_path, ODT_MIMETYPE)?;
    write_export_file(&styles_xml_path, export_styles_xml(chars))?;
    write_export_images(&pictures_dir, images)?;
    write_export_file(&content_xml_path, export_content_xml(chars, images))?;
    fs::create_dir_all(&manifest_dir).map_err(|source| OdtSaveError::Io {
        path: manifest_dir.clone(),
        source,
    })?;
    write_export_file(&manifest_xml_path, export_manifest_xml(images))?;

    if path.exists() {
        fs::remove_file(&path).map_err(|source| OdtSaveError::Io {
            path: path.clone(),
            source,
        })?;
    }

    zip_export_package(&temp_dir, &path)?;
    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

impl StyleRegistry {
    fn from_xml(styles_xml: &str, content_xml: &str) -> Self {
        let mut registry = Self {
            default_style: InlineStyle::default(),
            default_paragraph_style: ParagraphStyle::default(),
            default_graphic_style: GraphicStyle::default(),
            styles: HashMap::new(),
            list_styles: HashMap::new(),
            resolved_styles: HashMap::new(),
            resolved_graphic_styles: HashMap::new(),
        };

        registry.read_default_style(styles_xml);
        registry.read_style_definitions(styles_xml);
        registry.read_style_definitions(content_xml);
        registry.read_list_styles(styles_xml);
        registry.read_list_styles(content_xml);
        registry
    }

    fn read_default_style(&mut self, xml: &str) {
        let mut cursor = 0;
        while let Some(style_start) = xml[cursor..].find(DEFAULT_STYLE_OPEN_TAG) {
            let open_start = cursor + style_start;
            let Some(open_end) = xml[open_start..]
                .find('>')
                .map(|offset| open_start + offset + 1)
            else {
                break;
            };
            let open_tag = &xml[open_start..open_end];
            let Some(close_start) = xml[open_end..]
                .find(DEFAULT_STYLE_CLOSE_TAG)
                .map(|offset| open_end + offset)
            else {
                break;
            };
            let body = &xml[open_end..close_start];

            match attribute_value(open_tag, "style:family").as_deref() {
                Some("paragraph") => {
                    apply_text_properties_from_body(&mut self.default_style, body);
                    apply_paragraph_properties_from_body(&mut self.default_paragraph_style, body);
                }
                Some("graphic") => {
                    apply_graphic_properties_from_body(&mut self.default_graphic_style, body)
                }
                _ => {}
            }

            cursor = close_start + DEFAULT_STYLE_CLOSE_TAG.len();
        }
    }

    fn read_style_definitions(&mut self, xml: &str) {
        let mut cursor = 0;
        while let Some(style_start) = xml[cursor..].find(STYLE_OPEN_TAG) {
            let open_start = cursor + style_start;
            let Some(open_end) = xml[open_start..]
                .find('>')
                .map(|offset| open_start + offset + 1)
            else {
                break;
            };
            let open_tag = &xml[open_start..open_end];
            let Some(style_name) = attribute_value(open_tag, "style:name") else {
                cursor = open_end;
                continue;
            };

            let parent_name = attribute_value(open_tag, "style:parent-style-name");

            if open_tag.ends_with("/>") {
                self.styles
                    .entry(style_name)
                    .or_insert_with(|| StyleDefinition {
                        parent_name,
                        ..Default::default()
                    });
                cursor = open_end;
                continue;
            }

            let Some(close_start) = xml[open_end..]
                .find(STYLE_CLOSE_TAG)
                .map(|offset| open_end + offset)
            else {
                break;
            };
            let body = &xml[open_end..close_start];
            let mut definition = StyleDefinition {
                parent_name,
                ..Default::default()
            };
            apply_text_properties_to_definition(&mut definition, body);
            apply_paragraph_properties_to_definition(&mut definition, body);
            apply_graphic_properties_to_definition(&mut definition, body);
            self.styles.insert(style_name, definition);
            cursor = close_start + STYLE_CLOSE_TAG.len();
        }
    }

    fn read_list_styles(&mut self, xml: &str) {
        let mut cursor = 0;
        while let Some(style_start) = xml[cursor..].find(LIST_STYLE_OPEN_TAG) {
            let open_start = cursor + style_start;
            let Some(open_end) = xml[open_start..]
                .find('>')
                .map(|offset| open_start + offset + 1)
            else {
                break;
            };
            let open_tag = &xml[open_start..open_end];
            let Some(style_name) = attribute_value(open_tag, "style:name") else {
                cursor = open_end;
                continue;
            };
            let Some(close_start) = xml[open_end..]
                .find(LIST_STYLE_CLOSE_TAG)
                .map(|offset| open_end + offset)
            else {
                break;
            };
            let body = &xml[open_end..close_start];

            if let Some(list_style) = parse_list_style_definition(body) {
                self.list_styles.insert(style_name, list_style);
            }

            cursor = close_start + LIST_STYLE_CLOSE_TAG.len();
        }
    }

    fn resolve_style(&mut self, style_name: Option<&str>) -> InlineStyle {
        let Some(style_name) = style_name else {
            return self.default_style;
        };
        self.resolve_named_style(style_name)
    }

    fn resolve_named_style(&mut self, style_name: &str) -> InlineStyle {
        if let Some(style) = self.resolved_styles.get(style_name) {
            return *style;
        }

        let definition = self.styles.get(style_name).cloned().unwrap_or_default();
        let mut style = definition
            .parent_name
            .as_deref()
            .map(|parent_name| self.resolve_named_style(parent_name))
            .unwrap_or(self.default_style);

        if let Some(font_size) = definition.font_size {
            style.font_size = font_size;
        }
        if let Some(bold) = definition.bold {
            style.bold = bold;
        }
        if let Some(italic) = definition.italic {
            style.italic = italic;
        }
        if let Some(underline) = definition.underline {
            style.underline = underline;
        }
        if let Some(color) = definition.color {
            style.color = color;
        }
        if let Some(background_color) = definition.background_color {
            style.background_color = background_color;
        }

        self.resolved_styles.insert(style_name.to_owned(), style);
        style
    }

    fn resolve_span_style(&self, style_name: Option<&str>, base_style: InlineStyle) -> InlineStyle {
        let Some(style_name) = style_name else {
            return base_style;
        };

        let mut style = base_style;
        self.apply_named_text_style_over(style_name, &mut style);
        style
    }

    fn apply_named_text_style_over(&self, style_name: &str, style: &mut InlineStyle) {
        let Some(definition) = self.styles.get(style_name) else {
            return;
        };

        if let Some(parent_name) = definition.parent_name.as_deref() {
            self.apply_named_text_style_over(parent_name, style);
        }
        if let Some(font_size) = definition.font_size {
            style.font_size = font_size;
        }
        if let Some(bold) = definition.bold {
            style.bold = bold;
        }
        if let Some(italic) = definition.italic {
            style.italic = italic;
        }
        if let Some(underline) = definition.underline {
            style.underline = underline;
        }
        if let Some(color) = definition.color {
            style.color = color;
        }
        if let Some(background_color) = definition.background_color {
            style.background_color = background_color;
        }
    }

    fn resolve_graphic_style(&mut self, style_name: Option<&str>) -> GraphicStyle {
        let Some(style_name) = style_name else {
            return self.default_graphic_style;
        };
        self.resolve_named_graphic_style(style_name)
    }

    fn resolve_named_graphic_style(&mut self, style_name: &str) -> GraphicStyle {
        if let Some(style) = self.resolved_graphic_styles.get(style_name) {
            return *style;
        }

        let definition = self.styles.get(style_name).cloned().unwrap_or_default();
        let mut style = definition
            .parent_name
            .as_deref()
            .map(|parent_name| self.resolve_named_graphic_style(parent_name))
            .unwrap_or(self.default_graphic_style);

        if let Some(margin_left) = definition.margin_left {
            style.margin_left = margin_left;
        }
        if let Some(margin_right) = definition.margin_right {
            style.margin_right = margin_right;
        }
        if let Some(margin_top) = definition.margin_top {
            style.margin_top = margin_top;
        }
        if let Some(margin_bottom) = definition.margin_bottom {
            style.margin_bottom = margin_bottom;
        }
        if let Some(center_horizontally) = definition.center_horizontally {
            style.center_horizontally = center_horizontally;
        }

        self.resolved_graphic_styles
            .insert(style_name.to_owned(), style);
        style
    }

    fn resolve_paragraph_style(
        &mut self,
        style_name: Option<&str>,
        kind: ParagraphKind,
    ) -> ParagraphStyle {
        let mut style = style_name
            .map(|style_name| self.resolve_named_paragraph_style(style_name))
            .unwrap_or_else(|| self.default_paragraph_style.clone());
        style.kind = kind;
        if style_name.is_none() {
            style.style_name = match kind {
                ParagraphKind::Body => EXPORT_PARAGRAPH_STYLE_NAME.to_owned(),
                ParagraphKind::Heading { .. } => EXPORT_HEADING_STYLE_NAME.to_owned(),
            };
        }
        style
    }

    fn resolve_named_paragraph_style(&mut self, style_name: &str) -> ParagraphStyle {
        let definition = self.styles.get(style_name).cloned().unwrap_or_default();
        let mut style = definition
            .parent_name
            .as_deref()
            .map(|parent_name| self.resolve_named_paragraph_style(parent_name))
            .unwrap_or_else(|| self.default_paragraph_style.clone());

        style.style_name = style_name.to_owned();
        if let Some(alignment) = definition.alignment {
            style.alignment = alignment;
        }
        if let Some(margin_left) = definition.margin_left {
            style.margin_left = margin_left;
        }
        if let Some(margin_right) = definition.margin_right {
            style.margin_right = margin_right;
        }
        if let Some(margin_top) = definition.margin_top {
            style.margin_top = margin_top;
        }
        if let Some(margin_bottom) = definition.margin_bottom {
            style.margin_bottom = margin_bottom;
        }
        if let Some(line_height_percent) = definition.line_height_percent {
            style.line_height_percent = Some(line_height_percent);
        }

        style
    }
}

fn read_zip_entry(path: &Path, entry_name: &str) -> Result<String, OdtLoadError> {
    let output = Command::new("unzip")
        .arg("-p")
        .arg(path)
        .arg(entry_name)
        .output()
        .map_err(|error| {
            OdtLoadError::UnzipFailed(format!(
                "failed to execute unzip for {}: {error}",
                path.display()
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!(
                "unzip could not extract {entry_name} from {}",
                path.display()
            )
        } else {
            format!(
                "unzip could not extract {entry_name} from {}: {stderr}",
                path.display()
            )
        };
        return Err(OdtLoadError::UnzipFailed(message));
    }

    String::from_utf8(output.stdout).map_err(OdtLoadError::ContentXmlNotUtf8)
}

fn read_zip_entry_bytes(path: &Path, entry_name: &str) -> Result<Vec<u8>, OdtLoadError> {
    let output = Command::new("unzip")
        .arg("-p")
        .arg(path)
        .arg(entry_name)
        .output()
        .map_err(|error| {
            OdtLoadError::UnzipFailed(format!(
                "failed to execute unzip for {}: {error}",
                path.display()
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!(
                "unzip could not extract {entry_name} from {}",
                path.display()
            )
        } else {
            format!(
                "unzip could not extract {entry_name} from {}: {stderr}",
                path.display()
            )
        };
        return Err(OdtLoadError::UnzipFailed(message));
    }

    Ok(output.stdout)
}

fn extract_document_content(
    odt_path: &Path,
    content_xml: &str,
    styles: &mut StyleRegistry,
) -> Result<OdtDocument, OdtLoadError> {
    let mut chars = Vec::new();
    let mut images = Vec::new();
    let mut cursor = 0;
    let mut in_text_block = false;
    let mut current_block_has_content = false;
    let mut current_block_style = styles.default_style;
    let mut current_paragraph_style = styles.default_paragraph_style.clone();
    let mut last_closed_block_had_content = false;
    let mut pending_frame: Option<PendingFrame> = None;
    let mut list_stack: Vec<ActiveListState> = Vec::new();
    let mut style_stack = vec![styles.default_style];

    while let Some(relative_start) = content_xml[cursor..].find('<') {
        let tag_start = cursor + relative_start;
        if in_text_block {
            current_block_has_content |= append_decoded_text(
                &mut chars,
                &content_xml[cursor..tag_start],
                *style_stack.last().unwrap_or(&current_block_style),
                current_paragraph_style.clone(),
            );
        }

        let Some(relative_end) = content_xml[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_end + 1;
        let tag = &content_xml[tag_start..tag_end];

        if tag.starts_with(LIST_OPEN_TAG) && !tag.starts_with(LIST_STYLE_OPEN_TAG) {
            let list_style_name = attribute_value(tag, "text:style-name");
            if let Some((style_name, list_style)) =
                list_style_name.as_deref().and_then(|style_name| {
                    styles
                        .list_styles
                        .get(style_name)
                        .copied()
                        .map(|list_style| (style_name.to_owned(), list_style))
                })
            {
                list_stack.push(ActiveListState {
                    style_name,
                    marker: list_style.marker,
                    next_number: list_style.start_number,
                });
            }
        } else if tag == LIST_CLOSE_TAG {
            if !list_stack.is_empty() {
                list_stack.pop();
            }
        } else if tag.starts_with(PARAGRAPH_OPEN_TAG) || tag.starts_with(HEADING_OPEN_TAG) {
            if !chars.is_empty() && chars.last().is_some_and(|entry| entry.value != '\n') {
                chars.push(StyledChar::new(
                    '\n',
                    current_block_style,
                    current_paragraph_style.clone(),
                ));
            }
            let block_kind = if tag.starts_with(HEADING_OPEN_TAG) {
                ParagraphKind::Heading {
                    outline_level: attribute_value(tag, "text:outline-level")
                        .and_then(|value| value.parse::<u8>().ok())
                        .unwrap_or(2),
                }
            } else {
                ParagraphKind::Body
            };
            current_block_style =
                styles.resolve_style(attribute_value(tag, "text:style-name").as_deref());
            current_paragraph_style = styles.resolve_paragraph_style(
                attribute_value(tag, "text:style-name").as_deref(),
                block_kind,
            );
            if let Some(list_state) = list_stack.last_mut() {
                current_paragraph_style.list_style_name = Some(list_state.style_name.clone());
                current_paragraph_style.list_marker = Some(list_state.marker);
                current_paragraph_style.list_number = list_state.next_number;
                if let Some(next_number) = list_state.next_number.as_mut() {
                    *next_number = next_number.saturating_add(1);
                }
            }
            style_stack.clear();
            style_stack.push(current_block_style);
            current_block_has_content = false;

            if let Some(list_marker) = current_paragraph_style.list_marker {
                chars.push(StyledChar::new(
                    '\t',
                    current_block_style,
                    current_paragraph_style.clone(),
                ));
                if let Some(list_number) = current_paragraph_style.list_number {
                    for digit in list_number.to_string().chars() {
                        chars.push(StyledChar::new(
                            digit,
                            current_block_style,
                            current_paragraph_style.clone(),
                        ));
                    }
                }
                chars.push(StyledChar::new(
                    list_marker,
                    current_block_style,
                    current_paragraph_style.clone(),
                ));
                chars.push(StyledChar::new(
                    '\t',
                    current_block_style,
                    current_paragraph_style.clone(),
                ));
                current_block_has_content = true;
            }

            if tag.ends_with("/>") {
                chars.push(StyledChar::new(
                    '\n',
                    current_block_style,
                    current_paragraph_style.clone(),
                ));
                last_closed_block_had_content = false;
                in_text_block = false;
                style_stack.clear();
                style_stack.push(styles.default_style);
                current_paragraph_style = styles.default_paragraph_style.clone();
            } else {
                in_text_block = true;
            }
        } else if tag == PARAGRAPH_CLOSE_TAG || tag == HEADING_CLOSE_TAG {
            chars.push(StyledChar::new(
                '\n',
                current_block_style,
                current_paragraph_style.clone(),
            ));
            if current_block_has_content && current_paragraph_style.list_marker.is_none() {
                chars.push(StyledChar::new(
                    '\n',
                    current_block_style,
                    current_paragraph_style.clone(),
                ));
            }
            last_closed_block_had_content = current_block_has_content;
            in_text_block = false;
            current_block_has_content = false;
            style_stack.clear();
            style_stack.push(styles.default_style);
            current_paragraph_style = styles.default_paragraph_style.clone();
        } else if in_text_block && tag.starts_with(SPAN_OPEN_TAG) {
            let parent_style = *style_stack.last().unwrap_or(&current_block_style);
            let style = styles.resolve_span_style(
                attribute_value(tag, "text:style-name").as_deref(),
                parent_style,
            );
            style_stack.push(style);
        } else if in_text_block && tag == SPAN_CLOSE_TAG {
            if style_stack.len() > 1 {
                style_stack.pop();
            }
        } else if in_text_block && tag.starts_with(DRAW_FRAME_OPEN_TAG) {
            pending_frame = Some(PendingFrame {
                size: parse_frame_size(tag),
                graphic_style: styles
                    .resolve_graphic_style(attribute_value(tag, "draw:style-name").as_deref()),
            });
        } else if in_text_block && tag.starts_with(DRAW_IMAGE_TAG) {
            if let Some(href) = attribute_value(tag, "xlink:href") {
                let graphic_style = pending_frame
                    .map(|frame| frame.graphic_style)
                    .unwrap_or_default();
                let image = load_document_image(
                    odt_path,
                    &href,
                    pending_frame.and_then(|frame| frame.size),
                    graphic_style,
                )?;
                images.push(image);
                chars.push(StyledChar::new(
                    EMBEDDED_IMAGE_OBJECT_CHAR,
                    current_block_style,
                    current_paragraph_style.clone(),
                ));
                current_block_has_content = true;
            }
        } else if in_text_block && tag == DRAW_FRAME_CLOSE_TAG {
            pending_frame = None;
        } else if in_text_block && tag == LINE_BREAK_TAG {
            chars.push(StyledChar::new(
                '\n',
                *style_stack.last().unwrap_or(&current_block_style),
                current_paragraph_style.clone(),
            ));
            current_block_has_content = true;
        } else if in_text_block && tag == TAB_TAG {
            chars.push(StyledChar::new(
                '\t',
                *style_stack.last().unwrap_or(&current_block_style),
                current_paragraph_style.clone(),
            ));
            current_block_has_content = true;
        } else if in_text_block && tag == SOFT_PAGE_BREAK_TAG {
            chars.push(StyledChar::new(
                SOFT_PAGE_BREAK_CHAR,
                current_block_style,
                current_paragraph_style.clone(),
            ));
        }

        cursor = tag_end;
    }

    if last_closed_block_had_content {
        for _ in 0..2 {
            if chars.last().is_some_and(|entry| entry.value == '\n') {
                chars.pop();
            }
        }
    }
    Ok(OdtDocument { chars, images })
}

fn normalize_tab_prefixed_list_metadata_for_export(chars: &mut [StyledChar]) {
    let mut line_start = 0;
    while line_start < chars.len() {
        let mut line_end = line_start;
        while line_end < chars.len() && chars[line_end].value != '\n' {
            line_end += 1;
        }

        if chars[line_start..line_end]
            .iter()
            .all(|entry| entry.paragraph_style.list_marker.is_none())
        {
            if let Some(list_style) = detect_tab_prefixed_list_style(&chars[line_start..line_end]) {
                for entry in &mut chars[line_start..line_end] {
                    entry.paragraph_style.list_style_name = list_style.list_style_name.clone();
                    entry.paragraph_style.list_marker = list_style.list_marker;
                    entry.paragraph_style.list_number = list_style.list_number;
                }
                if let Some(entry) = chars.get_mut(line_end).filter(|entry| entry.value == '\n') {
                    entry.paragraph_style.list_style_name = list_style.list_style_name;
                    entry.paragraph_style.list_marker = list_style.list_marker;
                    entry.paragraph_style.list_number = list_style.list_number;
                }
            }
        }

        line_start = line_end.saturating_add(1);
    }
}

fn detect_tab_prefixed_list_style(line_chars: &[StyledChar]) -> Option<ParagraphStyle> {
    if line_chars.get(0)?.value != '\t' {
        return None;
    }

    let base_style = line_chars.get(0)?.paragraph_style.clone();
    if line_chars.get(1)?.value == '•' && line_chars.get(2)?.value == '\t' {
        let mut list_style = base_style;
        list_style.list_style_name = Some(GENERATED_BULLET_LIST_STYLE_NAME.to_owned());
        list_style.list_marker = Some('•');
        list_style.list_number = None;
        return Some(list_style);
    }

    let mut marker_index = 1;
    while line_chars
        .get(marker_index)
        .is_some_and(|entry| entry.value.is_ascii_digit())
    {
        marker_index += 1;
    }
    if marker_index == 1 || line_chars.get(marker_index + 1)?.value != '\t' {
        return None;
    }

    let list_number = line_chars[1..marker_index]
        .iter()
        .map(|entry| entry.value)
        .collect::<String>()
        .parse::<u32>()
        .ok()?;
    let mut list_style = base_style;
    list_style.list_style_name = Some(GENERATED_NUMBERED_LIST_STYLE_NAME.to_owned());
    list_style.list_marker = Some(line_chars.get(marker_index)?.value);
    list_style.list_number = Some(list_number);
    Some(list_style)
}

fn append_decoded_text(
    chars: &mut Vec<StyledChar>,
    raw_text: &str,
    style: InlineStyle,
    paragraph_style: ParagraphStyle,
) -> bool {
    if raw_text.is_empty() {
        return false;
    }

    let decoded = decode_xml_entities(raw_text);
    let has_content = !decoded.is_empty();
    chars.extend(
        decoded
            .chars()
            .map(|value| StyledChar::new(value, style, paragraph_style.clone())),
    );
    has_content
}

fn decode_xml_entities(raw_text: &str) -> String {
    raw_text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn apply_text_properties_from_body(style: &mut InlineStyle, body: &str) {
    if let Some(tag) = first_text_properties_tag(body) {
        if let Some(font_size) = parse_font_size(tag) {
            style.font_size = font_size;
        }
        if let Some(bold) = parse_font_weight(tag) {
            style.bold = bold;
        }
        if let Some(italic) = parse_font_style(tag) {
            style.italic = italic;
        }
        if let Some(underline) = parse_underline(tag) {
            style.underline = underline;
        }
        if let Some(color) = parse_color(tag) {
            style.color = color;
        }
        if let Some(background_color) = parse_background_color(tag) {
            style.background_color = background_color;
        }
    }
}

fn apply_text_properties_to_definition(definition: &mut StyleDefinition, body: &str) {
    if let Some(tag) = first_text_properties_tag(body) {
        definition.font_size = parse_font_size(tag);
        definition.bold = parse_font_weight(tag);
        definition.italic = parse_font_style(tag);
        definition.underline = parse_underline(tag);
        definition.color = parse_color(tag);
        definition.background_color = parse_background_color(tag);
    }
}

fn apply_paragraph_properties_from_body(style: &mut ParagraphStyle, body: &str) {
    if let Some(tag) = first_paragraph_properties_tag(body) {
        if let Some(alignment) = parse_paragraph_alignment(tag) {
            style.alignment = alignment;
        }
        if let Some(margin_left) =
            parse_length_to_px(attribute_value(tag, "fo:margin-left").as_deref())
        {
            style.margin_left = margin_left;
        }
        if let Some(margin_right) =
            parse_length_to_px(attribute_value(tag, "fo:margin-right").as_deref())
        {
            style.margin_right = margin_right;
        }
        if let Some(margin_top) =
            parse_length_to_px(attribute_value(tag, "fo:margin-top").as_deref())
        {
            style.margin_top = margin_top;
        }
        if let Some(margin_bottom) =
            parse_length_to_px(attribute_value(tag, "fo:margin-bottom").as_deref())
        {
            style.margin_bottom = margin_bottom;
        }
        if let Some(line_height_percent) = parse_line_height_percent(tag) {
            style.line_height_percent = Some(line_height_percent);
        }
    }
}

fn apply_paragraph_properties_to_definition(definition: &mut StyleDefinition, body: &str) {
    if let Some(tag) = first_paragraph_properties_tag(body) {
        definition.alignment = parse_paragraph_alignment(tag);
        definition.margin_left =
            parse_length_to_px(attribute_value(tag, "fo:margin-left").as_deref());
        definition.margin_right =
            parse_length_to_px(attribute_value(tag, "fo:margin-right").as_deref());
        definition.margin_top =
            parse_length_to_px(attribute_value(tag, "fo:margin-top").as_deref());
        definition.margin_bottom =
            parse_length_to_px(attribute_value(tag, "fo:margin-bottom").as_deref());
        definition.line_height_percent = parse_line_height_percent(tag);
    }
}

fn apply_graphic_properties_from_body(style: &mut GraphicStyle, body: &str) {
    if let Some(tag) = first_graphic_properties_tag(body) {
        if let Some(margin_left) =
            parse_length_to_px(attribute_value(tag, "fo:margin-left").as_deref())
        {
            style.margin_left = margin_left;
        }
        if let Some(margin_right) =
            parse_length_to_px(attribute_value(tag, "fo:margin-right").as_deref())
        {
            style.margin_right = margin_right;
        }
        if let Some(margin_top) =
            parse_length_to_px(attribute_value(tag, "fo:margin-top").as_deref())
        {
            style.margin_top = margin_top;
        }
        if let Some(margin_bottom) =
            parse_length_to_px(attribute_value(tag, "fo:margin-bottom").as_deref())
        {
            style.margin_bottom = margin_bottom;
        }
        if let Some(horizontal_pos) = attribute_value(tag, "style:horizontal-pos") {
            style.center_horizontally = horizontal_pos == "center";
        }
    }
}

fn apply_graphic_properties_to_definition(definition: &mut StyleDefinition, body: &str) {
    if let Some(tag) = first_graphic_properties_tag(body) {
        definition.margin_left =
            parse_length_to_px(attribute_value(tag, "fo:margin-left").as_deref());
        definition.margin_right =
            parse_length_to_px(attribute_value(tag, "fo:margin-right").as_deref());
        definition.margin_top =
            parse_length_to_px(attribute_value(tag, "fo:margin-top").as_deref());
        definition.margin_bottom =
            parse_length_to_px(attribute_value(tag, "fo:margin-bottom").as_deref());
        definition.center_horizontally =
            attribute_value(tag, "style:horizontal-pos").map(|value| value == "center");
    }
}

fn first_text_properties_tag(body: &str) -> Option<&str> {
    let start = body.find(TEXT_PROPERTIES_TAG)?;
    let end = body[start..].find('>')?;
    Some(&body[start..start + end + 1])
}

fn first_paragraph_properties_tag(body: &str) -> Option<&str> {
    let start = body.find(PARAGRAPH_PROPERTIES_TAG)?;
    let end = body[start..].find('>')?;
    Some(&body[start..start + end + 1])
}

fn first_graphic_properties_tag(body: &str) -> Option<&str> {
    let start = body.find(GRAPHIC_PROPERTIES_TAG)?;
    let end = body[start..].find('>')?;
    Some(&body[start..start + end + 1])
}

fn parse_paragraph_alignment(tag: &str) -> Option<ParagraphAlignment> {
    match attribute_value(tag, "fo:text-align")?.as_str() {
        "center" => Some(ParagraphAlignment::Center),
        "end" | "right" => Some(ParagraphAlignment::End),
        "justify" => Some(ParagraphAlignment::Justify),
        _ => Some(ParagraphAlignment::Start),
    }
}

fn parse_line_height_percent(tag: &str) -> Option<f32> {
    attribute_value(tag, "fo:line-height")?
        .strip_suffix('%')?
        .parse::<f32>()
        .ok()
}

fn parse_font_size(tag: &str) -> Option<f32> {
    let value = attribute_value(tag, "fo:font-size")?;
    let pt_size = value.strip_suffix("pt")?.parse::<f32>().ok()?;
    Some(pt_size * PT_TO_PX)
}

fn parse_font_weight(tag: &str) -> Option<bool> {
    Some(attribute_value(tag, "fo:font-weight")? == "bold")
}

fn parse_font_style(tag: &str) -> Option<bool> {
    Some(attribute_value(tag, "fo:font-style")? == "italic")
}

fn parse_underline(tag: &str) -> Option<bool> {
    attribute_value(tag, "style:text-underline-style")
        .map(|value| value != "none")
        .or_else(|| attribute_value(tag, "text:style-name").map(|_| false))
}

fn parse_list_style_definition(list_style_body: &str) -> Option<ListStyleDefinition> {
    parse_list_bullet_style(list_style_body).or_else(|| parse_list_number_style(list_style_body))
}

fn parse_list_bullet_style(list_style_body: &str) -> Option<ListStyleDefinition> {
    let tag_start = list_style_body.find(LIST_LEVEL_BULLET_TAG)?;
    let tag_end = list_style_body[tag_start..].find('>')?;
    let tag = &list_style_body[tag_start..tag_start + tag_end + 1];
    let marker = attribute_value(tag, "text:bullet-char")?.chars().next()?;
    Some(ListStyleDefinition {
        marker,
        start_number: None,
    })
}

fn parse_list_number_style(list_style_body: &str) -> Option<ListStyleDefinition> {
    let tag_start = list_style_body.find(LIST_LEVEL_NUMBER_TAG)?;
    let tag_end = list_style_body[tag_start..].find('>')?;
    let tag = &list_style_body[tag_start..tag_start + tag_end + 1];
    let marker = attribute_value(tag, "style:num-suffix")
        .and_then(|suffix| suffix.chars().last())
        .unwrap_or('.');
    let start_number = attribute_value(tag, "text:start-value")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1);
    Some(ListStyleDefinition {
        marker,
        start_number: Some(start_number),
    })
}

fn parse_color(tag: &str) -> Option<Color32> {
    let value = attribute_value(tag, "fo:color")?;
    parse_hex_color(&value)
}

fn parse_background_color(tag: &str) -> Option<Option<Color32>> {
    let value = attribute_value(tag, "fo:background-color")?;
    if value == "transparent" || value == "none" {
        return Some(None);
    }
    parse_hex_color(&value).map(Some)
}

fn parse_hex_color(value: &str) -> Option<Color32> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let rgb = u32::from_str_radix(hex, 16).ok()?;
    Some(Color32::from_rgb(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    ))
}

fn parse_frame_size(tag: &str) -> Option<Vec2> {
    let width = parse_length_to_px(attribute_value(tag, "svg:width").as_deref())?;
    let height = parse_length_to_px(attribute_value(tag, "svg:height").as_deref())?;
    Some(egui::vec2(width, height))
}

fn parse_length_to_px(value: Option<&str>) -> Option<f32> {
    let value = value?;
    if let Some(raw) = value.strip_suffix("in") {
        return Some(raw.parse::<f32>().ok()? * IN_TO_PX);
    }
    if let Some(raw) = value.strip_suffix("cm") {
        return Some(raw.parse::<f32>().ok()? * CM_TO_PX);
    }
    if let Some(raw) = value.strip_suffix("mm") {
        return Some(raw.parse::<f32>().ok()? * MM_TO_PX);
    }
    if let Some(raw) = value.strip_suffix("pt") {
        return Some(raw.parse::<f32>().ok()? * PT_TO_PX);
    }
    value.strip_suffix("px")?.parse::<f32>().ok()
}

fn load_document_image(
    odt_path: &Path,
    href: &str,
    requested_size: Option<Vec2>,
    graphic_style: GraphicStyle,
) -> Result<DocumentImage, OdtLoadError> {
    let image_path = PathBuf::from(href);
    let image_bytes = read_zip_entry_bytes(odt_path, href)?;

    DocumentImage::from_encoded_bytes(
        image_path.clone(),
        &image_bytes,
        requested_size,
        graphic_style.margin_left,
        graphic_style.margin_right,
        graphic_style.margin_top,
        graphic_style.margin_bottom,
        graphic_style.center_horizontally,
    )
    .map_err(|source| OdtLoadError::ImageDecodeFailed {
        entry_path: image_path,
        source,
    })
}

fn attribute_value(tag: &str, attribute_name: &str) -> Option<String> {
    let pattern = format!("{attribute_name}=\"");
    let value_start = tag.find(&pattern)? + pattern.len();
    let value_end = tag[value_start..].find('"')?;
    Some(decode_xml_entities(
        &tag[value_start..value_start + value_end],
    ))
}

fn create_export_temp_dir(target_path: &Path) -> Result<PathBuf, OdtSaveError> {
    let export_root = std::env::temp_dir();
    let file_stem = target_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("liberustoffice");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let temp_dir = export_root.join(format!(
        "liberustoffice_export_{file_stem}_{}_{}",
        std::process::id(),
        timestamp
    ));

    fs::create_dir_all(&temp_dir).map_err(|source| OdtSaveError::Io {
        path: temp_dir.clone(),
        source,
    })?;
    Ok(temp_dir)
}

fn write_export_file(path: &Path, content: impl AsRef<[u8]>) -> Result<(), OdtSaveError> {
    fs::write(path, content).map_err(|source| OdtSaveError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_export_images(pictures_dir: &Path, images: &[DocumentImage]) -> Result<(), OdtSaveError> {
    fs::create_dir_all(pictures_dir).map_err(|source| OdtSaveError::Io {
        path: pictures_dir.to_path_buf(),
        source,
    })?;

    for (image_index, image) in images.iter().enumerate() {
        let image_path = pictures_dir.join(export_image_file_name(image_index));
        let encoded = encode_document_image_png(image)?;
        write_export_file(&image_path, encoded)?;
    }

    Ok(())
}

fn encode_document_image_png(image: &DocumentImage) -> Result<Vec<u8>, OdtSaveError> {
    let mut rgba_bytes = Vec::with_capacity(image.color_image.pixels.len() * 4);
    for pixel in &image.color_image.pixels {
        rgba_bytes.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b(), pixel.a()]);
    }

    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(
            &rgba_bytes,
            image.color_image.size[0] as u32,
            image.color_image.size[1] as u32,
            ColorType::Rgba8,
        )
        .map_err(|source| OdtSaveError::ImageEncodeFailed {
            path: image.path.clone(),
            source,
        })?;
    Ok(encoded)
}

fn zip_export_package(temp_dir: &Path, target_path: &Path) -> Result<(), OdtSaveError> {
    if let Some(parent_dir) = target_path.parent() {
        fs::create_dir_all(parent_dir).map_err(|source| OdtSaveError::Io {
            path: parent_dir.to_path_buf(),
            source,
        })?;
    }

    let mimetype_output = Command::new("zip")
        .current_dir(temp_dir)
        .args(["-X0q"])
        .arg(target_path)
        .arg("mimetype")
        .output()
        .map_err(|source| OdtSaveError::ZipFailed(format!("failed to execute zip: {source}")))?;
    if !mimetype_output.status.success() {
        return Err(OdtSaveError::ZipFailed(zip_failure_message(
            "could not write mimetype entry",
            &mimetype_output.stderr,
        )));
    }

    let payload_output = Command::new("zip")
        .current_dir(temp_dir)
        .args(["-Xqr"])
        .arg(target_path)
        .arg(CONTENT_XML_ENTRY)
        .arg(STYLES_XML_ENTRY)
        .arg(PICTURES_DIR)
        .arg("META-INF")
        .output()
        .map_err(|source| OdtSaveError::ZipFailed(format!("failed to execute zip: {source}")))?;
    if !payload_output.status.success() {
        return Err(OdtSaveError::ZipFailed(zip_failure_message(
            "could not write ODT payload entries",
            &payload_output.stderr,
        )));
    }

    Ok(())
}

fn zip_failure_message(prefix: &str, stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr).trim().to_owned();
    if detail.is_empty() {
        prefix.to_owned()
    } else {
        format!("{prefix}: {detail}")
    }
}

fn export_manifest_xml(images: &[DocumentImage]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2">
  <manifest:file-entry manifest:full-path="/" manifest:version="1.2" manifest:media-type="application/vnd.oasis.opendocument.text"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>
"#,
    );

    for image_index in 0..images.len() {
        xml.push_str(&format!(
            "  <manifest:file-entry manifest:full-path=\"{}\" manifest:media-type=\"image/png\"/>\n",
            export_image_entry_path(image_index)
        ));
    }

    xml.push_str("</manifest:manifest>\n");
    xml
}

fn export_styles_xml(chars: &[StyledChar]) -> String {
    let mut export_chars = chars.to_vec();
    normalize_tab_prefixed_list_metadata_for_export(&mut export_chars);

    let mut xml = String::from(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
  office:version="1.2">
  <office:styles>
    <style:default-style style:family="graphic">
      <style:graphic-properties fo:wrap-option="no-wrap" style:vertical-pos="top" style:vertical-rel="paragraph" style:horizontal-pos="center" style:horizontal-rel="paragraph" style:flow-with-text="false"/>
    </style:default-style>
    <style:default-style style:family="paragraph">
      <style:text-properties fo:font-size="13.5pt" fo:color="#000000"/>
    </style:default-style>
"##,
    );

    for paragraph_style in collect_export_paragraph_styles(&export_chars) {
        xml.push_str(&format!(
            "    <style:style style:name=\"{}\" style:family=\"paragraph\">\n      <style:paragraph-properties fo:text-align=\"{}\" fo:margin-top=\"{}\" fo:margin-bottom=\"{}\" fo:margin-left=\"{}\" fo:margin-right=\"{}\" fo:line-height=\"{:.0}%\"/>\n    </style:style>\n",
            encode_xml_text(&paragraph_style.style_name),
            export_paragraph_alignment(paragraph_style.alignment),
            export_length_inches(paragraph_style.margin_top),
            export_length_inches(paragraph_style.margin_bottom),
            export_length_inches(paragraph_style.margin_left),
            export_length_inches(paragraph_style.margin_right),
            paragraph_style.line_height_percent.unwrap_or(115.0),
        ));
    }

    for list_style in collect_export_list_styles(&export_chars) {
        let style_name = list_style
            .list_style_name
            .as_deref()
            .unwrap_or(EXPORT_PARAGRAPH_STYLE_NAME);
        if let Some(list_number) = list_style.list_number {
            xml.push_str(&format!(
                "    <text:list-style style:name=\"{}\"><text:list-level-style-number text:level=\"1\" text:start-value=\"{}\" style:num-suffix=\"{}\"/></text:list-style>\n",
                encode_xml_text(style_name),
                list_number,
                encode_xml_text(&list_style.list_marker.unwrap_or('.').to_string()),
            ));
        } else if let Some(list_marker) = list_style.list_marker {
            xml.push_str(&format!(
                "    <text:list-style style:name=\"{}\"><text:list-level-style-bullet text:level=\"1\" text:bullet-char=\"{}\"/></text:list-style>\n",
                encode_xml_text(style_name),
                encode_xml_text(&list_marker.to_string()),
            ));
        }
    }

    xml.push_str(
        r##"  </office:styles>
</office:document-styles>
"##,
    );
    xml
}

fn collect_export_paragraph_styles(chars: &[StyledChar]) -> Vec<ParagraphStyle> {
    let mut styles = Vec::new();
    for entry in chars {
        let mut paragraph_style = entry.paragraph_style.clone();
        paragraph_style.list_style_name = None;
        paragraph_style.list_marker = None;
        paragraph_style.list_number = None;

        if !styles.iter().any(|style| *style == paragraph_style) {
            styles.push(paragraph_style);
        }
    }

    if styles.is_empty() {
        styles.push(ParagraphStyle::default());
    }

    let mut line_start = 0;
    while line_start < chars.len() {
        let mut line_end = line_start;
        while line_end < chars.len() && chars[line_end].value != '\n' {
            line_end += 1;
        }

        let line_style = chars
            .get(line_start..line_end)
            .and_then(|line_chars| line_chars.first())
            .map(|entry| entry.paragraph_style.clone());
        let next_line_style = chars
            .get(line_end + 1)
            .map(|entry| entry.paragraph_style.clone());
        if let Some(line_style) = line_style {
            if line_style.list_marker.is_some() {
                let list_item_style = paragraph_style_for_generated_list_item(line_style);
                if !styles.iter().any(|style| *style == list_item_style) {
                    styles.push(list_item_style);
                }
            } else if next_line_style
                .as_ref()
                .is_some_and(|style| style.list_marker.is_some())
            {
                let before_list_style = paragraph_style_before_generated_list(line_style);
                if !styles.iter().any(|style| *style == before_list_style) {
                    styles.push(before_list_style);
                }
            }
        }

        line_start = line_end.saturating_add(1);
    }

    styles
}

fn collect_export_list_styles(chars: &[StyledChar]) -> Vec<ParagraphStyle> {
    let mut styles = Vec::new();
    for entry in chars {
        if entry.paragraph_style.list_marker.is_none() {
            continue;
        }
        if !styles.iter().any(|style: &ParagraphStyle| {
            style.list_style_name == entry.paragraph_style.list_style_name
                && style.list_marker == entry.paragraph_style.list_marker
                && style.list_number.is_some() == entry.paragraph_style.list_number.is_some()
        }) {
            styles.push(entry.paragraph_style.clone());
        }
    }
    styles
}

fn export_paragraph_alignment(alignment: ParagraphAlignment) -> &'static str {
    match alignment {
        ParagraphAlignment::Start => "start",
        ParagraphAlignment::Center => "center",
        ParagraphAlignment::End => "end",
        ParagraphAlignment::Justify => "justify",
    }
}

fn export_content_xml(chars: &[StyledChar], images: &[DocumentImage]) -> String {
    let mut export_chars = chars.to_vec();
    normalize_tab_prefixed_list_metadata_for_export(&mut export_chars);

    let mut style_runs = ExportStyleRegistry::default();
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
  xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
  xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0"
  xmlns:xlink="http://www.w3.org/1999/xlink"
  office:version="1.2">
  <office:automatic-styles>
"#,
    );

    style_runs.collect_styles(&export_chars);
    xml.push_str(&style_runs.as_xml());
    xml.push_str(&export_image_styles_xml(images));
    xml.push_str(
        "  </office:automatic-styles>\n  <office:body>\n    <office:text text:use-soft-page-breaks=\"true\">\n",
    );

    let mut open_list_style_name: Option<String> = None;
    for paragraph in export_paragraphs(&export_chars, images, &style_runs) {
        let paragraph_list_style_name = paragraph.style.list_style_name.clone();
        if paragraph_list_style_name.is_some() && export_list_item_is_empty(&paragraph) {
            if open_list_style_name.is_some() {
                xml.push_str("      </text:list>\n");
                open_list_style_name = None;
            }
            xml.push_str("      ");
            xml.push_str(&export_paragraph_xml(&ExportParagraph::empty(
                String::new(),
                plain_paragraph_style_after_list_item(paragraph.style),
            )));
            xml.push('\n');
            continue;
        }

        if open_list_style_name != paragraph_list_style_name {
            if open_list_style_name.is_some() {
                xml.push_str("      </text:list>\n");
            }
            if let Some(list_style_name) = &paragraph_list_style_name {
                xml.push_str(&format!(
                    "      <text:list text:style-name=\"{}\">\n",
                    encode_xml_text(list_style_name)
                ));
            }
            open_list_style_name = paragraph_list_style_name.clone();
        }

        if paragraph_list_style_name.is_some() {
            xml.push_str("        <text:list-item>");
            xml.push_str(&export_paragraph_xml(&paragraph));
            xml.push_str("</text:list-item>\n");
        } else {
            xml.push_str("      ");
            xml.push_str(&export_paragraph_xml(&paragraph));
            xml.push('\n');
        }
    }
    if open_list_style_name.is_some() {
        xml.push_str("      </text:list>\n");
    }

    xml.push_str("    </office:text>\n  </office:body>\n</office:document-content>\n");
    xml
}

fn export_paragraph_xml(paragraph: &ExportParagraph) -> String {
    match paragraph.style.kind {
        ParagraphKind::Body => format!(
            "<text:p text:style-name=\"{}\">{}</text:p>",
            encode_xml_text(&paragraph.style.style_name),
            paragraph.xml,
        ),
        ParagraphKind::Heading { outline_level } => format!(
            "<text:h text:style-name=\"{}\" text:outline-level=\"{}\">{}</text:h>",
            encode_xml_text(&paragraph.style.style_name),
            outline_level.max(1),
            paragraph.xml,
        ),
    }
}

fn export_list_item_is_empty(paragraph: &ExportParagraph) -> bool {
    let mut payload = paragraph.xml.trim();
    while let Some(remaining_payload) = payload.strip_prefix("<text:tab/>") {
        payload = remaining_payload.trim_start();
    }
    payload.is_empty()
}

fn plain_paragraph_style_after_list_item(mut style: ParagraphStyle) -> ParagraphStyle {
    while let Some(base_name) = style.style_name.strip_suffix("LroListItem") {
        style.style_name = base_name.to_owned();
    }
    style.list_style_name = None;
    style.list_marker = None;
    style.list_number = None;
    if style.margin_bottom == 0.0 {
        style.margin_bottom = ParagraphStyle::default().margin_bottom;
    }
    style
}

fn export_paragraphs(
    chars: &[StyledChar],
    images: &[DocumentImage],
    styles: &ExportStyleRegistry,
) -> Vec<ExportParagraph> {
    let mut paragraphs = Vec::new();
    let mut paragraph_xml = String::new();
    let mut paragraph_chars = Vec::new();
    let mut run_text = String::new();
    let mut run_style: Option<InlineStyle> = None;
    let mut index = 0;
    let mut image_index = 0;
    let mut paragraph_has_content = false;
    let mut current_paragraph_style = chars
        .first()
        .map(|entry| entry.paragraph_style.clone())
        .unwrap_or_else(ParagraphStyle::default);

    while index < chars.len() {
        if !paragraph_has_content {
            if let Some(prefix_len) =
                export_list_prefix_len(&chars[index..], &current_paragraph_style)
            {
                index += prefix_len;
                continue;
            }
        }

        let entry = &chars[index];
        if entry.value == '\n' {
            let mut newline_count = 1;
            while index + newline_count < chars.len() && chars[index + newline_count].value == '\n'
            {
                newline_count += 1;
            }

            flush_export_run(&mut paragraph_xml, &mut run_text, &mut run_style, styles);
            let next_paragraph_style = chars
                .get(index + newline_count)
                .map(|entry| entry.paragraph_style.clone());
            let entering_list_block = paragraph_has_content
                && newline_count == 1
                && current_paragraph_style.list_marker.is_none()
                && next_paragraph_style
                    .as_ref()
                    .is_some_and(|style| style.list_marker.is_some());
            if paragraph_has_content
                && newline_count == 1
                && current_paragraph_style.list_marker.is_none()
                && next_paragraph_style
                    .as_ref()
                    .is_none_or(|style| style.list_marker.is_none())
            {
                paragraph_xml.push_str("<text:line-break/>");
                paragraph_chars.push(entry.clone());
            } else {
                let current_is_list_block = current_paragraph_style.list_marker.is_some();
                let paragraph_style = if entering_list_block {
                    paragraph_style_before_generated_list(current_paragraph_style.clone())
                } else if current_is_list_block {
                    paragraph_style_for_generated_list_item(current_paragraph_style.clone())
                } else {
                    current_paragraph_style.clone()
                };
                if paragraph_has_content || !current_is_list_block {
                    paragraphs.push(if entering_list_block || current_is_list_block {
                        ExportParagraph::empty(std::mem::take(&mut paragraph_xml), paragraph_style)
                    } else {
                        ExportParagraph::from_content(
                            std::mem::take(&mut paragraph_xml),
                            &paragraph_chars,
                            paragraph_style,
                        )
                    });
                } else {
                    paragraph_xml.clear();
                }
                paragraph_chars.clear();
                let empty_paragraph_start =
                    if paragraph_has_content && current_paragraph_style.list_marker.is_none() {
                        2
                    } else {
                        1
                    };
                for offset in empty_paragraph_start..newline_count {
                    paragraphs.push(ExportParagraph::empty(
                        String::new(),
                        chars[index + offset].paragraph_style.clone(),
                    ));
                }
                paragraph_has_content = false;
                if let Some(next_style) = next_paragraph_style {
                    current_paragraph_style = next_style;
                } else {
                    current_paragraph_style = entry.paragraph_style.clone();
                }
            }

            index += newline_count;
            continue;
        }

        if entry.value == SOFT_PAGE_BREAK_CHAR {
            flush_export_run(&mut paragraph_xml, &mut run_text, &mut run_style, styles);
            paragraph_xml.push_str(SOFT_PAGE_BREAK_TAG);
            paragraph_chars.push(entry.clone());
            index += 1;
            continue;
        }

        if entry.value == EMBEDDED_IMAGE_OBJECT_CHAR {
            flush_export_run(&mut paragraph_xml, &mut run_text, &mut run_style, styles);
            if let Some(image) = images.get(image_index) {
                paragraph_xml.push_str(&export_image_frame_xml(image, image_index));
                paragraph_chars.push(entry.clone());
                paragraph_has_content = true;
            }
            image_index += 1;
            index += 1;
            continue;
        }

        if entry.value == '\t' {
            flush_export_run(&mut paragraph_xml, &mut run_text, &mut run_style, styles);
            paragraph_xml.push_str("<text:tab/>");
            paragraph_chars.push(entry.clone());
            paragraph_has_content = true;
            index += 1;
            continue;
        }

        if run_style.is_some_and(|style| style != entry.style) {
            flush_export_run(&mut paragraph_xml, &mut run_text, &mut run_style, styles);
        }

        run_style.get_or_insert(entry.style);
        run_text.push(entry.value);
        paragraph_chars.push(entry.clone());
        paragraph_has_content = true;
        index += 1;
    }

    flush_export_run(&mut paragraph_xml, &mut run_text, &mut run_style, styles);
    if paragraph_has_content || paragraphs.is_empty() {
        let paragraph_style = if current_paragraph_style.list_marker.is_some() {
            paragraph_style_for_generated_list_item(current_paragraph_style)
        } else {
            current_paragraph_style
        };
        paragraphs.push(if paragraph_style.list_marker.is_some() {
            ExportParagraph::empty(paragraph_xml, paragraph_style)
        } else {
            ExportParagraph::from_content(paragraph_xml, &paragraph_chars, paragraph_style)
        });
    }

    if paragraphs.is_empty() {
        paragraphs.push(ExportParagraph::empty(
            String::new(),
            ParagraphStyle::default(),
        ));
    }

    paragraphs
}

fn paragraph_style_before_generated_list(mut style: ParagraphStyle) -> ParagraphStyle {
    style.style_name = format!("{}LroBeforeList", style.style_name);
    style.margin_bottom = 0.0;
    style
}

fn paragraph_style_for_generated_list_item(mut style: ParagraphStyle) -> ParagraphStyle {
    style.style_name = format!("{}LroListItem", style.style_name);
    style.margin_top = 0.0;
    style.margin_bottom = 0.0;
    style
}

fn export_list_prefix_len(chars: &[StyledChar], paragraph_style: &ParagraphStyle) -> Option<usize> {
    let mut index = 0;
    if paragraph_style.list_marker.is_none() || chars.get(index)?.value != '\t' {
        return None;
    }
    index += 1;

    if paragraph_style.list_number.is_some() {
        let digits_start = index;
        while chars
            .get(index)
            .is_some_and(|entry| entry.value.is_ascii_digit())
        {
            index += 1;
        }
        if index == digits_start {
            return None;
        }
    }

    if chars.get(index)?.value != paragraph_style.list_marker? {
        return None;
    }
    index += 1;

    if chars.get(index)?.value != '\t' {
        return None;
    }

    Some(index + 1)
}

#[derive(Debug, Clone)]
struct ExportParagraph {
    style: ParagraphStyle,
    xml: String,
}

impl ExportParagraph {
    fn empty(xml: String, style: ParagraphStyle) -> Self {
        Self { style, xml }
    }

    fn from_content(xml: String, chars: &[StyledChar], fallback_style: ParagraphStyle) -> Self {
        Self {
            style: chars
                .first()
                .map(|entry| entry.paragraph_style.clone())
                .unwrap_or(fallback_style),
            xml,
        }
    }
}

fn export_image_styles_xml(images: &[DocumentImage]) -> String {
    let mut xml = String::new();
    for (image_index, image) in images.iter().enumerate() {
        xml.push_str(&format!(
            "    <style:style style:name=\"{}\" style:family=\"graphic\"><style:graphic-properties fo:wrap-option=\"no-wrap\" style:wrap=\"dynamic\" style:number-wrapped-paragraphs=\"no-limit\" style:vertical-pos=\"top\" style:vertical-rel=\"paragraph\" style:horizontal-pos=\"{}\" style:horizontal-rel=\"paragraph\" fo:margin-left=\"{}\" fo:margin-right=\"{}\" fo:margin-top=\"{}\" fo:margin-bottom=\"{}\"/></style:style>\n",
            export_image_style_name(image_index),
            if image.center_horizontally { "center" } else { "from-left" },
            export_length_inches(image.margin_left),
            export_length_inches(image.margin_right),
            export_length_inches(image.margin_top),
            export_length_inches(image.margin_bottom),
        ));
    }
    xml
}

fn export_image_frame_xml(image: &DocumentImage, image_index: usize) -> String {
    format!(
        "<draw:frame draw:style-name=\"{}\" text:anchor-type=\"char\" svg:width=\"{}\" svg:height=\"{}\"><draw:image xlink:href=\"{}\" xlink:type=\"simple\" xlink:show=\"embed\" xlink:actuate=\"onLoad\"/></draw:frame>",
        export_image_style_name(image_index),
        export_length_inches(image.size.x),
        export_length_inches(image.size.y),
        export_image_entry_path(image_index),
    )
}

fn export_image_entry_path(image_index: usize) -> String {
    format!("{PICTURES_DIR}/{}", export_image_file_name(image_index))
}

fn export_image_style_name(image_index: usize) -> String {
    format!("G{}", image_index + 1)
}

fn export_image_file_name(image_index: usize) -> String {
    format!("liberustoffice-image-{}.png", image_index + 1)
}

fn export_length_inches(length_px: f32) -> String {
    format!("{:.4}in", length_px.max(0.0) / IN_TO_PX)
}

fn flush_export_run(
    paragraph_xml: &mut String,
    run_text: &mut String,
    run_style: &mut Option<InlineStyle>,
    styles: &ExportStyleRegistry,
) {
    if run_text.is_empty() {
        *run_style = None;
        return;
    }

    let escaped = encode_xml_text(run_text);
    if let Some(style) = run_style.and_then(|style| styles.style_name(style)) {
        paragraph_xml.push_str(&format!(
            "<text:span text:style-name=\"{style}\">{escaped}</text:span>"
        ));
    } else {
        paragraph_xml.push_str(&escaped);
    }

    run_text.clear();
    *run_style = None;
}

fn encode_xml_text(raw_text: &str) -> String {
    raw_text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[derive(Default)]
struct ExportStyleRegistry {
    styles: Vec<(InlineStyle, String)>,
}

impl ExportStyleRegistry {
    fn collect_styles(&mut self, chars: &[StyledChar]) {
        for entry in chars {
            if entry.value == '\n'
                || entry.value == '\t'
                || entry.value == EMBEDDED_IMAGE_OBJECT_CHAR
                || entry.value == SOFT_PAGE_BREAK_CHAR
            {
                continue;
            }

            if entry.style == InlineStyle::default() {
                continue;
            }

            if self.style_name(entry.style).is_none() {
                let style_name = format!("T{}", self.styles.len() + 1);
                self.styles.push((entry.style, style_name));
            }
        }
    }

    fn style_name(&self, style: InlineStyle) -> Option<&str> {
        self.styles
            .iter()
            .find_map(|(candidate, name)| (*candidate == style).then_some(name.as_str()))
    }

    fn as_xml(&self) -> String {
        let mut xml = String::new();
        for (style, name) in &self.styles {
            let background_attribute = style
                .background_color
                .map(|color| format!(" fo:background-color=\"#{}\"", style_color_hex(color)))
                .unwrap_or_default();
            xml.push_str(&format!(
                "    <style:style style:name=\"{name}\" style:family=\"text\"><style:text-properties fo:font-size=\"{:.2}pt\" fo:font-weight=\"{}\" fo:font-style=\"{}\" style:text-underline-style=\"{}\" fo:color=\"#{}\"{background_attribute}/></style:style>\n",
                style.font_size / PT_TO_PX,
                if style.bold { "bold" } else { "normal" },
                if style.italic { "italic" } else { "normal" },
                if style.underline { "solid" } else { "none" },
                style_color_hex(style.color),
            ));
        }
        xml
    }
}

fn style_color_hex(color: Color32) -> String {
    format!("{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
}

#[cfg(test)]
mod tests {
    use super::{
        export_content_xml, export_manifest_xml, export_styles_xml, extract_document_content,
        load_document_from_odt, read_zip_entry, save_document_to_odt, StyleRegistry,
        CONTENT_XML_ENTRY,
    };
    use crate::rich_textbox::{
        InlineStyle, ParagraphAlignment, ParagraphKind, ParagraphStyle, RichTextBoxState,
        StyledChar, EMBEDDED_IMAGE_OBJECT_CHAR, SOFT_PAGE_BREAK_CHAR,
    };
    use eframe::egui::Color32;
    use std::fs;
    use std::path::Path;

    #[test]
    fn extracts_text_and_ignores_image_only_paragraphs() {
        let content = r#"
            <office:document-content>
              <office:body>
                  <office:text>
                    <text:h text:outline-level="2">Title</text:h>
                    <text:p><text:span>Alpha</text:span> beta</text:p>
                  <text:p><draw:frame svg:width="5.3335in" svg:height="3.4835in"><draw:image xlink:href="Pictures/100000000000064000000415424CE288.jpg"/></draw:frame></text:p>
                  <text:p><text:soft-page-break/>After break</text:p>
                </office:text>
              </office:body>
            </office:document-content>
        "#;
        let mut styles = StyleRegistry {
            default_style: InlineStyle::default(),
            ..Default::default()
        };

        let text = extract_document_content(
            Path::new("sample_docs/sample_text_base.odt"),
            content,
            &mut styles,
        )
        .expect("content should parse")
        .chars
        .into_iter()
        .map(|entry| entry.value)
        .collect::<String>();

        assert_eq!(
            text,
            format!("Title\n\nAlpha beta\n\n{EMBEDDED_IMAGE_OBJECT_CHAR}\n\n{SOFT_PAGE_BREAK_CHAR}After break")
        );
    }

    #[test]
    fn parses_paragraph_and_span_styles() {
        let styles_xml = r##"
            <office:document-styles>
              <office:styles>
                <style:default-style style:family="paragraph">
                  <style:text-properties fo:font-size="12pt"/>
                </style:default-style>
                <style:style style:name="Heading" style:family="paragraph">
                  <style:text-properties fo:font-size="16pt" fo:font-weight="bold"/>
                </style:style>
                <style:style style:name="Strong_20_Emphasis" style:family="text">
                  <style:text-properties fo:font-weight="bold" style:text-underline-style="solid" fo:color="#000080"/>
                </style:style>
                <style:style style:name="RsidOnly" style:family="text">
                  <style:text-properties officeooo:rsid="00140aac"/>
                </style:style>
              </office:styles>
            </office:document-styles>
        "##;
        let content_xml = r#"
            <office:document-content>
              <office:automatic-styles>
                <style:style style:name="P2" style:family="paragraph" style:parent-style-name="Heading"/>
              </office:automatic-styles>
              <office:body>
                <office:text>
                  <text:h text:style-name="P2">Title</text:h>
                  <text:h text:style-name="P2"><text:span text:style-name="RsidOnly">Inherited</text:span>?</text:h>
                  <text:p><text:span text:style-name="Strong_20_Emphasis">Bold</text:span></text:p>
                </office:text>
              </office:body>
            </office:document-content>
        "#;
        let mut styles = StyleRegistry::from_xml(styles_xml, content_xml);
        let chars = extract_document_content(
            Path::new("sample_docs/sample_text_base.odt"),
            content_xml,
            &mut styles,
        )
        .expect("content should parse")
        .chars;

        assert_eq!(chars[0].style.font_size, 16.0 * 4.0 / 3.0);
        assert!(chars[0].style.bold);
        assert_eq!(
            chars[0].paragraph_style.kind,
            ParagraphKind::Heading { outline_level: 2 }
        );
        assert_eq!(chars[0].paragraph_style.style_name, "P2");

        let inherited_char = chars
            .iter()
            .find(|entry| entry.value == 'I')
            .expect("heading span char should exist");
        assert_eq!(inherited_char.style.font_size, 16.0 * 4.0 / 3.0);
        assert!(inherited_char.style.bold);

        let bold_char = chars
            .iter()
            .find(|entry| entry.value == 'B')
            .expect("bold span char should exist");
        assert!(bold_char.style.bold);
        assert!(bold_char.style.underline);
        assert_eq!(bold_char.style.color.r(), 0x00);
        assert_eq!(bold_char.style.color.g(), 0x00);
        assert_eq!(bold_char.style.color.b(), 0x80);
    }

    #[test]
    fn loads_sample_odt_document_text_with_styles_and_images() {
        let document = load_document_from_odt("sample_docs/sample_text_base.odt")
            .expect("sample ODT should be readable");

        let text = document
            .chars
            .iter()
            .map(|entry| entry.value)
            .collect::<String>();
        assert!(text.contains("LibeRustOffice"));
        assert!(text.contains("•\tItem 1"));
        assert!(document.chars.iter().any(|entry| {
            entry.value == '•'
                && entry.paragraph_style.list_style_name.as_deref() == Some("L1")
                && entry.paragraph_style.list_marker == Some('•')
        }));
        assert!(text.contains(EMBEDDED_IMAGE_OBJECT_CHAR));
        assert_eq!(document.images.len(), 1);
        assert!(document.images[0].size.x > 400.0);
        assert!(document.images[0].size.y > 250.0);
        assert!(document.images[0].center_horizontally);

        let heading_char = document
            .chars
            .iter()
            .find(|entry| matches!(entry.paragraph_style.kind, ParagraphKind::Heading { .. }))
            .expect("heading text should exist");
        assert!(heading_char.style.font_size > InlineStyle::default().font_size);
        assert!(heading_char.style.bold);
        assert!(matches!(
            heading_char.paragraph_style.kind,
            ParagraphKind::Heading { .. }
        ));
    }

    #[test]
    fn parses_numbered_list_prefixes_with_incrementing_numbers() {
        let styles_xml = r#"
            <office:document-styles>
              <office:styles>
                <text:list-style style:name="NumberedList">
                  <text:list-level-style-number text:level="1" text:start-value="3" style:num-suffix="."/>
                </text:list-style>
              </office:styles>
            </office:document-styles>
        "#;
        let content_xml = r#"
            <office:document-content>
              <office:body>
                <office:text>
                  <text:list text:style-name="NumberedList">
                    <text:list-item><text:p>First</text:p></text:list-item>
                    <text:list-item><text:p>Second</text:p></text:list-item>
                  </text:list>
                </office:text>
              </office:body>
            </office:document-content>
        "#;
        let mut styles = StyleRegistry::from_xml(styles_xml, content_xml);

        let chars = extract_document_content(
            Path::new("sample_docs/sample_text_base.odt"),
            content_xml,
            &mut styles,
        )
        .expect("numbered list content should parse")
        .chars;

        let text = chars.iter().map(|entry| entry.value).collect::<String>();
        assert_eq!(text, "\t3.\tFirst\n\t4.\tSecond");
        assert_eq!(
            chars[1].paragraph_style.list_style_name.as_deref(),
            Some("NumberedList")
        );
        assert_eq!(chars[1].paragraph_style.list_marker, Some('.'));
        assert_eq!(chars[1].paragraph_style.list_number, Some(3));

        let second_item_number = chars
            .iter()
            .find(|entry| entry.value == '4')
            .and_then(|entry| entry.paragraph_style.list_number);
        assert_eq!(second_item_number, Some(4));
    }

    #[test]
    fn saves_and_reloads_basic_text_and_inline_styles() {
        let export_path = std::env::temp_dir().join(format!(
            "liberustoffice_export_test_{}.odt",
            std::process::id()
        ));
        let chars = vec![
            StyledChar::new(
                'H',
                InlineStyle {
                    font_size: 24.0,
                    bold: true,
                    italic: true,
                    underline: true,
                    color: Color32::from_rgb(12, 34, 56),
                    background_color: Some(Color32::from_rgb(250, 240, 120)),
                },
                ParagraphStyle::default(),
            ),
            StyledChar::new(
                'i',
                InlineStyle {
                    font_size: 24.0,
                    bold: true,
                    italic: true,
                    underline: true,
                    color: Color32::from_rgb(12, 34, 56),
                    background_color: Some(Color32::from_rgb(250, 240, 120)),
                },
                ParagraphStyle::default(),
            ),
            StyledChar::new('\n', InlineStyle::default(), ParagraphStyle::default()),
            StyledChar::new('B', InlineStyle::default(), ParagraphStyle::default()),
        ];

        save_document_to_odt(&export_path, &chars, &[]).expect("document should save");
        let reloaded = load_document_from_odt(&export_path).expect("saved ODT should reopen");
        let _ = fs::remove_file(&export_path);

        let text = reloaded
            .chars
            .iter()
            .map(|entry| entry.value)
            .collect::<String>();
        assert_eq!(text, "Hi\nB");
        assert_eq!(reloaded.chars[0].style.font_size, 24.0);
        assert!(reloaded.chars[0].style.bold);
        assert!(reloaded.chars[0].style.italic);
        assert!(reloaded.chars[0].style.underline);
        assert_eq!(reloaded.chars[0].style.color, Color32::from_rgb(12, 34, 56));
        assert_eq!(
            reloaded.chars[0].style.background_color,
            Some(Color32::from_rgb(250, 240, 120))
        );
    }

    #[test]
    fn exports_paragraph_spacing_style() {
        let chars = vec![StyledChar::new(
            'A',
            InlineStyle::default(),
            ParagraphStyle::default(),
        )];

        let styles_xml = export_styles_xml(&chars);
        let content_xml = export_content_xml(&chars, &[]);

        assert!(styles_xml.contains("fo:margin-bottom=\"0.0972in\""));
        assert!(styles_xml.contains("style:name=\"LroBody\""));
        assert!(content_xml.contains("<text:p text:style-name=\"LroBody\">A</text:p>"));
    }

    #[test]
    fn preserves_soft_page_break_markers() {
        let chars = vec![
            StyledChar::new('A', InlineStyle::default(), ParagraphStyle::default()),
            StyledChar::new('\n', InlineStyle::default(), ParagraphStyle::default()),
            StyledChar::new('\n', InlineStyle::default(), ParagraphStyle::default()),
            StyledChar::new(
                SOFT_PAGE_BREAK_CHAR,
                InlineStyle::default(),
                ParagraphStyle::default(),
            ),
            StyledChar::new('\n', InlineStyle::default(), ParagraphStyle::default()),
            StyledChar::new('\n', InlineStyle::default(), ParagraphStyle::default()),
            StyledChar::new('B', InlineStyle::default(), ParagraphStyle::default()),
        ];

        let content_xml = export_content_xml(&chars, &[]);
        assert!(content_xml.contains("<text:soft-page-break/>"));
        assert!(content_xml
            .contains("<text:p text:style-name=\"LroBody\"><text:soft-page-break/></text:p>"));
        assert!(content_xml.contains("<text:p text:style-name=\"LroBody\"></text:p>"));
        assert!(content_xml.contains("<text:p text:style-name=\"LroBody\">B</text:p>"));
    }

    #[test]
    fn imports_self_closing_empty_paragraphs_and_keeps_trailing_empties() {
        let content_xml = r#"
            <office:document-content>
              <office:body>
                <office:text>
                  <text:p text:style-name="P1">A</text:p>
                  <text:p text:style-name="P2"/>
                  <text:p text:style-name="P3"/>
                </office:text>
              </office:body>
            </office:document-content>
        "#;
        let mut styles = StyleRegistry::from_xml("", content_xml);

        let document = extract_document_content(
            Path::new("sample_docs/sample_text_base.odt"),
            content_xml,
            &mut styles,
        )
        .expect("synthetic content should parse");

        let text = document
            .chars
            .iter()
            .map(|entry| entry.value)
            .collect::<String>();
        assert_eq!(text, "A\n\n\n\n");

        let exported_xml = export_content_xml(&document.chars, &document.images);
        assert!(exported_xml.contains("<text:p text:style-name=\"P1\">A</text:p>"));
        assert!(exported_xml.contains("<text:p text:style-name=\"P2\"></text:p>"));
        assert!(exported_xml.contains("<text:p text:style-name=\"P3\"></text:p>"));
    }

    #[test]
    fn saves_and_reloads_embedded_images() {
        let source = load_document_from_odt("sample_docs/sample_text_base.odt")
            .expect("sample ODT should be readable");
        let export_path = std::env::temp_dir().join(format!(
            "liberustoffice_export_image_test_{}.odt",
            std::process::id()
        ));
        let chars = vec![
            StyledChar::new('A', InlineStyle::default(), ParagraphStyle::default()),
            StyledChar::new('\n', InlineStyle::default(), ParagraphStyle::default()),
            StyledChar::new(
                EMBEDDED_IMAGE_OBJECT_CHAR,
                InlineStyle::default(),
                ParagraphStyle::default(),
            ),
        ];
        let image = source
            .images
            .first()
            .cloned()
            .expect("sample should include an image");

        save_document_to_odt(&export_path, &chars, &[image]).expect("document should save");
        let reloaded = load_document_from_odt(&export_path).expect("saved ODT should reopen");
        let _ = fs::remove_file(&export_path);

        assert_eq!(reloaded.images.len(), 1);
        assert_eq!(reloaded.images[0].size, source.images[0].size);
        assert!(reloaded.images[0].center_horizontally);
        assert_eq!(
            reloaded.images[0].color_image.size,
            source.images[0].color_image.size
        );
        assert!(reloaded
            .chars
            .iter()
            .any(|entry| entry.value == EMBEDDED_IMAGE_OBJECT_CHAR));

        let manifest_xml = export_manifest_xml(&reloaded.images);
        assert!(manifest_xml.contains("Pictures/liberustoffice-image-1.png"));
        let content_xml = export_content_xml(&reloaded.chars, &reloaded.images);
        assert!(
            content_xml.contains("<draw:image xlink:href=\"Pictures/liberustoffice-image-1.png\"")
        );
    }

    #[test]
    fn preserves_paragraph_style_identity_and_block_kind_on_export() {
        let heading_paragraph = ParagraphStyle {
            kind: ParagraphKind::Heading { outline_level: 2 },
            style_name: "P3".to_owned(),
            alignment: ParagraphAlignment::Center,
            margin_top: 13.344,
            margin_bottom: 8.016,
            margin_left: 4.8,
            margin_right: 2.4,
            line_height_percent: Some(130.0),
            ..ParagraphStyle::default()
        };
        let body_paragraph = ParagraphStyle {
            style_name: "P5".to_owned(),
            alignment: ParagraphAlignment::Justify,
            margin_bottom: 12.0,
            line_height_percent: Some(118.0),
            ..ParagraphStyle::default()
        };
        let chars = vec![
            StyledChar::new(
                'H',
                InlineStyle {
                    font_size: 24.0,
                    bold: true,
                    ..Default::default()
                },
                heading_paragraph.clone(),
            ),
            StyledChar::new('1', InlineStyle::default(), heading_paragraph.clone()),
            StyledChar::new('\n', InlineStyle::default(), heading_paragraph),
            StyledChar::new('\n', InlineStyle::default(), body_paragraph.clone()),
            StyledChar::new('B', InlineStyle::default(), body_paragraph.clone()),
            StyledChar::new('o', InlineStyle::default(), body_paragraph),
        ];

        let styles_xml = export_styles_xml(&chars);
        let content_xml = export_content_xml(&chars, &[]);

        assert!(styles_xml.contains("style:name=\"P3\""));
        assert!(styles_xml.contains("fo:text-align=\"center\""));
        assert!(styles_xml.contains("fo:line-height=\"130%\""));
        assert!(styles_xml.contains("style:name=\"P5\""));
        assert!(styles_xml.contains("fo:text-align=\"justify\""));
        assert!(content_xml.contains("<text:h text:style-name=\"P3\" text:outline-level=\"2\">"));
        assert!(content_xml.contains("<text:p text:style-name=\"P5\">Bo</text:p>"));
    }

    #[test]
    fn exports_and_reloads_bullet_lists_as_list_blocks() {
        let list_style = ParagraphStyle {
            list_style_name: Some("LroBulletList".to_owned()),
            list_marker: Some('•'),
            ..ParagraphStyle::default()
        };
        let export_path = std::env::temp_dir().join(format!(
            "liberustoffice_export_bullet_list_test_{}.odt",
            std::process::id()
        ));
        let chars = vec![
            StyledChar::new('\t', InlineStyle::default(), list_style.clone()),
            StyledChar::new('•', InlineStyle::default(), list_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_style.clone()),
            StyledChar::new('A', InlineStyle::default(), list_style.clone()),
            StyledChar::new('\n', InlineStyle::default(), list_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_style.clone()),
            StyledChar::new('•', InlineStyle::default(), list_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_style.clone()),
            StyledChar::new('B', InlineStyle::default(), list_style),
        ];

        let styles_xml = export_styles_xml(&chars);
        let content_xml = export_content_xml(&chars, &[]);
        assert!(styles_xml.contains("<text:list-style style:name=\"LroBulletList\">"));
        assert!(content_xml.contains("<text:list text:style-name=\"LroBulletList\">"));
        assert!(content_xml.contains("<text:list-item><text:p text:style-name=\"LroBodyLroListItem\">A</text:p></text:list-item>"));
        assert!(!content_xml.contains("<text:tab/>•<text:tab/>"));

        save_document_to_odt(&export_path, &chars, &[]).expect("bullet list document should save");
        let reloaded =
            load_document_from_odt(&export_path).expect("saved bullet list should reopen");
        let _ = fs::remove_file(&export_path);

        let text = reloaded
            .chars
            .iter()
            .map(|entry| entry.value)
            .collect::<String>();
        assert_eq!(text, "\t•\tA\n\t•\tB");
        assert!(reloaded.chars.iter().any(|entry| {
            entry.value == '•'
                && entry.paragraph_style.list_style_name.as_deref() == Some("LroBulletList")
        }));

        let mut state = RichTextBoxState::from_styled_document(reloaded.chars, reloaded.images);
        assert!(state.active_bullet_list());

        state.insert_char('\n');

        assert_eq!(state.plain_text(), "\t•\tA\n\t•\tB\n\t•\t");
        assert!(state.active_bullet_list());
        assert_eq!(
            state
                .chars
                .last()
                .and_then(|entry| entry.paragraph_style.list_style_name.as_deref()),
            Some("LroBulletList")
        );
    }

    #[test]
    fn saves_tab_prefixed_bullet_lines_as_real_text_lists() {
        let paragraph_style = ParagraphStyle {
            style_name: "P14".to_owned(),
            ..ParagraphStyle::default()
        };
        let export_path = std::env::temp_dir().join(format!(
            "liberustoffice_export_plain_bullet_prefix_test_{}.odt",
            std::process::id()
        ));
        let chars = "Sample Bulletpoint:\n\t•\tItem 1\n\t•\tItem 2"
            .chars()
            .map(|value| StyledChar::new(value, InlineStyle::default(), paragraph_style.clone()))
            .collect::<Vec<_>>();

        let content_xml = export_content_xml(&chars, &[]);
        assert!(content_xml
            .contains("<text:p text:style-name=\"P14LroBeforeList\">Sample Bulletpoint:</text:p>"));
        assert!(content_xml.contains("<text:list text:style-name=\"LroBulletList\">"));
        assert!(content_xml.contains("<text:list-item><text:p text:style-name=\"P14LroListItem\">Item 1</text:p></text:list-item>"));
        assert!(content_xml.contains("<text:list-item><text:p text:style-name=\"P14LroListItem\">Item 2</text:p></text:list-item>"));
        assert!(!content_xml.contains(
            "<text:list-item><text:p text:style-name=\"P14LroListItem\"></text:p></text:list-item>"
        ));
        assert!(!content_xml.contains("<text:tab/>•<text:tab/>"));

        save_document_to_odt(&export_path, &chars, &[])
            .expect("plain bullet prefixes should save as list blocks");
        let saved_content_xml = read_zip_entry(&export_path, CONTENT_XML_ENTRY)
            .expect("saved content.xml should be readable");
        let _ = fs::remove_file(&export_path);

        assert!(saved_content_xml.contains("<text:list text:style-name=\"LroBulletList\">"));
        assert!(saved_content_xml
            .contains("<text:list-item><text:p text:style-name=\"P14LroListItem\">Item 1</text:p></text:list-item>"));
        assert!(!saved_content_xml.contains(
            "<text:list-item><text:p text:style-name=\"P14LroListItem\"></text:p></text:list-item>"
        ));
        assert!(!saved_content_xml.contains("<text:tab/>•<text:tab/>"));
    }

    #[test]
    fn skips_trailing_whitespace_only_list_items_on_save() {
        let list_style = ParagraphStyle {
            list_style_name: Some("LroBulletList".to_owned()),
            list_marker: Some('•'),
            ..ParagraphStyle::default()
        };
        let body_style = ParagraphStyle {
            style_name: "P14".to_owned(),
            ..ParagraphStyle::default()
        };
        let chars = vec![
            StyledChar::new('\t', InlineStyle::default(), list_style.clone()),
            StyledChar::new('•', InlineStyle::default(), list_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_style.clone()),
            StyledChar::new('A', InlineStyle::default(), list_style.clone()),
            StyledChar::new('\n', InlineStyle::default(), list_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), list_style),
            StyledChar::new('\n', InlineStyle::default(), body_style.clone()),
            StyledChar::new('B', InlineStyle::default(), body_style),
        ];

        let content_xml = export_content_xml(&chars, &[]);

        assert!(content_xml.contains(
            "<text:list-item><text:p text:style-name=\"LroBodyLroListItem\">A</text:p></text:list-item>"
        ));
        assert!(!content_xml.contains(
            "<text:list-item><text:p text:style-name=\"LroBodyLroListItem\"><text:tab/><text:tab/></text:p></text:list-item>"
        ));
        assert!(content_xml.contains(
            "</text:list>\n      <text:p text:style-name=\"LroBody\"></text:p>\n      <text:p text:style-name=\"P14\">B</text:p>"
        ));
    }

    #[test]
    fn preserves_spacing_around_generated_bullet_lists() {
        let paragraph_style = ParagraphStyle {
            style_name: "P14".to_owned(),
            margin_bottom: 9.3312,
            ..ParagraphStyle::default()
        };
        let chars = "Sample Bulletpoint:\n\t•\tItem 1\n\t•\tItem 2\n\nAfter"
            .chars()
            .map(|value| StyledChar::new(value, InlineStyle::default(), paragraph_style.clone()))
            .collect::<Vec<_>>();

        let styles_xml = export_styles_xml(&chars);
        let content_xml = export_content_xml(&chars, &[]);

        assert!(styles_xml.contains("style:name=\"P14LroBeforeList\""));
        assert!(styles_xml.contains("style:name=\"P14LroListItem\""));
        assert!(styles_xml.contains(
            "<style:paragraph-properties fo:text-align=\"start\" fo:margin-top=\"0.0000in\" fo:margin-bottom=\"0.0000in\""
        ));
        assert!(content_xml
            .contains("<text:p text:style-name=\"P14LroBeforeList\">Sample Bulletpoint:</text:p>"));
        assert!(content_xml.contains("<text:list text:style-name=\"LroBulletList\">"));
        assert!(content_xml.contains(
            "<text:list-item><text:p text:style-name=\"P14LroListItem\">Item 1</text:p></text:list-item>"
        ));
        assert!(content_xml.contains(
            "</text:list>\n      <text:p text:style-name=\"P14\"></text:p>\n      <text:p text:style-name=\"P14\">After</text:p>"
        ));
    }

    #[test]
    fn exports_and_reloads_numbered_lists_as_list_blocks() {
        let first_item_style = ParagraphStyle {
            list_style_name: Some("LroNumberedList".to_owned()),
            list_marker: Some('.'),
            list_number: Some(1),
            ..ParagraphStyle::default()
        };
        let second_item_style = ParagraphStyle {
            list_number: Some(2),
            ..first_item_style.clone()
        };
        let export_path = std::env::temp_dir().join(format!(
            "liberustoffice_export_numbered_list_test_{}.odt",
            std::process::id()
        ));
        let chars = vec![
            StyledChar::new('\t', InlineStyle::default(), first_item_style.clone()),
            StyledChar::new('1', InlineStyle::default(), first_item_style.clone()),
            StyledChar::new('.', InlineStyle::default(), first_item_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), first_item_style.clone()),
            StyledChar::new('A', InlineStyle::default(), first_item_style.clone()),
            StyledChar::new('\n', InlineStyle::default(), first_item_style),
            StyledChar::new('\t', InlineStyle::default(), second_item_style.clone()),
            StyledChar::new('2', InlineStyle::default(), second_item_style.clone()),
            StyledChar::new('.', InlineStyle::default(), second_item_style.clone()),
            StyledChar::new('\t', InlineStyle::default(), second_item_style.clone()),
            StyledChar::new('B', InlineStyle::default(), second_item_style),
        ];

        let styles_xml = export_styles_xml(&chars);
        let content_xml = export_content_xml(&chars, &[]);
        assert!(styles_xml.contains("<text:list-level-style-number text:level=\"1\" text:start-value=\"1\" style:num-suffix=\".\"/>"));
        assert!(content_xml.contains("<text:list text:style-name=\"LroNumberedList\">"));
        assert!(content_xml.contains("<text:list-item><text:p text:style-name=\"LroBodyLroListItem\">B</text:p></text:list-item>"));

        save_document_to_odt(&export_path, &chars, &[])
            .expect("numbered list document should save");
        let reloaded =
            load_document_from_odt(&export_path).expect("saved numbered list should reopen");
        let _ = fs::remove_file(&export_path);

        let text = reloaded
            .chars
            .iter()
            .map(|entry| entry.value)
            .collect::<String>();
        assert_eq!(text, "\t1.\tA\n\t2.\tB");
        assert!(reloaded
            .chars
            .iter()
            .any(|entry| entry.paragraph_style.list_number == Some(2)));
    }
}
fn resolve_export_target_path(path: &Path) -> Result<PathBuf, OdtSaveError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    std::env::current_dir()
        .map(|current_dir| current_dir.join(path))
        .map_err(|source| OdtSaveError::Io {
            path: path.to_path_buf(),
            source,
        })
}
