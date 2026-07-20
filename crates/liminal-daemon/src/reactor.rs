//! The effect reactor (v4 §8.6, M08.6): observations of the outside world
//! enter the graph as ordinary transactions from OUTSIDE the query engine — a
//! query never performs effectful resolution merely because it is inspected
//! (v4 §39). `ToyWorkspace::inject_observation` is the toy's one entry point
//! (M08 Algorithm B); [`ScriptedStockResolver`] is the demo
//! [`ReplayableResolver`] instance that feeds it (D08.5: resolvers stay OUT of
//! `liminal-query`; the reactor lives here, not there).

use std::collections::VecDeque;

use liminal_graph::{GraphStore, Node, NodeFlags, Operation, PayloadRef, kind, ns::SYS_BLOB};
use liminal_id::{
    ContentHash, GraphRevisionId, JurisdictionKey, NodeId, RevisionId, SourceId, Timestamp,
};
use liminal_resolver::ReplayableResolver;
use liminal_revision::BasisComponent;

use crate::workspace::{ToyWorkspace, WorkspaceError, current_observation_key};

/// One point-in-time observation of external state (v4 §7.5 `Observation`
/// Basis component; M08 Algorithm B). Doubles as [`ScriptedStockResolver`]'s
/// associated `Observation` type — the resolver PRODUCES exactly what the
/// reactor materializes, so `observe`/`replay` output feeds
/// [`ToyWorkspace::inject_observation`] directly.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Observation {
    /// The observed external source.
    pub source: SourceId,
    /// When the observation was made.
    pub observed_at: Timestamp,
    /// The observed payload (canonical JSON).
    pub payload: serde_json::Value,
}

impl ToyWorkspace {
    /// Inject one observation as a graph transaction (M08 Algorithm B):
    ///
    /// 1. Hash the canonical payload bytes.
    /// 2. In one transaction, create the toy's external-value node when absent,
    ///    persist payload at `SYS_BLOB["obs/<source>/<hash>"]`, persist restart
    ///    metadata at `SYS_BLOB["obs-current/<source>"]`, and commit
    ///    `MaterializeExternal`.
    /// 3. Republish `AvailableInputs.durable[Source(source)]` so a fresh
    ///    Basis records the new observation as a dependency (D08.2) — the
    ///    same "durable input map is the single source of truth" discipline
    ///    `seed_durable_inputs` applies at reopen (D06.5).
    pub fn inject_observation(
        &mut self,
        obs: Observation,
    ) -> Result<GraphRevisionId, WorkspaceError> {
        let Observation {
            source,
            observed_at,
            payload,
        } = obs;

        let store = self.store();
        let (node, create_node) = external_value_node(store)?;

        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
        let hash = ContentHash::of(&payload_bytes);

        let component = BasisComponent::Observation {
            source,
            observed_at,
            hash,
        };
        let component_value = serde_json::to_value(&component)
            .map_err(|error| liminal_graph::StoreError::Corrupt(error.to_string()))?;

        let mut txn = store.begin()?;
        if create_node {
            txn.apply(Operation::CreateNode {
                node: Node {
                    id: node,
                    kind: kind::EXTERNAL_VALUE,
                    payload: PayloadRef::None,
                    revision: RevisionId(0),
                    flags: NodeFlags::default(),
                },
            })?;
        }
        txn.put_aux(SYS_BLOB, &format!("obs/{source}/{hash}"), payload)?;
        txn.put_aux(SYS_BLOB, &current_observation_key(source), component_value)?;
        txn.apply(Operation::MaterializeExternal {
            node,
            source,
            observed: hash,
            at: observed_at,
        })?;
        let (revision, _txn_id) = txn.commit(reactor_meta())?;

        self.inputs_mut()
            .durable
            .insert(JurisdictionKey::Source(source), component);

        Ok(revision)
    }
}

/// The single external-value node in the toy graph, creating it on first use.
/// A real implementation would key nodes on `source` explicitly; the toy
/// never scripts more than one external source per scenario, so "the one
/// `EXTERNAL_VALUE` node" is unambiguous — the same simplification
/// `file_node_for` applies to the single FILE node.
fn external_value_node(store: &GraphStore) -> Result<(NodeId, bool), WorkspaceError> {
    for node in store.nodes()? {
        if node.kind == kind::EXTERNAL_VALUE {
            return Ok((node.id, false));
        }
    }
    Ok((NodeId::new(), true))
}

/// Metadata for reactor-driven graph transactions. Origin is `Remote` — an
/// observation comes from an external service, not a human edit (v4 §84
/// origin vocabulary).
fn reactor_meta() -> liminal_graph::TxnMeta {
    liminal_graph::TxnMeta {
        actor: None,
        origin: liminal_graph::Origin::Remote,
        at: Timestamp::now(),
        provenance: Some("resolver:observe".into()),
        inverse: None,
    }
}

