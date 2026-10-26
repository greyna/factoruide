# Spec: group loot rules for quest objectives

## Context

In a group, every kill counts for every member, but a quest item that drops goes to one player only. A group of n players therefore needs n times the kills for a loot objective, and n times the clicks for an object objective, while a kill objective still needs its count once.

Factoruide already models the drop rule as one global setting, `group_shared_drops`, on by default (every member gets the drop). This feature makes the planner, the app and the addon follow the one-player rule when that setting is off, and turns it off for new settings.

## Today's behaviour (v0.14.0)

Kill objectives are already right; loot objectives follow the one-player rule only when `group_shared_drops` is off, and objects never do.

| Piece | Where | What it does today |
| --- | --- | --- |
| Group damage | `params.rs` `group_damage()` | Kill time divided by 1 + `group_kill_speed` × (n − 1), 0.8 by default: a duo kills 1.8× faster, a trio 2.6×. Applies to every kill. |
| Kill objectives | `model/objectives.rs` | `kills` = the count, once for the group. |
| Loot objectives | `model/builder.rs` `per_player`, `model/item.rs` | `kills` = count ÷ drop chance × `per_player`; `per_player` = 1 when shared, n otherwise. Also used for the elite-kill threshold (`solo_elite_kills`). |
| Object objectives | `model/item.rs` (object items) | `uses` = count ÷ object chance, never multiplied by n. |
| Spot and respawns | `model/crowd.rs` `respawn_wait` | Waits when `work` exceeds the spot's spawns; shares the spot with `crowded` × `crowd_players` other players and speeds respawns by 1 + `dynamic_respawn` × others. Group members are not counted. |
| Mob XP | `group_xp_share()` | The game's group bonus split between members. |
| Setting | `app/src/paramSchema.ts`, section Groupe | `group_shared_drops` checkbox, on by default (`Params::default`, `#[serde(default)]`). |

With the setting off, a loot objective in a duo takes 2 ÷ 1.8 = 1.11× the solo time and in a trio 3 ÷ 2.6 = 1.15×, versus 0.56× and 0.38× for a kill objective.

## Decisions

D3, D4, D5 and D8 apply when `group_shared_drops` is off and the group has 2 to 5 players; D6 and D7 apply to any group. Solo guides do not change.

| # | Topic | Rule | Decided |
| --- | --- | --- | --- |
| D1 | Setting | One global setting decides the drop rule, kept in the Groupe section of the settings. | No per-item or per-quest switch. |
| D2 | Setting | The default becomes off for new settings; settings already saved keep their value. | See Implementation ideas (Default). |
| D3 | Loot per member | Each quest item from a mob drops for one player only: loot kills × n. | Same for every edition (Forever, Classic Era, TBC Anniversary). |
| D4 | Loot per member | Exception to D3: an item from a mob with a single spawn point (a named mob: a head, a key) needs one kill for the whole group, whatever its count. | Recognised by the mob's spawn points, not by the item count or a rare flag. |
| D5 | Loot per member | Quest items picked up from objects (plants, crates, chests): each member clicks their own, uses × n. | New. |
| D6 | Group speed | Kills stay faster in a group (`group_kill_speed`, 80%). A loot objective is n ÷ group damage slower, not n× slower. | Formula kept, no new speed setting. |
| D7 | Group speed | The group counts at every spot, for kill, loot and object objectives: its members speed up respawns like other players (`dynamic_respawn`) and the spot runs out sooner. | The members count for faster respawns only, not among the players sharing the spawns: the faster kills and the n× work already empty the spot sooner. |
| D8 | Display | A loot or object step shows the group's work, e.g. "10 each, \~30 kills for the group", on the app's guide page and in the addon. | App and addon. |

## Implementation ideas

Pointers for the implementer, who chooses the design. Most of the change probably sits where the model builds objectives (`crates/fg-route/src/model`): with the right kills and uses there, the planner, the optimizer and the guide follow.

