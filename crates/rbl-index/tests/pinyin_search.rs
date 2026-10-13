use rbl_index::{testing::{library_from, TestTrack}, SearchField, ViewSpec, SortColumn, TrackSource};

fn spec(query: String) -> ViewSpec { ViewSpec { source: TrackSource::Collection, sort: SortColumn::TrackNo, descending: false, query, filter: Default::default() } }

#[test]
fn chinese_title_and_artist_accept_full_pinyin_initials_and_combined_tokens() {
    let library = library_from(&[
        TestTrack { id: 1, title: "七里香 (DJ Remix 2024)", artist: "周杰伦", ..TestTrack::default() },
        TestTrack { id: 2, title: "重庆森林", artist: "王菲", ..TestTrack::default() },
        TestTrack { id: 3, title: "女儿情", artist: "邓丽君", ..TestTrack::default() },
    ]);
    for query in ["七里香", "qilixiang", "qi li xiang", "qlx", "ZJL QLX", "qlx dj remix 2024"] {
        assert_eq!(library.open_view(&spec(query.into())).len(), 1, "{query}");
    }
    assert_eq!(library.open_view_scoped(&spec("zjl".into()), SearchField::Artist).len(), 1);
    assert_eq!(library.open_view_scoped(&spec("qlx".into()), SearchField::Title).len(), 1);
    assert!(library.open_view_scoped(&spec("zjl".into()), SearchField::Title).is_empty());
    for query in ["chongqing", "cqsl", "nverqing", "nv er qing"] {
        assert_eq!(library.open_view(&spec(query.into())).len(), 1, "{query}");
    }
}

#[test]
fn corrections_survive_regeneration_and_snapshot_but_expire_when_the_title_changes() {
    use rbl_index::{pinyin::{ReadingOverride, TrackOverride}, cache::{encode, decode, Fingerprint, FORMAT}, strings::StrColumn};
    let mut library = library_from(&[TestTrack { id: 1, title: "重庆森林", artist: "王菲", ..TestTrack::default() }]);
    let custom = std::collections::HashMap::from([(1, TrackOverride { title: Some(ReadingOverride { original: "重庆森林".into(), reading: "zhong qing sen lin".into() }), artist: None })]);
    library.apply_pinyin_overrides(custom.clone());
    assert_eq!(library.pinyin_reading("1").unwrap().title_initials, "zqsl");
    for _ in 0..2 {
        assert_eq!(library.rebuild_pinyin(None, |_, _| {}), (1, 1));
        assert_eq!(library.open_view(&spec("zqsl".into())).len(), 1);
        assert_eq!(library.title.get(0), "重庆森林");
    }
    let fingerprint = Fingerprint { format: FORMAT, db_len: 0, db_modified_ns: 0, wal_len: 0, wal_modified_ns: 0, db_version: 0, content: 0, database: 1 };
    let mut restored = decode(&encode(&library, fingerprint), fingerprint).unwrap();
    assert_eq!(restored.pinyin_reading("1").unwrap().title_initials, "zqsl");
    restored.apply_pinyin_overrides(std::collections::HashMap::new());
    assert_eq!(restored.open_view(&spec("cqsl".into())).len(), 1);
    let mut changed = StrColumn::default(); changed.push("七里香");
    library.title = changed;
    library.apply_pinyin_overrides(custom);
    assert_eq!(library.open_view(&spec("qlx".into())).len(), 1);
    assert_eq!(library.open_view(&spec("zqsl".into())).len(), 0);
}
