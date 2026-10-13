//! Cloud-library paths resolve under the Dropbox root rekordbox uses.
use std::path::Path;

#[test]
fn a_cloud_library_path_lands_under_the_cloud_root_and_a_plain_one_stays() {
    let root = Path::new("/Users/dj/Dropbox/rekordbox");
    assert_eq!(
        rbl_db::resolve_folder_path("/contents_1912725212/husko/milkshake.mp3", Some(root)),
        "/Users/dj/Dropbox/rekordbox/contents_1912725212/husko/milkshake.mp3"
    );
    assert_eq!(rbl_db::resolve_folder_path("/Volumes/SD/RB/one.mp3", Some(root)), "/Volumes/SD/RB/one.mp3");
    // Without a root the path is left alone, and reads as missing.
    assert_eq!(rbl_db::resolve_folder_path("/contents_1/x.mp3", None), "/contents_1/x.mp3");
}
