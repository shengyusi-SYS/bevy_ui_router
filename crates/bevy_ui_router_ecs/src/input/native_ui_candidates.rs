//! Scope the official UI picking candidates without changing authored content.
//!
//! The original stack is held only across `PickingSystems::Backend` and must
//! be restored before `PickingSystems::Hover`. The presence hint enables work;
//! target eligibility still comes from the shared Router input predicate.

use std::{collections::TryReserveError, mem};

use bevy_ecs::{entity::Entity, prelude::Resource, world::World};
use bevy_ui::UiStack;

#[derive(Debug)]
pub(crate) enum NativeUiInputProjectionFailure {
    InvalidPartition {
        index: usize,
        expected_start: usize,
        start: usize,
        end: usize,
        entity_count: usize,
    },
    IncompleteCoverage {
        covered: usize,
        entity_count: usize,
    },
    Capacity {
        entity_count: usize,
        partition_count: usize,
        source: TryReserveError,
    },
    StackRemovedDuringProjection,
}

impl std::fmt::Display for NativeUiInputProjectionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPartition {
                index,
                expected_start,
                start,
                end,
                entity_count,
            } => write!(
                formatter,
                "UI input partition {index} is {start}..{end}; expected a nonempty range from {expected_start} within {entity_count} entities"
            ),
            Self::IncompleteCoverage {
                covered,
                entity_count,
            } => write!(
                formatter,
                "UI input partitions cover {covered} of {entity_count} entities"
            ),
            Self::Capacity {
                entity_count,
                partition_count,
                source,
            } => write!(
                formatter,
                "UI input projection could not reserve {entity_count} entities and {partition_count} partitions: {source}"
            ),
            Self::StackRemovedDuringProjection => {
                formatter.write_str("UI stack was removed during its input projection")
            }
        }
    }
}

impl std::error::Error for NativeUiInputProjectionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capacity { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Default, Resource)]
pub(crate) struct NativeUiInputProjection {
    presence: bool,
    original: Option<UiStack>,
    scratch: UiStack,
    failure: Option<NativeUiInputProjectionFailure>,
}

impl NativeUiInputProjection {
    /// An optimization hint maintained by the visible-inert presence owner.
    /// It does not authorize a target or release an installed projection.
    pub(crate) fn set_presence(&mut self, presence: bool) {
        self.presence = presence;
    }

    /// The coordinator consumes a failure to end the affected visual hold.
    /// Retaining one failure prevents repeated preparation from growing a log.
    pub(crate) fn take_failure(&mut self) -> Option<NativeUiInputProjectionFailure> {
        self.failure.take()
    }
}

/// Run after Router input refresh and before the official picking Backend.
pub(crate) fn project_native_ui_input_stack(world: &mut World) {
    let Some(projection) = world.get_resource::<NativeUiInputProjection>() else {
        return;
    };
    // No presence means no UiStack read, registry lookup, or candidate scan.
    if !projection.presence || projection.original.is_some() {
        return;
    }

    world.resource_scope::<NativeUiInputProjection, _>(|world, mut projection| {
        let Some(source) = world.get_resource::<UiStack>() else {
            return;
        };
        let mut observation = super::RouterInputObservation::new(world);
        match rebuild_without(source, &mut projection.scratch, |entity| {
            observation
                .as_mut()
                .is_some_and(|observation| observation.target_is_denied(entity))
        }) {
            Ok(false) => return,
            Err(failure) => {
                projection.failure = Some(failure);
                return;
            }
            Ok(true) => {}
        }

        // All allocation and candidate classification happened before this
        // swap. Keep the exact source for this Backend interval, not a prior
        // frame's ordering or a reconstruction from the filtered list.
        let filtered = mem::take(&mut projection.scratch);
        let mut stack = world.resource_mut::<UiStack>();
        projection.original = Some(mem::replace(&mut *stack, filtered));
    });
}

