use super::*;

fn candidate(name: &str, tag: &str) -> MatchingReleaseRow {
    MatchingReleaseRow {
        repo_id: 1,
        registry: "registry.test".to_owned(),
        repository: name.replace(':', "/"),
        source_name: Some(name.to_owned()),
        tag: tag.to_owned(),
        world_id: None,
        world_name: None,
        is_synthetic: false,
    }
}

#[test]
fn relationship_release_stream_retains_only_the_page_and_current_identity() {
    let mut page = MatchingReleasePage::new(5_000, 3);
    for index in 0..10_000 {
        let name = format!("test:package{index:05}");
        for tag in [
            "latest",
            "1.9.0",
            "1.10.0_build.2",
            "v9.0.0",
            "sha256-abc.sig",
        ] {
            page.push(candidate(&name, tag)).expect("ordered candidate");
            let retained = page.page.results.len() + usize::from(page.current.is_some());
            assert!(retained <= 4, "retained {retained} releases after {name}");
        }
    }
    let result = page.finish();
    assert_eq!(result.total, Some(10_000));
    assert!(result.has_next);
    assert_eq!(result.offset, 5_000);
    assert_eq!(result.limit, 3);
    assert_eq!(
        result
            .results
            .iter()
            .map(|row| row.source_name.as_deref().expect("WIT identity"))
            .collect::<Vec<_>>(),
        [
            "test:package05000",
            "test:package05001",
            "test:package05002"
        ]
    );
    assert!(result.results.iter().all(|row| row.tag == "1.10.0_build.2"));
}

#[test]
fn relationship_release_stream_selects_mirrors_independently_of_candidate_order() {
    for reverse in [false, true] {
        let mut rows = [
            candidate("test:one", "1.10.0"),
            candidate("test:one", "1.10.0"),
            candidate("test:one", "1.9.0"),
        ];
        rows[0].registry = "z.test".to_owned();
        rows[1].registry = "a.test".to_owned();
        if reverse {
            rows.reverse();
        }
        let mut page = MatchingReleasePage::new(0, 1);
        for row in rows {
            page.push(row).expect("same identity");
        }
        let result = page.finish();
        assert_eq!(result.total, Some(1));
        assert!(!result.has_next);
        let selected = result.results.first().expect("selected release");
        assert_eq!(selected.tag, "1.10.0");
        assert_eq!(selected.registry, "a.test");
    }
}

#[test]
fn relationship_release_stream_handles_empty_and_extreme_offsets_without_preallocation() {
    let empty = MatchingReleasePage::new(0, 100).finish();
    assert_eq!(empty.total, Some(0));
    assert!(!empty.has_next);
    let mut page = MatchingReleasePage::new(u32::MAX, u32::MAX);
    for (name, tag) in [("test:a", "latest"), ("test:b", "1.0.0")] {
        page.push(candidate(name, tag)).expect("ordered candidates");
    }
    let result = page.finish();
    assert_eq!(result.total, Some(1));
    assert!(result.results.is_empty());
    assert!(!result.has_next);
}

#[test]
fn relationship_release_stream_rejects_broken_identity_ordering() {
    let mut page = MatchingReleasePage::new(0, 100);
    page.push(candidate("test:z", "1.0.0"))
        .expect("first identity");
    let error = page
        .push(candidate("test:a", "1.0.0"))
        .expect_err("out-of-order candidates must not silently split an identity");
    assert!(error.to_string().contains("not ordered"));
}