/// A demo effectful resolver (M08.6, D08.5): a scripted queue of
/// observations, popped one per [`observe`](ReplayableResolver::observe)
/// call — the toy's stand-in for a real network-backed stock-price resolver.
/// [`replay`](ReplayableResolver::replay) is the pure counterpart: it never
/// pops, it looks `request` up in the frozen observation log and returns it
/// unchanged (v4 §112's determinism law) — Ambiguous/absent lookups are never
/// silently guessed at, matching the project's anti-guessing stance
/// elsewhere (e.g. M09's D09.2 anchor recovery).
#[derive(Debug, Default)]
pub struct ScriptedStockResolver {
    script: VecDeque<Observation>,
}

impl ScriptedStockResolver {
    /// A resolver scripted to pop `observations` in order, one per `observe`
    /// call.
    #[must_use]
    pub fn scripted(observations: Vec<Observation>) -> Self {
        Self {
            script: observations.into(),
        }
    }
}

impl ReplayableResolver for ScriptedStockResolver {
    type Request = SourceId;
    type Observation = Observation;

    /// Effectful: pops and returns the next scripted observation for
    /// `request`. Panics if the script is exhausted or the next entry is for
    /// a different source — a scenario that observes without scripting is a
    /// fixture bug, not a case to guess through.
    fn observe(&mut self, request: &SourceId) -> Observation {
        let obs = self
            .script
            .pop_front()
            .expect("ScriptedStockResolver: observe() called with an empty script");
        assert_eq!(
            &obs.source, request,
            "ScriptedStockResolver: scripted observation is for a different source"
        );
        obs
    }

    /// Pure: the most recent frozen observation for `request`, unchanged.
    /// Panics rather than guessing when none matches (v4 §112: replay
    /// reproduces a recorded observation exactly, or it is not a replay).
    fn replay(&self, request: &SourceId, frozen: &[Observation]) -> Observation {
        frozen
            .iter()
            .rev()
            .find(|o| &o.source == request)
            .cloned()
            .expect("ScriptedStockResolver: replay() found no frozen observation for source")
    }
}

/// Parse a `YYYY-MM-DDTHH:MM:SSZ` UTC instant into a [`Timestamp`].
/// `Timestamp` is deliberately dependency-free for Phase -1 (no chrono/jiff
/// yet, per its own doc note), and this is the one format the scenario
/// vocabulary emits (`resolver_observe`'s `at` field) — anything else is
/// rejected rather than guessed at.
pub(crate) fn parse_utc_timestamp(s: &str) -> anyhow::Result<Timestamp> {
    let body = s
        .strip_suffix('Z')
        .ok_or_else(|| anyhow::anyhow!("timestamp {s:?} must be UTC (trailing 'Z')"))?;
    let (date, time) = body
        .split_once('T')
        .ok_or_else(|| anyhow::anyhow!("timestamp {s:?} missing 'T' date/time separator"))?;

    let field = |part: Option<&str>, name: &str| -> anyhow::Result<i64> {
        part.ok_or_else(|| anyhow::anyhow!("timestamp {s:?} missing {name}"))?
            .parse::<i64>()
            .map_err(|e| anyhow::anyhow!("timestamp {s:?} bad {name}: {e}"))
    };
    let mut date_parts = date.splitn(3, '-');
    let year = field(date_parts.next(), "year")?;
    let month = field(date_parts.next(), "month")?;
    let day = field(date_parts.next(), "day")?;

    let mut time_parts = time.splitn(3, ':');
    let hour = field(time_parts.next(), "hour")?;
    let minute = field(time_parts.next(), "minute")?;
    let second = field(time_parts.next(), "second")?;

    let days = days_from_civil(year, u32::try_from(month)?, u32::try_from(day)?);
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second;
    Ok(Timestamp(secs * 1000))
}

