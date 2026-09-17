//! Private transition evidence; this is not a public sequence parity claim.

use super::{FromJson, Value, img, json, require_ok, require_some, sha256};
use std::fs;
use std::path::Path;

fn field<T: FromJson>(value: &Value, name: &str) -> T {
    let object = require_some(value.as_object(), "trace record must be an object");
    require_ok(
        T::from_json(require_some(object.get(name), "trace field must exist").clone()),
        name,
    )
}

fn exact_keys(value: &Value, keys: &[&str]) {
    let object = require_some(value.as_object(), "trace record must be an object");
    assert_eq!(object.len(), keys.len(), "trace schema has unknown fields");
    assert!(keys.iter().all(|key| object.contains_key(*key)));
}

fn vector(value: [i32; 2]) -> [i16; 2] {
    value.map(|component| require_ok(i16::try_from(component), "native MV component fits i16"))
}

fn candidates(values: Vec<Value>, single: bool) -> Vec<img::Av1TemporalCandidate> {
    values
        .iter()
        .map(|value| {
            exact_keys(value, &["vectors", "weight"]);
            let vectors: [Option<[i32; 2]>; 2] = field(value, "vectors");
            assert_eq!(vectors[1].is_none(), single);
            img::Av1TemporalCandidate {
                vectors: [
                    vector(require_some(vectors[0], "first MV must be initialized")),
                    vectors[1].map_or([0; 2], vector),
                ],
                weight: field(value, "weight"),
            }
        })
        .collect()
}

#[test]
fn temporal_candidates_match_pinned_dav1d_animated_trace() {
    if super::matrix_selection_is_filtered() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bundle = root.join("tests/fixtures/outputs/av1_temporal/animated");
    let index: Value = require_ok(
        json::from_str(&require_ok(
            fs::read_to_string(bundle.join("index.json")),
            "oracle index",
        )),
        "oracle index JSON",
    );
    assert_eq!(
        field::<String>(&index, "schema"),
        "image-slash-star/av1-temporal-oracle@1"
    );
    let fixture: Value = field(&index, "fixture");
    assert_eq!(
        field::<String>(&fixture, "path"),
        "tests/fixtures/input/images/avif/animated.avif"
    );
    let data = require_ok(
        fs::read(root.join(field::<String>(&fixture, "path"))),
        "full AVIF input",
    );
    assert_eq!(
        sha256::digest_hex(&data),
        field::<String>(&fixture, "sha256")
    );
    let source: Value = field(&index, "source");
    assert_eq!(
        field::<String>(&source, "commit"),
        "b546257f770768b2c88258c533da38b91a06f737"
    );
    let artifacts: Vec<Value> = field(&index, "artifacts");
    for artifact in artifacts {
        let name: String = field(&artifact, "path");
        assert!(!name.contains('/') && !name.contains('\\') && !name.contains(".."));
        let bytes = require_ok(fs::read(bundle.join(name)), "oracle artifact");
        assert_eq!(bytes.len(), field::<usize>(&artifact, "bytes"));
        assert_eq!(
            sha256::digest_hex(&bytes),
            field::<String>(&artifact, "sha256")
        );
    }
    let trace = require_ok(
        fs::read_to_string(bundle.join("events.jsonl")),
        "temporal trace",
    );
    let mut valid_count = 0_usize;
    let mut insertions = 0_usize;
    let mut weight_updates = 0_usize;
    let mut context_updates = 0_usize;
    for (event_index, line) in trace.lines().enumerate() {
        let event: Value = require_ok(json::from_str(line), "temporal event JSON");
        exact_keys(
            &event,
            &[
                "event_index",
                "frame_offset",
                "target_references",
                "valid",
                "force_integer",
                "high_precision",
                "vector",
                "denominator",
                "distances",
                "global_vector",
                "global_context_before",
                "global_context_after",
                "stack_before",
                "stack_after",
            ],
        );
        assert_eq!(field::<usize>(&event, "event_index"), event_index);
        let references = field::<[i32; 2]>(&event, "target_references")
            .map(|value| require_ok(i8::try_from(value), "native reference fits i8"));
        let single = references[1] == -1;
        let valid: bool = field(&event, "valid");
        let denominator: Option<i32> = field(&event, "denominator");
        assert_eq!(valid, denominator.is_some());
        let before = img::Av1TemporalCandidateState {
            candidates: candidates(field(&event, "stack_before"), single),
            global_context: field(&event, "global_context_before"),
        };
        let expected = img::Av1TemporalCandidateState {
            candidates: candidates(field(&event, "stack_after"), single),
            global_context: field(&event, "global_context_after"),
        };
        if !valid {
            assert_eq!(
                before, expected,
                "invalid native entry must not change state"
            );
            continue;
        }
        valid_count = valid_count.saturating_add(1);
        insertions = insertions.saturating_add(usize::from(
            expected.candidates.len() > before.candidates.len(),
        ));
        weight_updates = weight_updates.saturating_add(usize::from(
            expected.candidates.len() == before.candidates.len()
                && expected.candidates != before.candidates,
        ));
        context_updates = context_updates.saturating_add(usize::from(
            expected.global_context != before.global_context,
        ));
        let input = img::Av1TemporalCandidateInput {
            references,
            projected: denominator.map(|value| (vector(field(&event, "vector")), value)),
            distances: field(&event, "distances"),
            force_integer: field(&event, "force_integer"),
            high_precision: field(&event, "high_precision"),
            global_vector: field::<Option<[i32; 2]>>(&event, "global_vector").map(vector),
            before,
        };
        let actual = require_ok(
            img::__coverage_av1_temporal_candidate(&input),
            "temporal candidate replay",
        );
        assert_eq!(actual, expected, "temporal event {event_index}");
    }
    assert_eq!(trace.lines().count(), 177);
    assert_eq!(
        (valid_count, insertions, weight_updates, context_updates),
        (32, 2, 30, 2)
    );
}
