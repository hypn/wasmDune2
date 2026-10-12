//! AI teams (team.c).

use super::script::Ctx;
use super::*;

impl Game {
    /// GameLoop_Team
    pub fn game_loop_team(&mut self) {
        if self.ticks.team_game_loop > self.timer_game {
            return;
        }
        self.ticks.team_game_loop = self.timer_game + (self.rng.random_256() & 7) as u32 + 5;
        self.script_current_object = None;
        self.script_current_unit = None;
        self.script_current_structure = None;
        let mut find = PoolFind::all();
        while let Some(t) = self.team_find(&mut find) {
            let h = self.teams[t as usize].house_id;
            self.script_current_team = Some(t);
            if !self.h(h).flags.is_ai_active {
                continue;
            }
            if self.teams[t as usize].script.delay != 0 {
                self.teams[t as usize].script.delay -= 1;
                continue;
            }
            if !self.teams[t as usize].script.is_loaded() {
                continue;
            }
            if !self.script_run(Ctx::Team(t)) && !self.config.dune2_enhanced {
                break;
            }
        }
    }

    /// Team_Create
    pub fn team_create(&mut self, house_id: u8, team_action: u16, movement_type: u16, min_members: u16, max_members: u16) -> Option<u16> {
        let t = self.team_allocate(INDEX_INVALID)?;
        {
            let team = &mut self.teams[t as usize];
            team.used = true;
            team.house_id = house_id;
            team.action = team_action;
            team.action_start = team_action;
            team.movement_type = movement_type;
            team.min_members = min_members;
            team.max_members = max_members;
            team.script.reset(ScriptKind::Team);
        }
        self.script_load(Ctx::Team(t), team_action as u8);
        self.teams[t as usize].script.delay = 0;
        Some(t)
    }
}
