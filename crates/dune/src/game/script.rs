//! The EMC script virtual machine (script/script.c).

use super::{Game, ObjRef, ScriptEngine, ScriptKind};

/// A loaded EMC file (UNIT.EMC, BUILD.EMC or TEAM.EMC).
#[derive(Clone, Debug, Default)]
pub struct ScriptInfo {
    /// TEXT chunk (big-endian word offsets followed by strings).
    pub text: Vec<u8>,
    /// ORDR chunk: start offset per type.
    pub offsets: Vec<u16>,
    /// DATA chunk: big-endian opcodes.
    pub start: Vec<u16>,
}

impl ScriptInfo {
    pub fn load(data: &[u8]) -> Result<ScriptInfo, String> {
        if data.len() < 12 || &data[0..4] != b"FORM" {
            return Err("EMC is not a FORM file".into());
        }
        let mut info = ScriptInfo::default();
        let mut p = 12;
        while p + 8 <= data.len() {
            let id = &data[p..p + 4];
            let len = u32::from_be_bytes(data[p + 4..p + 8].try_into().unwrap()) as usize;
            let body = &data[p + 8..(p + 8 + len).min(data.len())];
            match id {
                b"TEXT" => info.text = body.to_vec(),
                b"ORDR" => info.offsets = body.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect(),
                b"DATA" => info.start = body.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect(),
                _ => {}
            }
            p += 8 + ((len + 1) & !1);
        }
        if info.offsets.is_empty() || info.start.is_empty() {
            return Err("EMC without ORDR/DATA".into());
        }
        Ok(info)
    }

    /// The text string referenced by index (Script_General_DisplayText).
    pub fn text_string(&self, index: u16) -> Vec<u8> {
        let i = index as usize * 2;
        if i + 1 >= self.text.len() {
            return Vec::new();
        }
        let off = u16::from_be_bytes([self.text[i], self.text[i + 1]]) as usize;
        self.text[off.min(self.text.len())..].iter().take_while(|&&c| c != 0).copied().collect()
    }
}

/// Identifies which engine a script function operates on (the original's g_scriptCurrent*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ctx {
    Unit(u16),
    Structure(u16),
    Team(u16),
}

impl ScriptEngine {
    /// Script_Reset
    pub fn reset(&mut self, info: ScriptKind) {
        self.script = None;
        self.info = info;
        self.is_subroutine = 0;
        self.frame_pointer = 17;
        self.stack_pointer = 15;
    }

    /// Script_Stack_Push
    pub fn push(&mut self, value: u16) {
        if self.stack_pointer == 0 {
            self.script = None;
            return;
        }
        self.stack_pointer -= 1;
        self.stack[self.stack_pointer as usize] = value;
    }

    /// Script_Stack_Pop
    pub fn pop(&mut self) -> u16 {
        if self.stack_pointer >= 15 {
            self.script = None;
            return 0;
        }
        let v = self.stack[self.stack_pointer as usize];
        self.stack_pointer += 1;
        v
    }

    /// Script_Stack_Peek
    pub fn peek(&mut self, position: u8) -> u16 {
        if self.stack_pointer as i32 >= 16 - position as i32 {
            self.script = None;
            return 0;
        }
        self.stack[(self.stack_pointer + position - 1) as usize]
    }

    /// Script_IsLoaded
    pub fn is_loaded(&self) -> bool {
        self.script.is_some() && self.info != ScriptKind::None
    }
}

impl Game {
    pub fn script_info(&self, kind: ScriptKind) -> &ScriptInfo {
        match kind {
            ScriptKind::Unit => &self.script_unit,
            ScriptKind::Structure => &self.script_structure,
            ScriptKind::Team | ScriptKind::None => &self.script_team,
        }
    }

    pub fn engine(&mut self, ctx: Ctx) -> &mut ScriptEngine {
        match ctx {
            Ctx::Unit(i) => &mut self.units[i as usize].o.script,
            Ctx::Structure(i) => &mut self.structures[i as usize].o.script,
            Ctx::Team(i) => &mut self.teams[i as usize].script,
        }
    }

    /// STACK_PEEK(position) on the engine of the current script.
    pub fn peek(&mut self, ctx: Ctx, position: u8) -> u16 {
        self.engine(ctx).peek(position)
    }

    pub fn script_reset_obj(&mut self, o: ObjRef, kind: ScriptKind) {
        self.obj_mut(o).script.reset(kind);
    }

    /// Script_Load
    pub fn script_load(&mut self, ctx: Ctx, type_id: u8) {
        let kind = self.engine(ctx).info;
        if kind == ScriptKind::None {
            return;
        }
        let off = self.script_info(kind).offsets.get(type_id as usize).copied().unwrap_or(0);
        let e = self.engine(ctx);
        e.reset(kind);
        e.script = Some(off);
    }

    /// Script_LoadAsSubroutine
    pub fn script_load_as_subroutine(&mut self, ctx: Ctx, type_id: u8) {
        if !self.engine(ctx).is_loaded() || self.engine(ctx).is_subroutine != 0 {
            return;
        }
        let kind = self.engine(ctx).info;
        let off = self.script_info(kind).offsets.get(type_id as usize).copied().unwrap_or(0);
        let e = self.engine(ctx);
        e.is_subroutine = 1;
        let ip = e.script.unwrap();
        e.push(ip);
        let rv = e.return_value;
        e.push(rv);
        e.script = Some(off);
    }

