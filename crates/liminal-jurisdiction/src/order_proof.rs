//! Fallible, lossless host storage for ordering checks (v4 §7.7–7.8; DG17.6).
//!
//! Storage construction is not validation or authority. Invalid identities,
//! duplicate edges and arbitrary supplied schedules remain visible to the leaf.

use std::alloc::Layout;

use liminal_id::RepairStepId;

use crate::{RepairOperation, RepairPlan};

/// New proof storage could not be represented or allocated (DG17.6).
/// This error owns no message or collection; existing allocations are unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("ordering proof resources exhausted")]
pub struct ResourceExhaustion;

/// Borrowed input facts, not a validated schedule or an authorization capability.
#[derive(Debug, Clone, Copy)]
pub struct OrderProofView<'a> {
    /// Map key, declared step identity, and whether the operation is graph-native.
    pub steps: &'a [(u128, u128, bool)],
    /// Before/after endpoints in proposal order, retaining duplicates.
    pub dependencies: &'a [(u128, u128)],
    /// The supplied original schedule, without normalization.
    pub schedule: &'a [u128],
    /// The supplied applied schedule, without normalization.
    pub applied: &'a [u128],
}

/// Owned temporary copies of existing identities and ordering facts (v4 §7.7).
/// No semantic object, acceptance decision, or proof is created by this type.
#[derive(Debug)]
pub struct OrderProofInput {
    steps: Vec<(u128, u128, bool)>,
    dependencies: Vec<(u128, u128)>,
    schedule: Vec<u128>,
    applied: Vec<u128>,
}

impl OrderProofInput {
    /// Copy proposal facts and both actual schedules using fallible reservations.
    /// The caller must perform its legacy validations before this new allocation
    /// when their refusal precedence matters. This method does not validate them.
    pub fn try_new(
        plan: &RepairPlan,
        schedule: &[RepairStepId],
        applied: &[RepairStepId],
    ) -> Result<Self, ResourceExhaustion> {
        // Validate every layout and their aggregate before the first allocation.
        // Layout checks multiplication, alignment and the isize object-size bound.
        let sizes = [
            Layout::array::<(u128, u128, bool)>(plan.steps.len()),
            Layout::array::<(u128, u128)>(plan.dependencies.len()),
            Layout::array::<u128>(schedule.len()),
            Layout::array::<u128>(applied.len()),
        ];
        sizes.into_iter().try_fold(0usize, |total, layout| {
            total
                .checked_add(layout.map_err(|_| ResourceExhaustion)?.size())
                .ok_or(ResourceExhaustion)
        })?;

        let mut result = Self {
            steps: Vec::new(),
            dependencies: Vec::new(),
            schedule: Vec::new(),
            applied: Vec::new(),
        };
        result
            .steps
            .try_reserve_exact(plan.steps.len())
            .map_err(|_| ResourceExhaustion)?;
        result
            .dependencies
            .try_reserve_exact(plan.dependencies.len())
            .map_err(|_| ResourceExhaustion)?;
        result
            .schedule
            .try_reserve_exact(schedule.len())
            .map_err(|_| ResourceExhaustion)?;
        result
            .applied
            .try_reserve_exact(applied.len())
            .map_err(|_| ResourceExhaustion)?;

        // Each immutable input yields exactly the reserved number of elements.
        // These pushes cannot trigger growth; no map or operation payload is cloned.
        for (key, step) in &plan.steps {
            result.steps.push((
                key.as_uuid().as_u128(),
                step.id.as_uuid().as_u128(),
                matches!(step.operation, RepairOperation::Graph(_)),
            ));
        }
        for edge in &plan.dependencies {
            result.dependencies.push((
                edge.before.as_uuid().as_u128(),
                edge.after.as_uuid().as_u128(),
            ));
        }
        for id in schedule {
            result.schedule.push(id.as_uuid().as_u128());
        }
        for id in applied {
            result.applied.push(id.as_uuid().as_u128());
        }
        Ok(result)
    }

    /// Borrow all copied facts for the allocation-free checker.
    #[must_use]
    pub fn view(&self) -> OrderProofView<'_> {
        OrderProofView {
            steps: &self.steps,
            dependencies: &self.dependencies,
            schedule: &self.schedule,
            applied: &self.applied,
        }
    }
}
