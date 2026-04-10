/// Tests for file upload and sharing handlers
use server::database::utils::{build_storage_path, sanitize_filename};

// ── Filename sanitization ──────────────────────────────────────────────────

#[test]
fn sanitize_strips_path_traversal() {
    assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
}

#[test]
fn sanitize_strips_unix_path_traversal() {
    assert_eq!(
        sanitize_filename("../../../sensitive_file.txt"),
        "sensitive_file.txt"
    );
}

#[test]
fn sanitize_strips_windows_separators() {
    assert_eq!(sanitize_filename("C:\\Users\\file.txt"), "file.txt");
}

#[test]
fn sanitize_strips_multiple_windows_paths() {
    assert_eq!(
        sanitize_filename("C:\\Users\\Admin\\Documents\\report.pdf"),
        "report.pdf"
    );
}

#[test]
fn sanitize_normal_filename_unchanged() {
    assert_eq!(sanitize_filename("report-2026.pdf"), "report-2026.pdf");
}

#[test]
fn sanitize_filename_with_dots() {
    assert_eq!(sanitize_filename("archive.tar.gz"), "archive.tar.gz");
}

#[test]
fn sanitize_filename_with_underscores() {
    assert_eq!(
        sanitize_filename("my_file_name_2024.docx"),
        "my_file_name_2024.docx"
    );
}

#[test]
fn sanitize_filename_with_hyphens() {
    assert_eq!(
        sanitize_filename("report-final-v2.pdf"),
        "report-final-v2.pdf"
    );
}

#[test]
fn sanitize_null_byte_removed() {
    assert_eq!(sanitize_filename("file\0name.txt"), "filename.txt");
}

#[test]
fn sanitize_multiple_null_bytes() {
    assert_eq!(sanitize_filename("file\0na\0me.txt"), "filename.txt");
}

#[test]
fn sanitize_empty_becomes_unnamed() {
    assert_eq!(sanitize_filename(""), "unnamed");
}

#[test]
fn sanitize_only_path_separators_becomes_unnamed() {
    assert_eq!(sanitize_filename("///"), "unnamed");
}

#[test]
fn sanitize_only_backslashes_becomes_unnamed() {
    assert_eq!(sanitize_filename("\\\\\\"), "unnamed");
}

#[test]
fn sanitize_mixed_separators() {
    assert_eq!(sanitize_filename("path/to\\file.txt"), "file.txt");
}

#[test]
fn sanitize_preserves_special_chars_in_filename() {
    assert_eq!(
        sanitize_filename("report@2024[final].pdf"),
        "report@2024[final].pdf"
    );
}

#[test]
fn sanitize_preserves_unicode_chars() {
    assert_eq!(sanitize_filename("documento_é.pdf"), "documento_é.pdf");
}

#[test]
fn sanitize_whitespace_preserved() {
    assert_eq!(
        sanitize_filename("my file (copy).txt"),
        "my file (copy).txt"
    );
}

// ── Storage path building ──────────────────────────────────────────────────

#[test]
fn build_storage_path_includes_filename() {
    let p = build_storage_path("/uploads", "photo.jpg");
    let name = p.file_name().unwrap().to_str().unwrap();
    assert!(name.ends_with("photo.jpg"));
    assert!(name.len() > "photo.jpg".len()); // has UUID prefix
}

#[test]
fn build_storage_path_includes_base_dir() {
    let p = build_storage_path("/uploads", "photo.jpg");
    assert!(p.to_str().unwrap().starts_with("/uploads"));
}

#[test]
fn build_storage_path_with_relative_base() {
    let p = build_storage_path("./uploads", "photo.jpg");
    assert!(p.to_str().unwrap().starts_with("./uploads"));
}

#[test]
fn build_storage_path_contains_uuid() {
    let p1 = build_storage_path("/uploads", "file.txt");
    let p2 = build_storage_path("/uploads", "file.txt");
    // Different UUIDs should make different paths
    assert_ne!(p1, p2);
}

#[test]
fn build_storage_path_uuid_format() {
    let p = build_storage_path("/uploads", "test.pdf");
    let filename = p.file_name().unwrap().to_str().unwrap();
    // Should contain hyphen from UUID
    assert!(filename.contains('-'));
}

#[test]
fn build_storage_path_with_complex_filename() {
    let p = build_storage_path("/data/storage", "document (2024-01-15).pdf");
    let filename = p.file_name().unwrap().to_str().unwrap();
    assert!(filename.ends_with("document (2024-01-15).pdf"));
}

// ── Filename combinations ──────────────────────────────────────────────────