/// Days since the Unix epoch for a proleptic-Gregorian civil date (Howard
/// Hinnant's well-known constant-time `days_from_civil` formula).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (i64::from(m) + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_root(name: &str) -> camino::Utf8PathBuf {
        let dir = camino::Utf8PathBuf::from(std::env::temp_dir().to_str().unwrap()).join(format!(
            "liminal-reactor-{name}-{}-{:x}",
            std::process::id(),
            Timestamp::now().0
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parse_utc_timestamp_matches_known_epochs() {
        assert_eq!(parse_utc_timestamp("1970-01-01T00:00:00Z").unwrap().0, 0);
        assert_eq!(
            parse_utc_timestamp("2026-01-01T00:00:00Z").unwrap().0,
            1_767_225_600_000
        );
        assert_eq!(
            parse_utc_timestamp("2000-03-01T00:00:00Z").unwrap().0,
            951_868_800_000
        );
    }

    #[test]
    fn parse_utc_timestamp_rejects_non_utc() {
        assert!(parse_utc_timestamp("2026-01-01T00:00:00+01:00").is_err());
        assert!(parse_utc_timestamp("not-a-timestamp").is_err());
    }

    #[test]
    fn inject_observation_materializes_and_records_durable_component() {
        let root = tmp_root("inject");
        let mut ws = ToyWorkspace::open(&root).unwrap();
        let source = SourceId::from_name("stock:acme");
        let obs = Observation {
            source,
            observed_at: parse_utc_timestamp("2026-01-01T00:00:00Z").unwrap(),
            payload: serde_json::json!({"price": 42.17}),
        };
        let revision = ws.inject_observation(obs.clone()).unwrap();
        assert_eq!(revision, ws.store().head().unwrap());

        // One EXTERNAL_VALUE node exists; the blob is exactly the payload.
        let node = ws
            .store()
            .nodes()
            .unwrap()
            .into_iter()
            .find(|n| n.kind == kind::EXTERNAL_VALUE)
            .expect("external-value node must exist");
        let payload_bytes = serde_json::to_vec(&obs.payload).unwrap();
        let hash = ContentHash::of(&payload_bytes);
        let blob = ws
            .store()
            .get_aux(SYS_BLOB, &format!("obs/{source}/{hash}"))
            .unwrap()
            .unwrap();
        assert_eq!(blob, obs.payload);
        let _ = node;
    }

    #[test]
    fn first_observation_materializes_in_one_transaction() {
        let root = tmp_root("one-transaction");
        let mut ws = ToyWorkspace::open(&root).unwrap();
        let before = ws.store().head().unwrap();
        let revision = ws
            .inject_observation(Observation {
                source: SourceId::from_name("stock:acme"),
                observed_at: Timestamp(1),
                payload: serde_json::json!({"price": 42.17}),
            })
            .unwrap();
        assert_eq!(revision.0, before.0 + 1);
    }

    #[test]
    fn durable_observation_survives_workspace_reopen() {
        let root = tmp_root("reopen-observation");
        let source = SourceId::from_name("stock:acme");
        let observed_at = Timestamp(1234);
        let payload = serde_json::json!({"price": 42.17});
        let hash = ContentHash::of(&serde_json::to_vec(&payload).unwrap());
        {
            let mut ws = ToyWorkspace::open(&root).unwrap();
            ws.inject_observation(Observation {
                source,
                observed_at,
                payload,
            })
            .unwrap();
        }

        let reopened = ToyWorkspace::open(&root).unwrap();
        let basis = reopened
            .basis(liminal_revision::BasisPerspective::DurableOnly)
            .unwrap();
        assert_eq!(
            basis.components.get(&JurisdictionKey::Source(source)),
            Some(&BasisComponent::Observation {
                source,
                observed_at,
                hash,
            })
        );
    }

    #[test]
    fn second_observation_of_the_same_source_reuses_the_node() {
        let root = tmp_root("reuse-node");
        let mut ws = ToyWorkspace::open(&root).unwrap();
        let source = SourceId::from_name("stock:acme");
        let at = parse_utc_timestamp("2026-01-01T00:00:00Z").unwrap();
        ws.inject_observation(Observation {
            source,
            observed_at: at,
            payload: serde_json::json!({"price": 42.17}),
        })
        .unwrap();
        ws.inject_observation(Observation {
            source,
            observed_at: at,
            payload: serde_json::json!({"price": 43.00}),
        })
        .unwrap();
        let external_nodes = ws
            .store()
            .nodes()
            .unwrap()
            .into_iter()
            .filter(|n| n.kind == kind::EXTERNAL_VALUE)
            .count();
        assert_eq!(
            external_nodes, 1,
            "toy must reuse the one external-value node"
        );
    }

    #[test]
    fn scripted_resolver_observe_pops_in_order() {
        let source = SourceId::from_name("stock:acme");
        let first = Observation {
            source,
            observed_at: Timestamp(1),
            payload: serde_json::json!({"price": 1.0}),
        };
        let second = Observation {
            source,
            observed_at: Timestamp(2),
            payload: serde_json::json!({"price": 2.0}),
        };
        let mut resolver = ScriptedStockResolver::scripted(vec![first.clone(), second.clone()]);
        assert_eq!(resolver.observe(&source), first);
        assert_eq!(resolver.observe(&source), second);
    }

    #[test]
    fn scripted_resolver_replay_is_pure_and_deterministic() {
        let source = SourceId::from_name("stock:acme");
        let obs = Observation {
            source,
            observed_at: Timestamp(1),
            payload: serde_json::json!({"price": 1.0}),
        };
        let resolver = ScriptedStockResolver::default();
        let frozen = vec![obs.clone()];
        // replay never mutates internal state and never effects — calling it
        // twice against the same frozen log reproduces the same observation.
        assert_eq!(resolver.replay(&source, &frozen), obs);
        assert_eq!(resolver.replay(&source, &frozen), obs);
    }
}
