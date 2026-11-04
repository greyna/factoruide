//! The spot where an objective's normal mobs (or objects) are farmed: how many live there, how
//! many come in each pull, and the wait when the objective asks for more than the spot holds,
//! shared with the other players of a crowded server, quickened by them and the group's own
//! members. Measured once, when the model is built.

use super::builder::QuestBuilder;
use super::guard::Guards;
use super::sources::QuestRow;
use super::types::{EntityKind, Objective};
use crate::world::Pos;

/// Spawns this close to an objective's place are its spot (yards).
const SPOT: f64 = 200.0;

impl QuestBuilder<'_> {
    pub(super) fn spots_and_crowd(&self, r: &QuestRow, objectives: &mut [Objective]) {
        let p = self.params;
        let level = r.level.unwrap_or(1).max(1);
        let crowd = p.crowd_at(level);
        for o in objectives.iter_mut().filter(|o| o.dungeon_of().is_none()) {
            if o.loc.kind == EntityKind::Area && is_event(r, &o.text) {
                o.extra += crowd * p.crowd_event_time;
                continue;
            }
            let (work, each, kind) = if o.kills > 0.0 && !o.elite && !o.mobs.is_empty() {
                let points = self.spot_points(o, EntityKind::Npc);
                o.spawns = points.len() as u32;
                if !points.is_empty() {
                    let pulls: u32 = points.iter().map(|q| self.sources.guards.pull_at(q)).sum();
                    o.pack = f64::from(pulls) / points.len() as f64;
                }
                (o.kills, p.kill_time(level, 0.0, o.mob_level), EntityKind::Npc)
            } else if o.uses > 0.0 && o.loc.kind == EntityKind::Object {
                o.spawns = self.spot_points(o, EntityKind::Object).len() as u32;
                (o.uses, p.object_time, EntityKind::Object)
            } else {
                continue;
            };
            let crowd = Crowd {
                others: crowd * p.crowd_players,
                mates: (p.group_size.clamp(1, 5) - 1) as f64,
                dynamic: p.dynamic_respawn,
            };
            o.extra += respawn_wait(work, f64::from(o.spawns), each, self.respawn(o, kind), crowd);
        }
    }

    /// Seconds the objective's mobs (or its object) take to come back: the mean of those the
    /// data knows (Meven Korgal: a minute), else `Params::respawn_time`.
    fn respawn(&self, o: &Objective, kind: EntityKind) -> f64 {
        let ids: Vec<i64> = if kind == EntityKind::Npc {
            o.mobs.iter().map(|(id, _)| *id).collect()
        } else {
            vec![o.loc.id]
        };
        let known: Vec<f64> = (ids.iter())
            .filter_map(|id| self.sources.entities.respawn.get(&(kind, *id)).copied())
            .collect();
        if known.is_empty() {
            self.params.respawn_time
        } else {
            known.iter().sum::<f64>() / known.len() as f64
        }
    }

    /// Spawn points of the objective's mobs (or its object) at its spot, those at one point
    /// counted once.
    fn spot_points(&self, o: &Objective, kind: EntityKind) -> Vec<Pos> {
        let ids: Vec<i64> = if kind == EntityKind::Npc {
            o.mobs.iter().map(|(id, _)| *id).collect()
        } else {
            vec![o.loc.id]
        };
        let mut points: Vec<Pos> = Vec::new();
        for id in ids {
            for (q, _) in self.sources.entities.points(self.world, kind, id) {
                if q.dist(&o.loc.pos) <= SPOT && !points.iter().any(|x| x.dist(&q) < Guards::ONE_POINT) {
                    points.push(q);
                }
            }
        }
        points
    }
}

/// The other players at a spot, the character's group mates, and how much the game shortens
/// respawns for each player around (`Params::dynamic_respawn`). The mates do not share the spawns:
/// their kills, and the items each of them loots, are already the objective's work.
#[derive(Clone, Copy)]
struct Crowd {
    others: f64,
    mates: f64,
    dynamic: f64,
}

/// Time waiting for respawns: `work` kills (or uses) of `each` seconds at a spot of `spawns`
/// coming back in `respawn` seconds, shared with the crowd. Once the spot is cleared, one comes
/// back every `respawn` / spawns seconds, sooner with players around (divided by 1 + dynamic x
/// the others and the mates); the wait is what that leaves beyond the kill itself. Nothing when
/// the spot is unknown.
fn respawn_wait(work: f64, spawns: f64, each: f64, respawn: f64, crowd: Crowd) -> f64 {
    if spawns <= 0.0 {
        return 0.0;
    }
    let mine = spawns / (1.0 + crowd.others);
    if work <= mine {
        return 0.0;
    }
    let respawn = respawn / (1.0 + crowd.dynamic * (crowd.others + crowd.mates));
    (respawn / mine - each).max(0.0) * (work - mine)
}

