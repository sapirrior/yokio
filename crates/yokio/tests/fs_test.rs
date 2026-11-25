//! Integration tests for Yokio's asynchronous filesystem (`fs`) operations.

#![cfg(feature = "fs")]

use yokio::fs::{self, File};

#[yokio::test]
fn fs_write_and_read_to_string() {
    let tmp_dir = std::env::temp_dir().join("yokio_test_write_read");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let file_path = tmp_dir.join("test.txt");

    let content = "Hello from Yokio async fs!";
    let path_clone = file_path.clone();

    fs::write(file_path.clone(), content.as_bytes(), move |write_res| {
        assert!(write_res.is_ok());

        fs::read_to_string(path_clone, move |read_res| {
            let text = read_res.expect("failed to read file");
            assert_eq!(text, content);
            let _ = std::fs::remove_dir_all(&tmp_dir);
        });
    });
}

#[yokio::test]
fn fs_append_and_metadata() {
    let tmp_dir = std::env::temp_dir().join("yokio_test_append_meta");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let file_path = tmp_dir.join("append.txt");

    let path_clone = file_path.clone();
    let path_clone2 = file_path.clone();

    fs::write(file_path.clone(), b"Hello ", move |res1| {
        assert!(res1.is_ok());

        fs::append(path_clone, b"World!", move |res2| {
            assert!(res2.is_ok());

            fs::metadata(path_clone2, move |meta_res| {
                let meta = meta_res.expect("metadata failed");
                assert_eq!(meta.len(), 12);
                let _ = std::fs::remove_dir_all(&tmp_dir);
            });
        });
    });
}

#[yokio::test]
fn fs_dir_operations() {
    let tmp_dir = std::env::temp_dir().join("yokio_test_dirs");
    let _ = std::fs::remove_dir_all(&tmp_dir);

    let sub_dir = tmp_dir.join("a/b/c");
    let sub_clone = sub_dir.clone();
    let tmp_clone = tmp_dir.clone();

    fs::create_dir_all(sub_dir, move |res| {
        assert!(res.is_ok());

        fs::read_dir(sub_clone, move |entries_res| {
            let entries = entries_res.expect("read_dir failed");
            assert!(entries.is_empty());

            fs::remove_dir_all(tmp_clone, move |rm_res| {
                assert!(rm_res.is_ok());
            });
        });
    });
}

#[yokio::test]
fn fs_file_handle_stream() {
    let tmp_dir = std::env::temp_dir().join("yokio_test_file_handle");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let file_path = tmp_dir.join("stream.bin");
    let path_clone = file_path.clone();

    File::create(file_path, move |create_res| {
        let file = create_res.expect("failed to create file");
        let file_clone = file.clone();

        file.write(b"0123456789", move |write_res| {
            assert_eq!(write_res.expect("write failed"), 10);

            file_clone.sync_all(move |sync_res| {
                assert!(sync_res.is_ok());

                File::open(path_clone, move |open_res| {
                    let rfile = open_res.expect("open failed");

                    rfile.read(5, move |read_res| {
                        let bytes = read_res.expect("read failed");
                        assert_eq!(&bytes, b"01234");
                        let _ = std::fs::remove_dir_all(&tmp_dir);
                    });
                });
            });
        });
    });
}