/// Run after Backend and before Hover, without the presence run condition.
pub(crate) fn restore_native_ui_input_stack(world: &mut World) {
    if world
        .get_resource::<NativeUiInputProjection>()
        .is_none_or(|projection| projection.original.is_none())
    {
        return;
    }

    world.resource_scope::<NativeUiInputProjection, _>(|world, mut projection| {
        let Some(original) = projection.original.take() else {
            return;
        };
        let Some(mut stack) = world.get_resource_mut::<UiStack>() else {
            // Do not resurrect an application-removed resource. The original
            // is derived UI data, not a Router lifecycle capability.
            projection.failure = Some(NativeUiInputProjectionFailure::StackRemovedDuringProjection);
            return;
        };
        projection.scratch = mem::replace(&mut *stack, original);
    });
}

/// Build a filtered view without mutating the source. Preserve entity order
/// and camera partitions; omit empty partitions because the official backend
/// obtains each partition's camera from its first entity.
fn rebuild_without(
    source: &UiStack,
    scratch: &mut UiStack,
    mut excluded: impl FnMut(Entity) -> bool,
) -> Result<bool, NativeUiInputProjectionFailure> {
    let mut covered = 0;
    for (index, range) in source.partition.iter().enumerate() {
        if range.start != covered || range.end <= range.start || range.end > source.uinodes.len() {
            return Err(NativeUiInputProjectionFailure::InvalidPartition {
                index,
                expected_start: covered,
                start: range.start,
                end: range.end,
                entity_count: source.uinodes.len(),
            });
        }
        covered = range.end;
    }
    if covered != source.uinodes.len() {
        return Err(NativeUiInputProjectionFailure::IncompleteCoverage {
            covered,
            entity_count: source.uinodes.len(),
        });
    }

    scratch.uinodes.clear();
    scratch.partition.clear();
    let capacity_failure = |source_error| NativeUiInputProjectionFailure::Capacity {
        entity_count: source.uinodes.len(),
        partition_count: source.partition.len(),
        source: source_error,
    };
    scratch
        .uinodes
        .try_reserve(source.uinodes.len())
        .map_err(capacity_failure)?;
    scratch
        .partition
        .try_reserve(source.partition.len())
        .map_err(capacity_failure)?;

    let mut changed = false;
    for range in &source.partition {
        let start = scratch.uinodes.len();
        for entity in source.uinodes[range.clone()].iter().copied() {
            if excluded(entity) {
                changed = true;
            } else {
                scratch.uinodes.push(entity);
            }
        }
        let end = scratch.uinodes.len();
        if start != end {
            scratch.partition.push(start..end);
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_projection_preserves_order_and_camera_partitions() {
        let entities = (1..=5)
            .map(|index| Entity::from_raw_u32(index).unwrap())
            .collect::<Vec<_>>();
        let source = UiStack {
            uinodes: entities.clone(),
            partition: vec![0..2, 2..3, 3..5],
        };
        let mut scratch = UiStack::default();
        assert!(
            rebuild_without(&source, &mut scratch, |entity| {
                [entities[0], entities[2], entities[4]].contains(&entity)
            })
            .unwrap()
        );
        assert_eq!(scratch.uinodes, [entities[1], entities[3]]);
        assert_eq!(scratch.partition, [0..1, 1..2]);
        assert_eq!(source.uinodes, entities);
        assert_eq!(source.partition, [0..2, 2..3, 3..5]);

        assert!(!rebuild_without(&source, &mut scratch, |_| false).unwrap());
        assert_eq!(scratch.uinodes, source.uinodes);
        assert_eq!(scratch.partition, source.partition);
        assert!(rebuild_without(&source, &mut scratch, |_| true).unwrap());
        assert!(scratch.uinodes.is_empty() && scratch.partition.is_empty());
    }
}

#[cfg(all(test, feature = "bsn"))]
#[path = "native_ui_candidates/tests.rs"]
mod native_input_tests;