/// An escort or an event (defend someone, overhear a talk): the quest is flagged as one, or its
/// objective says so.
fn is_event(r: &QuestRow, text: &str) -> bool {
    r.flags.is_some_and(|f| f & 2 != 0) || ["Escort", "Protect", "Defend"].iter().any(|w| text.starts_with(w))
}

#[cfg(test)]
mod tests {
    use super::{Crowd, respawn_wait};

    const ALONE: Crowd = Crowd {
        others: 0.0,
        mates: 0.0,
        dynamic: 0.5,
    };
    const FIXED: Crowd = Crowd {
        others: 3.0,
        mates: 0.0,
        dynamic: 0.0,
    };

    /// Enough mobs: no wait; a small spot: the respawns are waited for, longer when shared.
    #[test]
    fn small_spots_wait_for_respawns() {
        assert!(respawn_wait(10.0, 30.0, 25.0, 300.0, ALONE).abs() < 1e-9);
        let alone = respawn_wait(22.0, 7.0, 25.0, 300.0, ALONE);
        let shared = respawn_wait(22.0, 7.0, 25.0, 300.0, FIXED);
        assert!(alone > 0.0 && shared > 3.0 * alone, "{alone} {shared}");
        assert!(respawn_wait(22.0, 0.0, 25.0, 300.0, FIXED).abs() < 1e-9, "unknown spot");
    }

    /// Meven Korgal (GitHub #20): one spawn back in a minute, killed once on a launch day. With
    /// every mob at 5 minutes and fixed respawns the wait was a quarter of an hour; his own minute
    /// shortened for the crowd leaves about one.
    #[test]
    fn a_lone_mob_respawning_fast_on_a_crowded_server_is_a_short_wait() {
        let launch = Crowd {
            others: 3.0,
            mates: 0.0,
            dynamic: 0.5,
        };
        let before = respawn_wait(1.0, 1.0, 25.0, 300.0, FIXED);
        let now = respawn_wait(1.0, 1.0, 25.0, 60.0, launch);
        assert!(before > 800.0, "{before}");
        assert!((20.0..120.0).contains(&now), "{now}");
    }

    /// Respawns quicken with the crowd: the wait grows with the players less than with fixed
    /// respawns, and not at all beyond the first share when they keep up (dynamic 1).
    #[test]
    fn faster_respawns_with_players_around_soften_the_crowded_wait() {
        let at = |others: f64, dynamic: f64| {
            respawn_wait(
                30.0,
                10.0,
                25.0,
                300.0,
                Crowd {
                    others,
                    mates: 0.0,
                    dynamic,
                },
            )
        };
        let (alone, fixed, dynamic) = (at(0.0, 0.5), at(3.0, 0.0), at(3.0, 0.5));
        assert!(alone < dynamic && dynamic < fixed, "{alone} {dynamic} {fixed}");
        // Keeping up: each comes back as often for each player as for one alone.
        let rate = |others: f64| at(others, 1.0) / (30.0 - 10.0 / (1.0 + others));
        assert!((rate(3.0) - rate(0.0)).abs() < 1e-9, "{} {}", rate(3.0), rate(0.0));
    }

    /// A trio at a small spot: its two mates bring the mobs back sooner without sharing them, so
    /// each respawn is waited for less than alone; with no mates, the wait is the same as before
    /// groups counted.
    #[test]
    fn a_group_quickens_the_respawns_of_its_spot() {
        let with = |mates: f64| Crowd { mates, ..ALONE };
        let (solo, trio) = (
            respawn_wait(22.0, 7.0, 10.0, 300.0, with(0.0)),
            respawn_wait(22.0, 7.0, 10.0, 300.0, with(2.0)),
        );
        assert!(0.0 < trio && trio < solo, "{solo} {trio}");
        // Before: the wait shared with nobody, respawns of 300 s.
        let before = (300.0 / 7.0 - 10.0) * (22.0 - 7.0);
        assert!((solo - before).abs() < 1e-9, "{solo} {before}");
        // The mates do not take the spot's mobs: the first 7 kills still wait for nothing.
        assert!(respawn_wait(7.0, 7.0, 10.0, 300.0, with(2.0)).abs() < 1e-9);
    }
}
