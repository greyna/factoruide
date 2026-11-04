//! A group guide: one route for several players, each starting from what its addon recorded. It
//! is planned for the one with the least XP (`PlanRequest::members`); the others follow the same
//! stops, each getting the XP of the quests still for it.

use super::StartState;
use crate::model::{MateStart, Model, still_for};
use crate::world::World;
use std::collections::HashSet;

/// The other members joined to the character's initial state (already in `model.initial`):
/// which quests are still for whom, the log they share, the flights they all know, whether their
/// hearthstones are bound to the same inn.
pub(super) fn join(
    model: &mut Model,
    world: &World,
    character: &StartState,
    mates: &[&StartState],
    notes: &mut Vec<String>,
) {
    let init = &mut model.initial;
    init.mates = mates
        .iter()
        .map(|m| MateStart {
            level: m.level,
            xp: m.xp,
            rested: m.rested,
            completed: m.completed.iter().map(|q| q.0).collect(),
        })
        .collect();
    for m in mates {
        init.known &= m.known_flights(world);
    }
    let bind = character.bind_inn(model, world);
    if bind.is_none() || mates.iter().any(|m| m.bind_inn(model, world) != bind) {
        model.initial.unbound = true;
        notes.push("hearthstones bound to different inns: the group binds them together first".into());
    }
    let init = &model.initial;
    let need: Vec<u8> = (model.quests.iter())
        .map(|q| {
            still_for(
                q.members,
                mates.len() + 1,
                q.id,
                &q.exclusive,
                q.breadcrumb_for,
                |m, id| init.done_by(m, id),
            )
        })
        .collect();
    // In the log of everyone it is for: the group carries on with it, as far as the character
    // got. The others are taken again by the group (who has one already just goes along).
    let logs: Vec<HashSet<i64>> = mates.iter().map(|m| m.log.iter().map(|q| q.id.0).collect()).collect();
    let shared = |i: usize| {
        let id = model.quests[i].id;
        need[i] & 1 != 0 && (0..mates.len()).all(|k| need[i] & (2 << k) == 0 || logs[k].contains(&id))
    };
    let init = &mut model.initial;
    init.accepted.retain(|&i| shared(i));
    init.progress.retain(|p| init.accepted.contains(&p.0));
    let index = &model.index;
    init.log.retain(|(id, _)| {
        index
            .get(id)
            .is_none_or(|&i| init.accepted.contains(&i) || need[i] == 0)
    });
    init.turned = (0..need.len()).filter(|&i| need[i] == 0).collect();
    init.need = need;
}
