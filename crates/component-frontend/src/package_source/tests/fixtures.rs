use wasm_meta_registry_client::{KnownPackage, PackageVersion};

pub(super) const WIT: &str = r"
package example:http@0.1.0;

interface types {
    record request {
        value: u32,
    }
    resource response {
        constructor();
        get: func() -> u32;
    }
    send: func(value: request) -> u32;
}

world proxy {
    import types;
    export types;
    export run: func();
}
";

pub(super) fn package() -> KnownPackage {
    let mut pkg = crate::components::ds::package_row::tests::packages().remove(0);
    pkg.registry = "mirror.test".to_owned();
    pkg.repository = "mirrors/http".to_owned();
    pkg.tags = vec!["0.2.0".to_owned(), "0.1.0".to_owned()];
    pkg.dependencies = vec![wasm_meta_registry_client::PackageDependencyRef {
        package: "other:types".to_owned(),
        version: Some("1.0.0".to_owned()),
    }];
    pkg
}

pub(super) fn version(wit: Option<&str>) -> PackageVersion {
    serde_json::from_value(serde_json::json!({
        "tag": "0.1.0",
        "digest": "sha256:mirror-a",
        "wit_text": wit,
        "components": [{
            "kind": "component",
            "children": [
                {"kind": "module", "name": "tool"},
                {"kind": "component", "name": "worker"}
            ]
        }]
    }))
    .expect("valid selected-mirror version fixture")
}