- **Default** (D2): `group_shared_drops` is set in `Params::default` (`params.rs`). The app's form takes its defaults from `options.params` and saved guides store their whole `params`, so changing the default should leave old guides as they are. No `overrides/*/routing.toml` sets it.
- **Loot** (D3, D4): `model/builder.rs` computes `per_player` (1 when shared or solo, n otherwise), used by `model/item.rs` for mob drops, approximate sources and the solo elite threshold. The named-mob exception could count the droppers' spawn points the way `model/crowd.rs` `spot_points` does.
- **Objects** (D5): object items get `uses = count / object_chance` in `model/item.rs`, never multiplied today. Only objects whose item goes to the bags should multiply, not levers or braziers.
- **Kills** (D6): kill objectives come from `model/objectives.rs`; every kill time is divided by `Params::group_damage()`.
- **Group at the spot** (D7): `model/crowd.rs` `respawn_wait` already shares a spot with a crowded server's players and speeds respawns with `dynamic_respawn`. Counting the members among the players sharing the spawns would count the group twice (D7), since faster kills and the n× work already empty the spot sooner.
- **Guide text** (D8): `export/writer.rs` `objective_phrase` writes a step's phrases, from templates in `export/templates.rs` (9 languages). The app's guide page and the addon both show them.
- **Setting text**: `app/src/paramSchema.ts` (Groupe section); the CI checks that every `t()` text is in the catalog.

## Tests and acceptance

The feature is done when the unit tests below pass, solo guides come out unchanged, and a trio guide with the setting off shows the expected kills and texts.

**Unit tests** (next to the code, `cargo test -p fg-route`)

- `item.rs`: a loot objective of 10 items at 50% gives 20 kills solo, 60 in a trio with the setting off, 20 with it on.
- `item.rs`: the same item from a mob with one spawn point gives 20 kills in a trio, setting off (D4).
- `item.rs`: an object item of 8 gives 8 uses solo and 24 in a trio, setting off; a lever used once stays 1 (D5).
- `crowd.rs`: `respawn_wait` with a trio waits less per respawn than solo for the same work (D7), and the same as today with n = 1.
- `writer.rs`: a trio loot step has the `objective.group` phrase with `n` 10 and `total` 60; a solo step and a shared-drops step do not.
- `params.rs`: `Params::default().group_shared_drops` is false; a request JSON with `"group_shared_drops": true` keeps it.

**Acceptance runs** (with `fg route`, same seed)

- [ ] A solo guide's route and time are identical before and after the change.
- [ ] A trio guide, setting off: loot quests drop down the route or out of it versus setting on, kill quests stay; the time per level shown is higher than with the setting on.
- [ ] A trio guide, setting off: a named-mob item (e.g. a head) is one kill, not three.
- [ ] The app's guide page and the addon show "10 each, \~30 kills for the group" on a loot step, in English and French.
- [ ] Regenerating a guide saved before the change keeps its setting (on).
- [ ] `scripts/check.sh` passes (fmt, clippy, tests, catalog check).

## Open points and risks

One point needs checking before coding (O1); the others are risks to watch during the acceptance runs.

| # | Point | Proposal |
| --- | --- | --- |
| O1 | Telling an object that gives a quest item (×n) from one that is only used (×1) depends on how `item.rs` and `objectives.rs` mark them. | Multiply only objectives whose target is an item (`EntityKind::Item` with object sources). To check in the data on 5 to 10 quests. |
| O2 | Spawn data is incomplete: a mob with several spawns in game but one in the database would be counted as named (×1). | Accept; the contributions (positions recorded by the addon) fix the data over time. |
| O3 | A guide made by the new app (shared as a .fgguide file) opened in an older app: the older app does not know the new step text objective.group. | Seen in `export/format.rs` and templates.rs: an unknown text shows as its key, so the older app shows a line "objective.group" and nothing breaks. Accept it, or raise FORMAT so the older app asks to be updated. |
| O4 | The extra kills of loot objectives also give mob XP (`kills × mob_xp`), which offsets part of the slowdown. | Expected and correct: keep it. |
| O5 | Kill credit needs the members near the kill; the planner assumes the group stays together. | Out of scope. |
