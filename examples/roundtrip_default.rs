#![allow(dead_code)]

#[path = "../src/odt_pipeline.rs"]
mod odt_pipeline;
#[path = "../src/rich_textbox.rs"]
mod rich_textbox;

use odt_pipeline::{load_document_from_odt, save_document_to_odt_with_page_margins};
use rich_textbox::{DocumentImage, StyledChar};
use std::{fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new("sample_docs/sample_text_base.odt");
    let output_path = Path::new("sample_docs/sample_text_base_test.odt");

    let source = load_document_from_odt(source_path)?;
    save_document_to_odt_with_page_margins(
        output_path,
        &source.chars,
        &source.images,
        source.page_margins,
    )?;
    let saved = load_document_from_odt(output_path)?;

    let source_bytes = fs::read(source_path)?;
    let saved_bytes = fs::read(output_path)?;
    println!("source: {}", source_path.display());
    println!("saved:  {}", output_path.display());
    println!("byte-identical: {}", source_bytes == saved_bytes);
    println!(
        "source bytes: {}, saved bytes: {}",
        source_bytes.len(),
        saved_bytes.len()
    );

    compare_chars(&source.chars, &saved.chars)?;
    compare_images(&source.images, &saved.images)?;
    if (source.page_margins.left_cm - saved.page_margins.left_cm).abs() > 0.01
        || (source.page_margins.right_cm - saved.page_margins.right_cm).abs() > 0.01
        || (source.page_margins.top_cm - saved.page_margins.top_cm).abs() > 0.01
        || (source.page_margins.bottom_cm - saved.page_margins.bottom_cm).abs() > 0.01
    {
        return Err(format!(
            "page margins changed: source={:?}, saved={:?}",
            source.page_margins, saved.page_margins
        )
        .into());
    }

    println!("semantic reload comparison: ok");
    Ok(())
}

fn compare_chars(source: &[StyledChar], saved: &[StyledChar]) -> Result<(), String> {
    if source.len() != saved.len() {
        return Err(format!(
            "char count changed: source={}, saved={}",
            source.len(),
            saved.len()
        ));
    }

    for (index, (source_char, saved_char)) in source.iter().zip(saved).enumerate() {
        if source_char.value != saved_char.value {
            return Err(format!(
                "char value mismatch at {index}: context={:?}, source={source_char:?}, saved={saved_char:?}",
                context(source, index)
            ));
        }
        if source_char.paragraph_style != saved_char.paragraph_style {
            return Err(format!(
                "paragraph style mismatch at {index}: context={:?}, source={source_char:?}, saved={saved_char:?}",
                context(source, index)
            ));
        }
        if source_char.value != '\n'
            && source_char.value != rich_textbox::EMBEDDED_IMAGE_OBJECT_CHAR
            && source_char.value != rich_textbox::SOFT_PAGE_BREAK_CHAR
            && source_char.style != saved_char.style
        {
            return Err(format!(
                "char/style mismatch at {index}: context={:?}, source={source_char:?}, saved={saved_char:?}",
                context(source, index)
            ));
        }
    }

    Ok(())
}

fn context(chars: &[StyledChar], index: usize) -> String {
    let start = index.saturating_sub(30);
    let end = (index + 30).min(chars.len());
    chars[start..end]
        .iter()
        .map(|entry| match entry.value {
            '\n' => '⏎',
            '\t' => '⇥',
            value => value,
        })
        .collect()
}

fn compare_images(source: &[DocumentImage], saved: &[DocumentImage]) -> Result<(), String> {
    if source.len() != saved.len() {
        return Err(format!(
            "image count changed: source={}, saved={}",
            source.len(),
            saved.len()
        ));
    }

    for (index, (source_image, saved_image)) in source.iter().zip(saved).enumerate() {
        if source_image.size != saved_image.size {
            return Err(format!(
                "image size changed at {index}: source={:?}, saved={:?}",
                source_image.size, saved_image.size
            ));
        }
        if source_image.margin_left != saved_image.margin_left
            || source_image.margin_right != saved_image.margin_right
            || source_image.margin_top != saved_image.margin_top
            || source_image.margin_bottom != saved_image.margin_bottom
        {
            return Err(format!("image margins changed at {index}"));
        }
        if source_image.center_horizontally != saved_image.center_horizontally {
            return Err(format!("image centering changed at {index}"));
        }
        if source_image.color_image.size != saved_image.color_image.size
            || source_image.color_image.pixels != saved_image.color_image.pixels
        {
            return Err(format!("image pixels changed at {index}"));
        }
    }

    Ok(())
}
