pub fn sanitize_filename(filename: &str) -> String {
    // Only allow alphanumeric characters, dots, and underscores.
    // Replace everything else with an underscore.
    filename.replace(|c: char| !c.is_alphanumeric() && c != '.' && c != '_', "_")
}

pub fn is_allowed_mime_type(mime_type: &str) -> bool {
    let allowed = [
        "image/png",
        "image/jpeg",
        "image/gif",
        "image/webp",
        "application/pdf",
        "text/plain",
        "application/zip",
    ];
    allowed.contains(&mime_type)
}
