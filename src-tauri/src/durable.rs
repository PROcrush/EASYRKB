pub use rbl_core::durable::{create_dir_all, sync_dir, write};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[test]
    fn replaces_existing_files_and_cleans_up_on_rename_failure() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("test.dat");
        super::write(&file, b"first").unwrap();
        super::write(&file, b"second").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"second");
        assert!(super::write(root.path(), b"cannot replace directory").is_err());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
