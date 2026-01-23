//! Unit tests for FileStats
//!
//! Note: FileRepository.get_stats() requires a database connection and is
//! covered in integration tests. These unit tests focus on the FileStats struct.

use systemprompt_files::FileStats;

fn create_test_stats() -> FileStats {
    FileStats {
        total_files: 100,
        total_size_bytes: 10_000_000,
        ai_images_count: 5,
        image_count: 40,
        image_size_bytes: 5_000_000,
        document_count: 30,
        document_size_bytes: 3_000_000,
        audio_count: 15,
        audio_size_bytes: 1_500_000,
        video_count: 5,
        video_size_bytes: 400_000,
        other_count: 10,
        other_size_bytes: 100_000,
    }
}

#[test]
fn test_file_stats_total_files() {
    let stats = create_test_stats();
    assert_eq!(stats.total_files, 100);
}

#[test]
fn test_file_stats_total_size_bytes() {
    let stats = create_test_stats();
    assert_eq!(stats.total_size_bytes, 10_000_000);
}

#[test]
fn test_file_stats_ai_images_count() {
    let stats = create_test_stats();
    assert_eq!(stats.ai_images_count, 5);
}

#[test]
fn test_file_stats_image_count() {
    let stats = create_test_stats();
    assert_eq!(stats.image_count, 40);
}

#[test]
fn test_file_stats_image_size_bytes() {
    let stats = create_test_stats();
    assert_eq!(stats.image_size_bytes, 5_000_000);
}

#[test]
fn test_file_stats_document_count() {
    let stats = create_test_stats();
    assert_eq!(stats.document_count, 30);
}

#[test]
fn test_file_stats_document_size_bytes() {
    let stats = create_test_stats();
    assert_eq!(stats.document_size_bytes, 3_000_000);
}

#[test]
fn test_file_stats_audio_count() {
    let stats = create_test_stats();
    assert_eq!(stats.audio_count, 15);
}

#[test]
fn test_file_stats_audio_size_bytes() {
    let stats = create_test_stats();
    assert_eq!(stats.audio_size_bytes, 1_500_000);
}

#[test]
fn test_file_stats_video_count() {
    let stats = create_test_stats();
    assert_eq!(stats.video_count, 5);
}

#[test]
fn test_file_stats_video_size_bytes() {
    let stats = create_test_stats();
    assert_eq!(stats.video_size_bytes, 400_000);
}

#[test]
fn test_file_stats_other_count() {
    let stats = create_test_stats();
    assert_eq!(stats.other_count, 10);
}

#[test]
fn test_file_stats_other_size_bytes() {
    let stats = create_test_stats();
    assert_eq!(stats.other_size_bytes, 100_000);
}

#[test]
fn test_file_stats_clone() {
    let stats = create_test_stats();
    let cloned = stats;
    assert_eq!(stats.total_files, cloned.total_files);
    assert_eq!(stats.total_size_bytes, cloned.total_size_bytes);
    assert_eq!(stats.ai_images_count, cloned.ai_images_count);
}

#[test]
fn test_file_stats_copy() {
    let stats = create_test_stats();
    let copied: FileStats = stats;
    assert_eq!(stats.total_files, copied.total_files);
}

#[test]
fn test_file_stats_debug() {
    let stats = create_test_stats();
    let debug_str = format!("{:?}", stats);
    assert!(debug_str.contains("FileStats"));
    assert!(debug_str.contains("total_files"));
    assert!(debug_str.contains("100"));
}

#[test]
fn test_file_stats_empty() {
    let stats = FileStats {
        total_files: 0,
        total_size_bytes: 0,
        ai_images_count: 0,
        image_count: 0,
        image_size_bytes: 0,
        document_count: 0,
        document_size_bytes: 0,
        audio_count: 0,
        audio_size_bytes: 0,
        video_count: 0,
        video_size_bytes: 0,
        other_count: 0,
        other_size_bytes: 0,
    };
    assert_eq!(stats.total_files, 0);
    assert_eq!(stats.total_size_bytes, 0);
}

#[test]
fn test_file_stats_large_values() {
    let stats = FileStats {
        total_files: 1_000_000,
        total_size_bytes: 1_000_000_000_000,
        ai_images_count: 500_000,
        image_count: 400_000,
        image_size_bytes: 500_000_000_000,
        document_count: 300_000,
        document_size_bytes: 300_000_000_000,
        audio_count: 200_000,
        audio_size_bytes: 150_000_000_000,
        video_count: 50_000,
        video_size_bytes: 40_000_000_000,
        other_count: 50_000,
        other_size_bytes: 10_000_000_000,
    };
    assert_eq!(stats.total_files, 1_000_000);
    assert_eq!(stats.total_size_bytes, 1_000_000_000_000);
}

#[test]
fn test_file_stats_category_counts_sum() {
    let stats = create_test_stats();
    let category_sum =
        stats.image_count + stats.document_count + stats.audio_count + stats.video_count + stats.other_count;
    assert_eq!(category_sum, stats.total_files);
}

#[test]
fn test_file_stats_only_images() {
    let stats = FileStats {
        total_files: 50,
        total_size_bytes: 5_000_000,
        ai_images_count: 10,
        image_count: 50,
        image_size_bytes: 5_000_000,
        document_count: 0,
        document_size_bytes: 0,
        audio_count: 0,
        audio_size_bytes: 0,
        video_count: 0,
        video_size_bytes: 0,
        other_count: 0,
        other_size_bytes: 0,
    };
    assert_eq!(stats.image_count, stats.total_files);
    assert_eq!(stats.image_size_bytes, stats.total_size_bytes);
}

#[test]
fn test_file_stats_only_documents() {
    let stats = FileStats {
        total_files: 25,
        total_size_bytes: 2_500_000,
        ai_images_count: 0,
        image_count: 0,
        image_size_bytes: 0,
        document_count: 25,
        document_size_bytes: 2_500_000,
        audio_count: 0,
        audio_size_bytes: 0,
        video_count: 0,
        video_size_bytes: 0,
        other_count: 0,
        other_size_bytes: 0,
    };
    assert_eq!(stats.document_count, stats.total_files);
    assert_eq!(stats.document_size_bytes, stats.total_size_bytes);
}

#[test]
fn test_file_stats_all_ai_images() {
    let stats = FileStats {
        total_files: 20,
        total_size_bytes: 2_000_000,
        ai_images_count: 20,
        image_count: 20,
        image_size_bytes: 2_000_000,
        document_count: 0,
        document_size_bytes: 0,
        audio_count: 0,
        audio_size_bytes: 0,
        video_count: 0,
        video_size_bytes: 0,
        other_count: 0,
        other_size_bytes: 0,
    };
    assert_eq!(stats.ai_images_count, stats.total_files);
    assert_eq!(stats.ai_images_count, stats.image_count);
}