#[test]
fn sanitize_path_with_dots_preserved() {
    assert_eq!(
        sanitize_filename("thesis.v2.final.docx"),
        "thesis.v2.final.docx"
    );
}

#[test]
fn sanitize_filename_with_numbers() {
    assert_eq!(
        sanitize_filename("IMG_20240315_123456.jpg"),
        "IMG_20240315_123456.jpg"
    );
}

#[test]
fn sanitize_filename_parentheses() {
    assert_eq!(sanitize_filename("document (1).txt"), "document (1).txt");
}

#[test]
fn sanitize_filename_brackets() {
    assert_eq!(sanitize_filename("file[backup].zip"), "file[backup].zip");
}

#[test]
fn sanitize_with_trailing_slash() {
    assert_eq!(sanitize_filename("filename.txt/"), "filename.txt");
}

#[test]
fn sanitize_with_leading_slash() {
    assert_eq!(sanitize_filename("/filename.txt"), "filename.txt");
}

#[test]
fn sanitize_with_multiple_slashes() {
    assert_eq!(sanitize_filename("///filename.txt///"), "filename.txt");
}

// ── Edge cases ─────────────────────────────────────────────────────────────

#[test]
fn sanitize_very_long_filename() {
    let long_name = "a".repeat(500);
    let result = sanitize_filename(&long_name);
    assert_eq!(result.len(), 500);
}

#[test]
fn sanitize_single_character() {
    assert_eq!(sanitize_filename("a"), "a");
}

#[test]
fn sanitize_dot_file_unix_style() {
    assert_eq!(sanitize_filename(".hidden"), ".hidden");
}

#[test]
fn sanitize_dotdot_becomes_unnamed() {
    assert_eq!(sanitize_filename(".."), "unnamed");
}

#[test]
fn sanitize_single_dot_becomes_unnamed() {
    assert_eq!(sanitize_filename("."), "unnamed");
}

#[test]
fn sanitize_null_at_end() {
    assert_eq!(sanitize_filename("filename.txt\0"), "filename.txt");
}

#[test]
fn sanitize_null_at_start() {
    assert_eq!(sanitize_filename("\0filename.txt"), "filename.txt");
}

// ── Path building edge cases ───────────────────────────────────────────────

#[test]
fn build_storage_path_home_directory() {
    let p = build_storage_path("~/uploads", "file.txt");
    assert!(p.to_str().unwrap().contains("~/uploads"));
}

#[test]
fn build_storage_path_absolute_unix_path() {
    let p = build_storage_path("/var/uploads", "file.txt");
    assert!(p.to_str().unwrap().starts_with("/var/uploads"));
}

#[test]
fn build_storage_path_multiple_base_dirs() {
    let p = build_storage_path("/data/uploads/files", "file.txt");
    assert!(p.to_str().unwrap().contains("/data/uploads/files"));
}

// ── Integration scenarios ──────────────────────────────────────────────────

#[test]
fn sanitize_then_build_path() {
    let dirty = "../../etc/bad.txt";
    let clean = sanitize_filename(dirty);
    let path = build_storage_path("/uploads", &clean);
    let filename = path.file_name().unwrap().to_str().unwrap();
    assert!(filename.ends_with("bad.txt"));
    assert!(!filename.contains(".."));
}

#[test]
fn multiple_file_paths_are_unique() {
    let p1 = build_storage_path("/uploads", "file.txt");
    let p2 = build_storage_path("/uploads", "file.txt");
    let p3 = build_storage_path("/uploads", "file.txt");

    // All should be different due to UUID
    assert_ne!(p1, p2);
    assert_ne!(p2, p3);
    assert_ne!(p1, p3);
}

#[test]
fn sanitize_real_world_examples() {
    // Common real-world test cases
    assert_eq!(sanitize_filename("Invoice_2024.pdf"), "Invoice_2024.pdf");
    assert_eq!(
        sanitize_filename("Meeting Notes (Jan 15).docx"),
        "Meeting Notes (Jan 15).docx"
    );
    assert_eq!(
        sanitize_filename("video-HD-final.mp4"),
        "video-HD-final.mp4"
    );
    assert_eq!(
        sanitize_filename("backup_20240315.tar.gz"),
        "backup_20240315.tar.gz"
    );
}

#[test]
fn sanitize_prevents_directory_listing() {
    let attempts = vec![".", "..", "..\\", "../", "~/.", "/~"];
    for attempt in attempts {
        let result = sanitize_filename(attempt);
        // Should either be "unnamed" or just the filename part
        assert!(!result.contains("..")); // No path traversal
    }
}
