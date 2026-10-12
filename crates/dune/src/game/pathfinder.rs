//! The original unit pathfinder (Script_Unit_Pathfinder and helpers in script/unit.c).
//!
//! It walks straight towards the destination and, when blocked, follows the obstacle edge
//! clockwise and counter-clockwise, keeping the cheaper of the two; the route is then smoothed.

use super::*;

pub const MAP_DIRECTION: [i16; 8] = [-64, -63, 1, 65, 64, 63, -1, -65];

pub struct PathfinderData {
    pub packed: u16,
    pub score: i16,
    pub route_size: u16,
    pub buffer: Vec<u8>,
}

fn step(packed: u16, dir: u8) -> u16 {
    packed.wrapping_add(MAP_DIRECTION[(dir & 7) as usize] as u16)
}

impl Game {
    /// Script_Unit_Pathfind_GetScore
    fn pathfind_get_score(&self, u: u16, packed: u16, orient8: u8) -> i16 {
        let res = if self.config.dune2_enhanced {
            self.unit_get_tile_enter_score(u, packed, orient8 as u16)
        } else {
            self.unit_get_tile_enter_score(u, packed, (orient8 as u16) << 5)
        };
        if res == -1 {
            256
        } else {
            res
        }
    }

    /// Script_Unit_Pathfinder_Smoothen
    fn pathfinder_smoothen(&self, u: u16, data: &mut PathfinderData) {
        const DIRECTION_OFFSET: [i8; 8] = [0, 0, 1, 2, 3, -2, -1, 0];
        let buf = &mut data.buffer;
        let rs = data.route_size as usize;
        if rs < buf.len() {
            buf[rs] = 0xFF;
        }
        let mut packed = data.packed;
        if data.route_size > 1 {
            let mut to = 1usize;
            while to < buf.len() && buf[to] != 0xFF {
                let mut from = to - 1;
                while buf[from] == 0xFE && from != 0 {
                    from -= 1;
                }
                if buf[from] == 0xFE {
                    to += 1;
                    continue;
                }
                let direction = DIRECTION_OFFSET[((buf[to].wrapping_sub(buf[from])) & 0x7) as usize];
                if direction == 3 {
                    buf[from] = 0xFE;
                    buf[to] = 0xFE;
                    to += 1;
                    continue;
                }
                if direction == 0 {
                    packed = step(packed, buf[from]);
                    to += 1;
                    continue;
                }
                let dir;
                if buf[from] & 0x1 != 0 {
                    dir = ((buf[from] as i16 + if direction < 0 { -1 } else { 1 }) & 0x7) as u8;
                    if direction.abs() == 1 {
                        if self.pathfind_get_score(u, step(packed, dir), dir) <= 255 {
                            buf[to] = dir;
                            buf[from] = dir;
                        }
                        packed = step(packed, buf[from]);
                        to += 1;
                        continue;
                    }
                } else {
                    dir = ((buf[from] as i16 + direction as i16) & 0x7) as u8;
                }
                buf[to] = dir;
                buf[from] = 0xFE;
                while buf[from] == 0xFE && from != 0 {
                    from -= 1;
                }
                if buf[from] != 0xFE {
                    packed = step(packed, (buf[from] + 4) & 0x7);
                } else {
                    packed = data.packed;
                }
            }
        }

        let mut write = 0usize;
        let mut packed = data.packed;
        data.score = 0;
        data.route_size = 0;
        let mut read = 0usize;
        while read < buf.len() && buf[read] != 0xFF {
            let d = buf[read];
            read += 1;
            if d == 0xFE {
                continue;
            }
            packed = step(packed, d);
            data.score = data.score.wrapping_add(self.pathfind_get_score(u, packed, d));
            data.route_size += 1;
            buf[write] = d;
            write += 1;
        }
        data.route_size += 1;
        if write < buf.len() {
            buf[write] = 0xFF;
        }
    }

