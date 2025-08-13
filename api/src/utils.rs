use std::fmt::format;

use crate::Command;
use crate::Path;
use pdfium_render::prelude::PdfPageRenderRotation;
use pdfium_render::prelude::PdfRenderConfig;
use pdfium_render::prelude::Pdfium;
use pdfium_render::prelude::PdfiumError;
use tantivy::Term;
use tantivy::query::BooleanQuery;
use tantivy::query::FuzzyTermQuery;
use tantivy::query::Occur;
use tantivy::schema::Field;

pub async fn pdf_to_string(path: &Path) -> String {
    let output = Command::new("pdftotext")
        .args(&["-q", &path.to_string_lossy(), "-"])
        .output()
        .await
        .unwrap();

    let contents = str::from_utf8(&output.stdout).expect("Invalid UTF-8");

    return contents.to_string();
}

/// Not async so should be run on a blocking thread pool
pub fn export_pdf_to_jpegs(
    file_name: &String,
    path: &impl AsRef<Path>,
    password: Option<&str>,
) -> Result<String, PdfiumError> {
    // Renders each page in the PDF file at the given path to a separate JPEG file.

    // Bind to a Pdfium library in the same directory as our Rust executable.
    // See the "Dynamic linking" section below.

    let pdfium = Pdfium::new(
        Pdfium::bind_to_library("/Users/somsinha/projects/papers/api/libpdfium.dylib").unwrap(),
    );

    // Load the document from the given path...

    let document = pdfium.load_pdf_from_file(path, password)?;

    // ... set rendering options that will be applied to all pages...

    let render_config = PdfRenderConfig::new()
        .set_target_width(2000)
        .set_maximum_height(2000)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

    // ... then render each page to a bitmap image, saving each image to a JPEG file.

    let first_page = document.pages().get(0);
    let image_path = format!("tmp/thumbnails/{}.jpg", file_name);

    first_page
        .expect("Expected a first page")
        .render_with_config(&render_config)?
        .as_image() // Renders this page to an image::DynamicImage...
        .into_rgb8() // ... then converts it to an image::Image...
        .save_with_format(&image_path, image::ImageFormat::Jpeg) // ... and saves it to a file.
        .map_err(|_| PdfiumError::ImageError)?;

    // Convert to absolute path
    let image_path = format!(
        "http://localhost:8080/api/static/thumbnails/{}.jpg",
        file_name
    );

    Ok(image_path)
}

pub fn simple_fuzzy_query(title: Field, body: Field, input: &str) -> tantivy::Result<BooleanQuery> {
    let q = input.trim().to_lowercase();
    if q.len() < 2 {
        return Ok(BooleanQuery::new(vec![]));
    }

    // choose edit distance based on length
    let dist = if q.len() >= 5 { 2 } else { 1 };

    let t_title = Term::from_field_text(title, &q);
    let t_body = Term::from_field_text(body, &q);

    let fq_title = FuzzyTermQuery::new(t_title, dist, true);
    let fq_body = FuzzyTermQuery::new(t_body, dist, true);

    Ok(BooleanQuery::new(vec![
        (Occur::Should, Box::new(fq_title)),
        (Occur::Should, Box::new(fq_body)),
    ]))
}