    /// Script_Run: execute one instruction. Returns false on error / not loaded.
    pub fn script_run(&mut self, ctx: Ctx) -> bool {
        if !self.engine(ctx).is_loaded() {
            return false;
        }
        let kind = self.engine(ctx).info;
        let ip = self.engine(ctx).script.unwrap();
        let start_len = self.script_info(kind).start.len() as u16;
        if ip >= start_len {
            self.engine(ctx).script = None;
            return false;
        }
        let current = self.script_info(kind).start[ip as usize];
        let mut ip = ip + 1;
        let mut opcode = ((current >> 8) & 0x1F) as u8;
        let mut parameter: u16 = 0;
        if current & 0x8000 != 0 {
            opcode = 0;
            parameter = current & 0x7FFF;
        } else if current & 0x4000 != 0 {
            parameter = (current & 0xFF) as u8 as i8 as i16 as u16;
        } else if current & 0x2000 != 0 {
            parameter = self.script_info(kind).start.get(ip as usize).copied().unwrap_or(0);
            ip += 1;
        }
        self.engine(ctx).script = Some(ip);

        let e = self.engine(ctx);
        match opcode {
            0 => {
                // JUMP
                e.script = Some(parameter);
                true
            }
            1 => {
                e.return_value = parameter;
                true
            }
            2 => {
                if parameter == 0 {
                    let rv = e.return_value;
                    e.push(rv);
                    return true;
                }
                if parameter == 1 {
                    let location = ip.wrapping_add(1);
                    e.push(location);
                    let fp = e.frame_pointer as u16;
                    e.push(fp);
                    e.frame_pointer = e.stack_pointer.wrapping_add(2);
                    return true;
                }
                e.script = None;
                false
            }
            3 | 4 => {
                e.push(parameter);
                true
            }
            5 => {
                let v = e.variables[(parameter & 0xFF) as usize % 5];
                e.push(v);
                true
            }
            6 => {
                let idx = e.frame_pointer as i32 - parameter as i16 as i32 - 2;
                if !(0..15).contains(&idx) {
                    e.script = None;
                    return false;
                }
                let v = e.stack[idx as usize];
                e.push(v);
                true
            }
            7 => {
                let idx = e.frame_pointer as i32 + parameter as i16 as i32 - 1;
                if !(0..15).contains(&idx) {
                    e.script = None;
                    return false;
                }
                let v = e.stack[idx as usize];
                e.push(v);
                true
            }
            8 => {
                if parameter == 0 {
                    e.return_value = e.pop();
                    return true;
                }
                if parameter == 1 {
                    e.peek(2);
                    if e.script.is_none() {
                        return false;
                    }
                    e.frame_pointer = e.pop() as u8;
                    let loc = e.pop();
                    e.script = Some(loc);
                    return true;
                }
                e.script = None;
                false
            }
            9 => {
                let v = e.pop();
                e.variables[(parameter & 0xFF) as usize % 5] = v;
                true
            }
            10 => {
                let idx = e.frame_pointer as i32 - parameter as i16 as i32 - 2;
                if !(0..15).contains(&idx) {
                    e.script = None;
                    return false;
                }
                let v = e.pop();
                e.stack[idx as usize] = v;
                true
            }
            11 => {
                let idx = e.frame_pointer as i32 + parameter as i16 as i32 - 1;
                if !(0..15).contains(&idx) {
                    e.script = None;
                    return false;
                }
                let v = e.pop();
                e.stack[idx as usize] = v;
                true
            }
            12 => {
                e.stack_pointer = e.stack_pointer.wrapping_add(parameter as u8);
                true
            }
            13 => {
                e.stack_pointer = e.stack_pointer.wrapping_sub(parameter as u8);
                true
            }
            14 => {
                let f = parameter & 0xFF;
                let rv = self.script_call_function(kind, f, ctx);
                self.engine(ctx).return_value = rv;
                true
            }
            15 => {
                e.peek(1);
                if e.script.is_none() {
                    return false;
                }
                if e.pop() != 0 {
                    return true;
                }
                e.script = Some(parameter & 0x7FFF);
                true
            }
            16 => {
                match parameter {
                    0 => {
                        let v = e.pop();
                        e.push(if v == 0 { 1 } else { 0 });
                    }
                    1 => {
                        let v = e.pop();
                        e.push(v.wrapping_neg());
                    }
                    2 => {
                        let v = e.pop();
                        e.push(!v);
                    }
                    _ => {
                        e.script = None;
                        return false;
                    }
                }
                true
            }
            17 => {
                let right = e.pop() as i16;
                let left = e.pop() as i16;
                let v: i16 = match parameter {
                    0 => (left != 0 && right != 0) as i16,
                    1 => (left != 0 || right != 0) as i16,
                    2 => (left == right) as i16,
                    3 => (left != right) as i16,
                    4 => (left < right) as i16,
                    5 => (left <= right) as i16,
                    6 => (left > right) as i16,
                    7 => (left >= right) as i16,
                    8 => left.wrapping_add(right),
                    9 => left.wrapping_sub(right),
                    10 => left.wrapping_mul(right),
                    11 => {
                        if right == 0 {
                            0
                        } else {
                            left.wrapping_div(right)
                        }
                    }
                    12 => ((left as i32) >> (right as i32 & 31)) as i16,
                    13 => ((left as i32) << (right as i32 & 31)) as i16,
                    14 => left & right,
                    15 => left | right,
                    16 => {
                        if right == 0 {
                            0
                        } else {
                            left.wrapping_rem(right)
                        }
                    }
                    17 => left ^ right,
                    _ => {
                        e.script = None;
                        return false;
                    }
                };
                e.push(v as u16);
                true
            }
            18 => {
                e.peek(2);
                if e.script.is_none() {
                    return false;
                }
                e.return_value = e.pop();
                let loc = e.pop();
                e.script = Some(loc);
                e.is_subroutine = 0;
                true
            }
            _ => {
                e.script = None;
                false
            }
        }
    }
}