    /// Script_Unit_Pathfinder_Connect
    fn pathfinder_connect(&self, u: u16, packed_dst: u16, data: &mut PathfinderData, search_direction: i8, direction_start: u8) -> bool {
        let mut packed_cur = data.packed;
        let mut size: usize = 0;
        let mut direction_start = direction_start;
        while size < 100 {
            let mut direction = direction_start;
            let packed_next;
            loop {
                direction = ((direction as i16 + search_direction as i16) & 0x7) as u8;
                let ahead = ((direction as i16 + search_direction as i16) & 0x7) as u8;
                if direction & 0x1 != 0 && step(packed_cur, ahead) == packed_dst {
                    direction = ahead;
                    packed_next = step(packed_cur, direction);
                    break;
                }
                if direction == direction_start {
                    return false;
                }
                let n = step(packed_cur, direction);
                if self.pathfind_get_score(u, n, direction) <= 255 {
                    packed_next = n;
                    break;
                }
            }
            data.buffer[size] = direction;
            size += 1;
            if packed_next == packed_dst {
                data.buffer[size] = 0xFF;
                data.route_size = size as u16;
                self.pathfinder_smoothen(u, data);
                data.route_size -= 1;
                return true;
            }
            if data.packed == packed_next {
                return false;
            }
            direction_start = ((direction as i16 - search_direction as i16 * 3) & 0x7) as u8;
            packed_cur = packed_next;
        }
        false
    }

    /// Script_Unit_Pathfinder
    pub fn pathfinder(&self, u: u16, packed_src: u16, packed_dst: u16, buffer_size: i16) -> PathfinderData {
        let mut res = PathfinderData { packed: packed_src, score: 0, route_size: 0, buffer: vec![0xFF; (buffer_size + 2) as usize] };
        let buffer_size = buffer_size - 1;
        let mut packed_cur = packed_src;

        while (res.route_size as i16) < buffer_size {
            if packed_cur == packed_dst {
                break;
            }
            let direction = get_direction_packed(packed_cur, packed_dst) / 32;
            let mut packed_next = step(packed_cur, direction);
            let score = self.pathfind_get_score(u, packed_next, direction);
            if score <= 255 {
                res.buffer[res.route_size as usize] = direction;
                res.route_size += 1;
                res.score = res.score.wrapping_add(score);
            } else {
                let mut found_ccw = false;
                let mut found_cw = false;
                let mut routes = [
                    PathfinderData { packed: 0, score: 0, route_size: 0, buffer: vec![0; 102] },
                    PathfinderData { packed: 0, score: 0, route_size: 0, buffer: vec![0; 102] },
                ];
                loop {
                    if packed_next == packed_dst {
                        break;
                    }
                    let dir = get_direction_packed(packed_next, packed_dst) / 32;
                    packed_next = step(packed_next, dir);
                    if self.pathfind_get_score(u, packed_next, dir) > 255 {
                        continue;
                    }
                    routes[1] = PathfinderData { packed: packed_cur, score: 0, route_size: 0, buffer: vec![0; 102] };
                    found_ccw = self.pathfinder_connect(u, packed_next, &mut routes[1], -1, direction);
                    routes[0] = PathfinderData { packed: packed_cur, score: 0, route_size: 0, buffer: vec![0; 102] };
                    found_cw = self.pathfinder_connect(u, packed_next, &mut routes[0], 1, direction);
                    if found_ccw || found_cw {
                        break;
                    }
                    loop {
                        if packed_next == packed_dst {
                            break;
                        }
                        let dir = get_direction_packed(packed_next, packed_dst) / 32;
                        packed_next = step(packed_next, dir);
                        if self.pathfind_get_score(u, packed_next, dir) > 255 {
                            break;
                        }
                    }
                }
                if found_ccw || found_cw {
                    let best = if !found_cw {
                        1
                    } else if !found_ccw {
                        0
                    } else if routes[1].score < routes[0].score {
                        1
                    } else {
                        0
                    };
                    let route_size = (buffer_size - res.route_size as i16).min(routes[best].route_size as i16);
                    if route_size <= 0 {
                        break;
                    }
                    let rs = res.route_size as usize;
                    for k in 0..route_size as usize {
                        res.buffer[rs + k] = routes[best].buffer[k];
                    }
                    res.route_size += route_size as u16;
                    res.score = res.score.wrapping_add(routes[best].score);
                } else {
                    break;
                }
            }
            packed_cur = packed_next;
        }
        if (res.route_size as i16) < buffer_size {
            res.buffer[res.route_size as usize] = 0xFF;
            res.route_size += 1;
        }
        self.pathfinder_smoothen(u, &mut res);
        res
    }
}
