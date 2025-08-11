use crate::Command;
use crate::Path;

pub async fn pdf_to_string(path: &Path) -> String {
    let output = Command::new("pdftotext")
        .args(&["-q", &path.to_string_lossy(), "-"])
        .output()
        .await
        .unwrap();

    let contents = str::from_utf8(&output.stdout).expect("Invalid UTF-8");
    println!("Output: {}", contents);

    return contents.to_string();
}

