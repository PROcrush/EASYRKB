#![allow(clippy::unwrap_used)]
use rbl_db::{fixture, write::{Writer, TrackField}, Library, OpenMode};
use rbl_index::{SearchField, TrackFilter, TrackSource, ViewSpec, SortColumn};

#[test]
fn field_search_resolves_metadata_and_survives_a_cached_restart() {
    let root = tempfile::tempdir().unwrap();
    let location = fixture::build(root.path(), fixture::Shape::default()).unwrap();
    let id = fixture::track_id(0);
    let cases = [
        ("title", "UniqueTitle", SearchField::Title), ("artist", "UniqueArtist", SearchField::Artist),
        ("album", "UniqueAlbum", SearchField::Album), ("genre", "UniqueGenre", SearchField::Genre),
        ("year", "1997", SearchField::Year), ("bpm", "127.5", SearchField::Bpm),
        ("composer", "UniqueComposer", SearchField::Composer), ("albumArtist", "UniqueAlbumArtist", SearchField::AlbumArtist),
        ("remixer", "UniqueRemixer", SearchField::Remixer), ("label", "UniqueLabel", SearchField::Label),
        ("originalArtist", "UniqueOriginalArtist", SearchField::OriginalArtist), ("mixName", "UniqueMix", SearchField::MixName),
    ];
    let mut writer = Writer::open(location.clone(), root.path().join("backups")).unwrap();
    for (field, value, _) in cases {
        if let Some(field) = TrackField::parse(field) { writer.set_field(&id, field, value).unwrap(); }
    }
    writer.library().connection().execute_batch("INSERT INTO djmdArtist (ID, Name, created_at, updated_at) VALUES ('search-album-artist', 'UniqueAlbumArtist', '', '')").unwrap();
    writer.library().connection().execute("UPDATE djmdAlbum SET AlbumArtistID='search-album-artist' WHERE ID=(SELECT AlbumID FROM djmdContent WHERE ID=?1)", [&id]).unwrap();
    writer.library().connection().execute("UPDATE djmdContent SET Subtitle='UniqueMix' WHERE ID=?1", [&id]).unwrap();
    writer.set_comment(&id, "Café\tMoon").unwrap();
    drop(writer);
    let db = Library::open(location.clone(), OpenMode::ReadOnly).unwrap();
    let (index, _) = rbl_index::load(&db).unwrap();
    let fingerprint = rbl_index::cache::Fingerprint::of(&location.master_db, 0, 0).unwrap();
    let encoded = rbl_index::cache::encode(&index, fingerprint);
    let cached = rbl_index::cache::decode(&encoded, fingerprint).unwrap();
    for library in [&index, &cached] {
        for (_, query, field) in cases {
            let spec = ViewSpec { source: TrackSource::Collection, sort: SortColumn::Title,
                descending: false, query: query.into(), filter: TrackFilter::default() };
            let view = library.open_view_scoped(&spec, field);
            assert_eq!(view.len(), 1, "{field:?}");
            assert_eq!(library.ids[view.rows[0] as usize].to_string(), id);
            assert_eq!(library.open_view(&spec).len(), 1, "All includes {field:?}");
            let wrong = if field == SearchField::Title { SearchField::Comment } else { SearchField::Title };
            assert!(library.open_view_scoped(&spec, wrong).is_empty(), "{field:?} does not search other fields");
        }
        let spec = ViewSpec { source: TrackSource::Collection, sort: SortColumn::Title,
            descending: false, query: "cafe moon".into(), filter: TrackFilter::default() };
        assert_eq!(library.open_view_scoped(&spec, SearchField::Comment).len(), 1);
        assert!(library.open_view_scoped(&spec, SearchField::MixName).is_empty());
    }
}
