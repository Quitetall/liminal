//! AM-17.12: explicit acceptance records the caller, never a generated actor.

use liminal_daemon::{FsExecutor, runner::StepRunner, scenario::ScenarioScript};
use liminal_graph::ns::{ILRP_INTENT, JUR_REPAIR};
use liminal_id::ActorId;
use liminal_jurisdiction::{NoCrash, RepairRecord, SafetyEvidence};

const ACTOR: &str = "actor:018f0000-0000-7000-8000-000000000001";

fn acceptance_script() -> ScenarioScript {
    toml::from_str(include_str!(
        "../../../conformance/fixtures/scenarios/dag_accept.scenario.toml"
    ))
    .unwrap()
}

#[test]
fn acceptance_without_valid_actor_has_no_acceptance_effects() {
    for actor in [
        None,
        Some(toml::Value::String("not-an-actor".into())),
        Some(toml::Value::Integer(7)),
    ] {
        let root = liminal_scratch::ScratchDir::new("acceptance-missing-actor").unwrap();
        let mut script = acceptance_script();
        let accept = script.steps.last_mut().unwrap();
        assert_eq!(accept.kind, "accept_repair");
        accept.extra.remove("actor");
        if let Some(actor) = actor {
            accept.extra.insert("actor".into(), actor);
        }
        let mut runner = StepRunner::open(
            &root,
            &script.setup,
            FsExecutor::new(root.to_owned()),
            NoCrash,
        )
        .unwrap();
        for step in &script.steps[..script.steps.len() - 1] {
            runner.step(step).unwrap();
        }
        let bytes = std::fs::read(root.join("notes.md")).unwrap();
        let head = runner.workspace().store().head().unwrap();
        assert!(
            runner.step(script.steps.last().unwrap()).is_err(),
            "missing/malformed actor must refuse"
        );
        assert_eq!(runner.workspace().store().head().unwrap(), head);
        assert_eq!(std::fs::read(root.join("notes.md")).unwrap(), bytes);
        assert!(
            runner
                .workspace()
                .store()
                .scan_aux(ILRP_INTENT)
                .unwrap()
                .is_empty()
        );
        assert!(
            runner
                .workspace()
                .store()
                .scan_aux(JUR_REPAIR)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn acceptance_records_exact_caller_supplied_actor() {
    let root = liminal_scratch::ScratchDir::new("acceptance-explicit-actor").unwrap();
    let mut script = acceptance_script();
    script
        .steps
        .last_mut()
        .unwrap()
        .extra
        .insert("actor".into(), toml::Value::String(ACTOR.into()));
    let mut runner = StepRunner::open(
        &root,
        &script.setup,
        FsExecutor::new(root.to_owned()),
        NoCrash,
    )
    .unwrap();
    for step in &script.steps {
        runner.step(step).unwrap();
    }
    let records = runner.workspace().store().scan_aux(JUR_REPAIR).unwrap();
    assert_eq!(records.len(), 1);
    let record: RepairRecord = serde_json::from_value(records[0].1.clone()).unwrap();
    match record.evidence {
        SafetyEvidence::HumanApproval { actor, .. } => {
            assert_eq!(actor, ACTOR.parse::<ActorId>().unwrap());
        }
        other => panic!("expected explicit human approval, got {other:?}"),
    }
}
