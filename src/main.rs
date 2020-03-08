use std::collections::HashMap;
use ggez::*;
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use std::io::{BufReader, BufRead};

// constants

const GAME_NAME: &str = "Falling Carefully";
const STAR_GRAV_MUL: f64 = 1.5;
const STAR_RES: f32 = 27.0;
const STAR_E_SZ: f32 = 1500.0;
const E_TG: f64 = 0.0018;
const E_TS: f64 = 0.0012;
const E_TF: f64 = 0.0009;
const EXPLOSION_RES: f32 = 10.0;
const EXPLOSION_STROKE: f32 = 27.0;
const PORTAL_SCALE: f32 = 300.0;
const PORTAL_GRAV: f64 = 300.0 * 200.0 * 100.0;
const SHIP_SCALE: f32 = 18.0;
const SHIP_TRAIL_SZ: f32 = 3.6;
const SHIP_TRAIL_AMT: usize = 120;
const TURRET_SCALE: f32 = 120.0;
const POWERUP_SCALE: f32 = 100.0;
const FUEL_PER_PUP: f64 = 450.0;
const SHIP_MINSZ: f32 = 1.0;    // minimun scale factor it will get to
const GRAV_REACH: f64 = 0.45;    // minimum pull before out of range
const MAX_CAM_SCALE: f32 = 90.0;
const MIN_CAM_SCALE: f32 = 0.21;
const ZOOM_AMT: f32 = 0.06;
const LOG_TICKS: usize = 81;
const PLAYER_THRUST: f64 = 69.0;
const PLAYER_FUEL: f64 = 1200.0;
const PLAYER_EMPTY_THRUST: f64 = 12.0;
const PLAYER_AMMO: usize = 15;
const PLAYER_NUKE_SIZE: f32 = 900.0;
const PLAYER_NUKE_IVEL: f64 = 750.0;
const PLAYER_NUKE_DIST: f64 = 18.0;
const PLAYER_NUKE_THRUST: f64 = 15.0;
const NUKE_TRAIL_SZ: f32 = 2.0;
const EXPLOSION_COLOR: [f32; 4] = [1.0, 0.12, 0.27, 0.9];
const SHIP_TRAIL_COLOR: [f32; 4] = [1.0, 0.81, 0.90, 0.69];
const NUKE_TRAIL_COLOR: [f32; 4] = [1.0, 0.0, 0.0, 0.6];
const PRED_COLOR: [f32; 4] = [0.6, 0.75, 1.0, 0.69];
const PRED_SIZE: f32 = 0.81;
const PRED_RATE: f64 = 0.0;
const PRED_TSTEP: f64 = 0.27;
const PRED_AMT: usize = 45;
const TRAIL_DIST: f32 = 30.0;
const TURRET_FIRE_RATE: f64 = 2.1;
const TURRET_UPDATE_RATE: f64 = 1.0;
const TURRET_NUKE_THRUST: f64 = 180.0;
const TURRET_NUKE_SIZE: f32 = 270.0;
const TURRET_NUKE_DIST: f64 = 60.0;
const TURRET_NUKE_IVEL: f64 = 300.0;
const TURRET_DIST2: f64 = 8000.0 * 8000.0;

const GUIDE: &str = concat!(
    "      Welcome to Falling Carefully\n",
    "\n",
    "     @     =  Your Ship (Right Click to Thrust)\n",
    "    =>     =  Nuke (Left Click to Release)\n",
    "     *     =  Fuel\n",
    "     #     =  Enemy Turret\n",
    "     &     =  Portal Lock (Destroy These)\n",
    "\n",
    "   ( X )   =  Closed portal\n",
    "\n",
    "  \\ | /\n",
    "--(   )--  = Open portal to Next Zone\n",
    "  / | \\\n",
    "\n",
    "       Press R to Start / Restart\n",
);

type IdVal = usize;

struct Entity {
    id: IdVal,
    to_destroy: bool,
}

struct CPos {
    x: f64,
    y: f64,
    a: f32,
}

struct CGrav {
    mass: f64,
    dist2: f64,  // distance squared at which this object can be ignored
}

struct CDynamic {
    x_vel: f64,
    y_vel: f64,
    //a_vel: f32,
    in_ax: f64,
    in_ay: f64,
}

struct CTrail {
    objid: IdVal,
    pts: Vec<[f32; 2]>, // x, y, thrust
    max_len: usize,
    size: f32, // width of the trail
    color: [f32; 4],
    dist: f32,
}

// Used for predictions on movement
struct CPredictable {
    objid: IdVal,
    pts: Vec<[f32; 2]>,
    tstep: f64,
    rate: f64,
    till_next: f64,
    valid_len: usize,
    collidable: bool,
    color: [f32; 4],
}

enum CollisionType {
    Explosion(f32, bool),
    FuelPup(f64),
    Portal,
    None,
}

struct CCollider {
    rad: f64,
    col_action: CollisionType,
    stop_col: bool,
}

struct CCollides {
    rad: f64,
}

struct CExplosion {
    grow_size: f32,
    time_grow: f64,
    time_stay: f64,
    time_fade: f64,
    time_so_far: f64,
}

struct CRocket {
    thrust: f64,
    target: Option<IdVal>,
}

struct CTurret {
    fire_rate: f64,
    till_next_shot: f64,
}

enum DrawThing {
    Blank,
    Mesh(graphics::Mesh),
    MeshScale(graphics::Mesh, f32),
    MeshInd(usize),
}

struct CDrawable {
    thing: DrawThing,
    r: f32, // radius for culling
    minsz: f32,
}

impl CDrawable {
    fn draw(&self, st: &State, ctx: &mut Context, mut param: graphics::DrawParam) -> error::GameResult {
        if self.minsz != 0.0 && st.cam.s < self.minsz {
            let s = self.minsz / st.cam.s;
            param = param.scale([s, s]);
        }
        match self.thing {
            DrawThing::Blank => return Ok(()),
            DrawThing::Mesh(ref m) => {
                return graphics::draw(ctx, m, param);
            },
            DrawThing::MeshScale(ref m, sc) => {
                return graphics::draw(ctx, m, param.scale([sc,sc]));
            },
            DrawThing::MeshInd(i) => {
                let (m, _) = &st.meshs[i];
                return graphics::draw(ctx, m, param);
            },
        }
    }
}

struct CShip {
    thrust: f64, // fake thrust actually, pure accelaration, no mass
    empty_thrust: f64, // small amount of thrust when no fuel is available
    fuel: f64,
    ammo: usize,
}

struct InputState {
    up: bool,
    down: bool,
    right: bool,
    left: bool,
    cw: bool,
    ccw: bool,
    reset: bool,

    mx: f32,
    my: f32,
    lmb: bool,
    rmb: bool,
}

struct Camera {
    x: f64,
    y: f64,
    s: f32,
    //TODO camera rotation a: f32,

    update: bool,
}

impl Camera {
    fn world2cam(&self, sc: &graphics::Rect, px: f64, py: f64) -> (f32, f32) {
        let px = (px - self.x) as f32;
        let py = (py - self.y) as f32;

        (
            (sc.w/2.0) + (px * self.s),
            (sc.h/2.0) + (py * self.s),
        )
    }

    fn cam2world(&self, sc: &graphics::Rect, cx: f32, cy: f32) -> (f64, f64) {
        let px = (cx - (sc.w/2.0)) / self.s;
        let py = (cy - (sc.h/2.0)) / self.s;

        return ((px as f64) + self.x, (py as f64) + self.y);
    }

    fn do_update(&mut self, ctx: &mut Context, sc: &graphics::Rect) -> bool {
        if !self.update {
            return false;
        }

        let mut scrn = graphics::DrawParam::default();

        let (cx, cy) = (
            (sc.w/2.0) - ((self.x as f32) * self.s),
            (sc.h/2.0) - ((self.y as f32) * self.s)
        );
        scrn.dest.x = cx;
        scrn.dest.y = cy;

        scrn.scale.x = self.s;
        scrn.scale.y = self.s;

        graphics::push_transform(ctx, Some(scrn.to_matrix()));

        //TODO figure out how to apply roataion? Do it to the matrix?
        // pushing another transform doesn't seem to work?

        graphics::apply_transformations(ctx).unwrap();
        self.update = false;

        return true;
    }

    fn is_visible(&self, _ctx: &mut Context, sc: &graphics::Rect, x: f64, y: f64, r: f32) -> bool {
        if r == std::f32::INFINITY {
            return true;
        }

        let (cx, cy) = Camera::world2cam(self, sc, x, y);

        let r = r * self.s;
        
        if (cx+r) < 0.0 || (cx-r) > sc.w || (cy+r) < 0.0 || (cy-r) > sc.h {
            return false;
        }

        return true;
    } 
}

enum MeshNum {
    AngMesh = 0,
    AMesh,
    AstMesh,
    BangVMesh,
    CapMesh,
    HashMesh,
    NukeMesh,
    LockMesh,
    ClosedMesh,
    OpenMesh,
}

struct State {
    //shaders: Vec<graphics::Shader>,
    meshs: Vec<(graphics::Mesh, f32)>,
    rng: SmallRng,
    cam: Camera,
    level: usize,
    next_id: IdVal,
    entities: Vec<Entity>,
    c_pos: HashMap<IdVal, CPos>,
    c_grav: HashMap<IdVal, CGrav>,
    c_dynamic: HashMap<IdVal, CDynamic>,
    c_collider: HashMap<IdVal, CCollider>,
    c_collides: HashMap<IdVal, CCollides>,
    c_drawable: HashMap<IdVal, CDrawable>,
    c_trail: HashMap<IdVal, CTrail>,
    c_predictable: HashMap<IdVal, CPredictable>,
    c_ship: HashMap<IdVal, CShip>,
    c_explosion: HashMap<IdVal, CExplosion>,
    c_rocket: HashMap<IdVal, CRocket>,
    c_turret: HashMap<IdVal, CTurret>,

    locks: Vec<IdVal>,
    portal: Option<IdVal>,

    s_turret_next: f64,

    input: InputState,
    playerid: Option<IdVal>,
    finished: bool, // finished level
    started: bool,

    font: graphics::Font,

    //DEBUG
    //log_time: usize,
    //grav_count: usize,
}

impl State {
    fn new(ctx: &mut Context) -> ggez::GameResult<State> {
        let s = State{
            meshs: [
                    ("\\ang.obj", SHIP_SCALE, [1.0; 4]),
                    ("\\A.obj", SHIP_SCALE, [1.0; 4]),
                    ("\\ast.obj", POWERUP_SCALE, [0.75, 0.75, 0.81, 1.0]),
                    ("\\bangv.obj", POWERUP_SCALE, [0.75, 0.75, 1.0, 1.0]),
                    ("\\capital.obj", SHIP_SCALE, [1.0; 4]),
                    ("\\pnd.obj", TURRET_SCALE, [0.9, 0.48, 0.45, 1.0]),
                    ("\\nuke.obj", SHIP_SCALE, [0.81, 0.3, 0.3, 1.0]),
                    ("\\lock.obj", POWERUP_SCALE, [0.5, 0.5, 0.69, 1.0]),
                    ("\\ClosedPortal.obj", PORTAL_SCALE, [0.6, 0.6, 0.81, 1.0]),
                    ("\\OpenPortal.obj", PORTAL_SCALE, [0.3, 0.42, 0.9, 1.0]),
                ].iter().map(
                |x| load_mesh(ctx, x.0, x.1, x.2)
            ).collect(),
            rng: SmallRng::from_entropy(),

            cam: Camera{
                x: 0.0,
                y: 0.0,
                s: 1.0,
                update: true,
            },

            level: 0,

            font: graphics::Font::new(ctx, "\\NovaMono-Regular.ttf").unwrap(),

            //DEBUG
            //log_time: 0,
            //grav_count: 0,

            input: InputState{
                up: false,
                down: false,
                left: false,
                right: false,
                cw: false,
                ccw: false,
                reset: false,
                mx: 0.0,
                my: 0.0,
                lmb: false,
                rmb: false,
            },

            // items that change between levels /etc
            next_id: 1,
            entities: Vec::new(),
            c_pos: HashMap::new(),
            c_grav: HashMap::new(),
            c_dynamic: HashMap::new(),
            c_collider: HashMap::new(),
            c_collides: HashMap::new(),
            c_drawable: HashMap::new(),
            c_trail: HashMap::new(),
            c_predictable: HashMap::new(),
            c_ship: HashMap::new(),
            c_explosion: HashMap::new(),
            c_rocket: HashMap::new(),
            c_turret: HashMap::new(),

            locks: Vec::new(),
            portal: None,

            s_turret_next: 0.0,

            playerid: None,
            finished: false,
            started: false,

        };

        Ok(s)
    }

    fn reset(&mut self) {
        self.entities.clear();
        self.c_pos.clear();
        self.c_grav.clear();
        self.c_dynamic.clear();
        self.c_collider.clear();
        self.c_collides.clear();
        self.c_drawable.clear();
        self.c_trail.clear();
        self.c_predictable.clear();
        self.c_ship.clear();
        self.c_explosion.clear();
        self.c_rocket.clear();
        self.c_turret.clear();
        self.locks.clear();
        self.portal = None;
        self.s_turret_next = 0.0;
        self.playerid = None;
        self.level = 0;
        self.finished = false;
        self.started = true;
    }

    fn s_destroy(&mut self) {
        let mut i = 0;
        while i < self.entities.len() {
            let e = &self.entities[i];
            if !e.to_destroy {
                i += 1;
                continue;
            }

            if let Some(pid) = self.playerid {
                if pid == e.id {
                    self.playerid = None;
                }
            }

            let mut j = 0;
            while j < self.locks.len() {
                if self.locks[j] == e.id {
                    self.locks.remove(j);
                } else {
                    j += 1;
                }
            }
            if self.locks.len() == 0 {
                //open portal
                if let Some(portalid) = &self.portal {
                    let d = self.c_drawable.get_mut(portalid).unwrap();
                    let c = self.c_collider.get_mut(portalid).unwrap();

                    c.col_action = CollisionType::Portal;
                    d.thing = DrawThing::MeshInd(MeshNum::OpenMesh as usize);
                }
            }

            self.c_pos.remove(&e.id);
            self.c_grav.remove(&e.id);
            self.c_dynamic.remove(&e.id);
            self.c_collider.remove(&e.id);
            self.c_collides.remove(&e.id);
            self.c_drawable.remove(&e.id);
            self.c_predictable.remove(&e.id);
            self.c_ship.remove(&e.id);
            self.c_trail.remove(&e.id);
            self.c_explosion.remove(&e.id);
            self.c_rocket.remove(&e.id);
            self.c_turret.remove(&e.id);

            self.entities.remove(i);
        }
    }

    fn gen_level(&mut self, ctx: &mut Context, level: usize) {
        self.reset();
        self.level = level;
        //TODO make this good
        //need portal, key, turrets, boosters

        // add Portal

        self.add_portal(ctx, 0.0, 0.0, 0.0);

        // add stars
        let num_big = self.rng.gen_range(1, 6);
        let mut i = 0;
        'star_loop_large: while i < num_big {
            let a = self.rng.gen_range(0.0, std::f64::consts::PI * 2.0);
            let d = self.rng.gen_range(450.0, 3000.0);

            let x = d * a.cos();
            let y = d * a.sin();
            let s = self.rng.gen_range(300.0, 360.0 + (300.0 / (num_big as f64)));

            for (cid, c) in &self.c_collider {
                let colpos = &self.c_pos.get(cid).unwrap();
                
                let dx = colpos.x - x;
                let dy = colpos.y - y;

                let rdist = c.rad + s;
                if (rdist * rdist) > ((dx * dx) + (dy * dy)) {
                    // would overlap
                    continue 'star_loop_large;
                }
            }

            self.add_star(
                ctx,
                x, y,
                s,
                true,
            );

            i += 1;
        }

        let num_small = self.rng.gen_range(45, 180);
        let mut i = 0;
        'star_loop_small: while i < num_small {
            let a = self.rng.gen_range(0.0, std::f64::consts::PI * 2.0);
            let d = self.rng.gen_range(999.0, 6000.0);

            let x = d * a.cos();
            let y = d * a.sin();
            let s = self.rng.gen_range(45.0, 120.0);

            for (cid, c) in &self.c_collider {
                let colpos = &self.c_pos.get(cid).unwrap();
                
                let dx = colpos.x - x;
                let dy = colpos.y - y;
                let rdist = c.rad + s;
                if (rdist * rdist) > ((dx * dx) + (dy * dy)) {
                    // would overlap
                    continue 'star_loop_small;
                }
            }

            self.add_star(
                ctx,
                x, y,
                s,
                false,
            );

            i += 1;
        }

        let (_, turret_r) = self.meshs[MeshNum::HashMesh as usize];
        let turret_r = turret_r as f64;
        let mut i = 0;
        let turret_amt = 1 + (2 * level);
        'turret_loop: while i < turret_amt {
            let a = self.rng.gen_range(0.0, std::f64::consts::PI * 2.0);
            let d = self.rng.gen_range(900.0, 6600.0);

            let x = d * a.cos();
            let y = d * a.sin();

            let a = self.rng.gen_range(-std::f32::consts::PI, std::f32::consts::PI);

            for (cid, c) in &self.c_collider {
                let colpos = &self.c_pos.get(cid).unwrap();
                
                let dx = colpos.x - x;
                let dy = colpos.y - y;
                let rdist = c.rad + turret_r;
                if (rdist * rdist) > ((dx * dx) + (dy * dy)) {
                    // would overlap
                    continue 'turret_loop;
                }
            }

            self.add_turret(
                ctx,
                x, y, a
            );

            i += 1;
        }

        let (_, fuel_r) = self.meshs[MeshNum::HashMesh as usize];
        let fuel_r = fuel_r as f64;
        let mut i = 0;
        let fuel_amt = 90 / (level + 1);
        'fuel_loop: while i < fuel_amt {
            let a = self.rng.gen_range(0.0, std::f64::consts::PI * 2.0);
            let d = self.rng.gen_range(450.0, 4500.0);

            let x = d * a.cos();
            let y = d * a.sin();

            for (cid, c) in &self.c_collider {
                let colpos = &self.c_pos.get(cid).unwrap();
                
                let dx = colpos.x - x;
                let dy = colpos.y - y;
                let rdist = c.rad + fuel_r + 1.0;
                if (rdist * rdist) > ((dx * dx) + (dy * dy)) {
                    // would overlap
                    continue 'fuel_loop;
                }
            }

            self.add_fuel_powerup(
                ctx,
                x, y,
            );

            i += 1;
        }

        let (_, lock_r) = self.meshs[MeshNum::HashMesh as usize];
        let lock_r = lock_r as f64;
        let num_locks = level+1;
        let mut i = 0;
        'lock_loop: while i < num_locks {
            let a = self.rng.gen_range(0.0, std::f64::consts::PI * 2.0);
            let d = self.rng.gen_range(300.0, 6000.0);

            let x = d * a.cos();
            let y = d * a.sin();

            for (cid, c) in &self.c_collider {
                let colpos = &self.c_pos.get(cid).unwrap();
                
                let dx = colpos.x - x;
                let dy = colpos.y - y;
                let rdist = c.rad + lock_r + 45.0;
                if (rdist * rdist) > ((dx * dx) + (dy * dy)) {
                    // would overlap
                    continue 'lock_loop;
                }
            }
            self.add_lock(
                ctx,
                x, y, a as f32,
            );

            i += 1;
        }

        let mut spawned_player = false;
        let (_, player_r) = self.meshs[MeshNum::AngMesh as usize];
        let player_r = player_r as f64;
        'player_loop: while !spawned_player {
            let a = self.rng.gen_range(0.0, std::f64::consts::PI * 2.0);
            let d = self.rng.gen_range(5500.0, 6900.0);

            let x = d * a.cos();
            let y = d * a.sin();

            for (cid, c) in &self.c_collider {
                let colpos = &self.c_pos.get(cid).unwrap();
                
                let dx = colpos.x - x;
                let dy = colpos.y - y;
                let rdist = c.rad + player_r + 100.0;
                if (rdist * rdist) > ((dx * dx) + (dy * dy)) {
                    // would overlap
                    continue 'player_loop;
                }
            }

            spawned_player = true;

            let shipid = self.add_ship(
                ctx,
                MeshNum::AngMesh,
                x, y,
                PLAYER_THRUST, PLAYER_EMPTY_THRUST, PLAYER_FUEL,
                PLAYER_AMMO,
            );
            self.make_player(shipid);

            // add prediction on the player ship
            self.add_prediction(ctx, shipid, true);
        }
        
        self.started = true;
    }

    fn make_player(&mut self, id: IdVal) {
        self.playerid = Some(id);
    }

    fn add_entity(&mut self) -> IdVal {
        let id = self.next_id;
        self.next_id += 1;

        self.entities.push(
            Entity{
                id,
                to_destroy: false
            }
        );

        return id;
    }

    fn add_portal(&mut self, _ctx: &mut Context, x: f64, y: f64, a: f32) -> IdVal {
        let id = self.add_entity();

        let i = MeshNum::ClosedMesh as usize;
        let (_, r) = self.meshs[i];

        self.c_pos.insert(
            id,
            CPos{x, y, a},
        );
        self.c_grav.insert(
            id,
            CGrav{mass: PORTAL_GRAV, dist2: std::f64::INFINITY},
        );
        self.c_drawable.insert(
            id,
            CDrawable{
                thing: DrawThing::MeshInd(i),
                r,
                minsz: 0.0,
            }
        );
        self.c_collider.insert(
            id,
            CCollider{
                rad: r as f64,
                col_action: CollisionType::None,
                stop_col: false,
            },
        );

        self.portal = Some(id);

        return id;
    }

    fn add_lock(&mut self, _ctx: &mut Context, x: f64, y: f64, a: f32) -> IdVal {
        let id = self.add_entity();

        let i = MeshNum::LockMesh as usize;
        let (_, r) = self.meshs[i];

        self.c_pos.insert(
            id,
            CPos{x, y, a},
        );
        self.c_drawable.insert(
            id,
            CDrawable{
                thing: DrawThing::MeshInd(i),
                r,
                minsz: 0.0,
            }
        );
        self.c_collides.insert(
            id,
            CCollides{
                rad: r as f64,
            },
        );

        self.locks.push(id);

        return id;
    }

    fn add_turret(&mut self, _ctx: &mut Context, x: f64, y: f64, a: f32) -> IdVal {
        let id = self.add_entity();

        let i = MeshNum::HashMesh as usize;
        let (_, r) = self.meshs[i];

        self.c_drawable.insert(
            id,
            CDrawable{
                thing: DrawThing::MeshInd(i),
                r,
                minsz: 0.0,
            }
        );
        self.c_pos.insert(
            id,
            CPos{x, y, a},
        );
        self.c_collides.insert(
            id,
            CCollides{
                rad: r as f64,
            },
        );

        self.c_turret.insert(
            id,
            CTurret{
                fire_rate: TURRET_FIRE_RATE,
                till_next_shot: 0.0,
            },
        );

        return id;
    }

    fn add_fuel_powerup(&mut self, _ctx: &mut Context, x: f64, y: f64) -> IdVal {
        let id = self.add_entity();

        let i = MeshNum::AstMesh as usize;
        let (_, r) = self.meshs[i];

        self.c_drawable.insert(
            id,
            CDrawable{
                thing: DrawThing::MeshInd(i),
                r,
                minsz: 0.0,
            }
        );
        self.c_collider.insert(
            id,
            CCollider{
                rad: r as f64,
                col_action: CollisionType::FuelPup(FUEL_PER_PUP),
                stop_col: false,
            },
        );
        self.c_pos.insert(
            id,
            CPos{x, y, a: 0.0},
        );
        self.c_collides.insert(
            id,
            CCollides{
                rad: r as f64,
            },
        );

        return id;
    }

    fn add_prediction(&mut self, _ctx: &mut Context, objid: IdVal, drawable: bool) -> IdVal {
        let p_id = self.add_entity();

        let mut ptsvec = Vec::new();
        //Add points
        for _ in 0..PRED_AMT {
            ptsvec.push([0.0,0.0]);
        }
        self.c_predictable.insert(
            p_id,
            CPredictable{
                objid,
                pts: ptsvec,
                tstep: PRED_TSTEP,
                rate: PRED_RATE,
                till_next: 0.0,
                valid_len: 0,
                collidable: true,
                color: PRED_COLOR,
            },
        );
        if drawable {
            self.c_drawable.insert(
                p_id,
                CDrawable{
                    thing: DrawThing::Blank,
                    r: std::f32::INFINITY,
                    minsz: 0.0, // we do this in the mesh gen
                },
            );
        }
        self.c_pos.insert(
            p_id,
            CPos{x: 0.0, y: 0.0, a: 0.0},
        );

        return p_id;
    }

    fn add_star(&mut self, ctx: &mut Context, x: f64, y: f64, size: f64, always_pull: bool) -> IdVal {
        let id = self.add_entity();

        self.c_pos.insert(
            id,
            CPos{x, y, a: 0.0},
        );
        let mass = STAR_GRAV_MUL * size * size * size;
        // GRAV_REACH = mass / dist2
        //dist2 = mass / GRAV_REACH
        let dist2 = if always_pull {
            std::f64::INFINITY
        } else {
            mass / GRAV_REACH
        };

        self.c_grav.insert(
            id,
            CGrav{mass, dist2: dist2},
        );
        self.c_drawable.insert(
            id,
            CDrawable{
                thing: DrawThing::Mesh(
                    graphics::Mesh::new_circle(
                        ctx,
                        graphics::DrawMode::fill(),
                        [0.0, 0.0],
                        size as f32,
                        STAR_RES,
                        graphics::WHITE,
                    ).unwrap()
                ),
                r: size as f32,
                minsz: 0.0,
            },
        );
        self.c_collider.insert(
            id,
            CCollider{
                rad: size,
                col_action: CollisionType::Explosion(STAR_E_SZ, false),
                stop_col: true,
            },
        );

        return id;
    }

    fn add_ship(&mut self, ctx: &mut Context, m: MeshNum, x: f64, y: f64, thrust: f64, empty_thrust: f64, fuel: f64, ammo: usize) -> IdVal {
        let id = self.add_entity();

        self.c_pos.insert(
            id,
            CPos{x, y, a: 0.0},
        );
        let i = m as usize;
        let (_, rad) = self.meshs[i];
        self.c_drawable.insert(
            id,
            CDrawable{
                thing: DrawThing::MeshInd(i),
                r: rad,
                minsz: SHIP_MINSZ,
            },
        );
        self.c_dynamic.insert(
            id,
            CDynamic {
                x_vel: 0.0,
                y_vel: 0.0,
                in_ax: 0.0,
                in_ay: 0.0,
            },
        );
        self.c_collides.insert(
            id,
            CCollides{
                rad: rad as f64,
            },
        );
        self.c_ship.insert(
            id,
            CShip {
                thrust,
                empty_thrust,
                fuel,
                ammo,
            }
        );

        self.add_trail(ctx, &id, SHIP_TRAIL_SZ, SHIP_TRAIL_COLOR);

        return id;
    }

    fn add_trail(&mut self, _ctx: &mut Context, pid: &IdVal, size: f32, color: [f32; 4]) -> IdVal {
        let id = self.add_entity();
        self.c_trail.insert(
            id,
            CTrail{
                objid: *pid,
                pts: Vec::new(),
                max_len: SHIP_TRAIL_AMT,
                size,
                color: color,
                dist: TRAIL_DIST,
            },
        );
        self.c_drawable.insert(
            id,
            CDrawable{
                thing: DrawThing::Blank,
                r: std::f32::INFINITY,
                minsz: 0.0,
            },
        );
        self.c_pos.insert(
            id,
            CPos{x: 0.0, y: 0.0, a: 0.0},
        );

        return id;
    }

    fn spawn_explosion(&mut self, ctx: &mut Context, px: f64, py: f64, size: f32, collidable: bool) -> IdVal {
        let id = self.add_entity();
        self.c_pos.insert(
            id,
            CPos{x: px, y: py, a: 0.0},
        );

        let s64 = size as f64;
        let tg = E_TG * s64;
        let ts = tg + (E_TS * s64);
        let tf = ts + (E_TF * s64);
        self.c_explosion.insert(
            id,
            CExplosion{
                grow_size: size,
                time_grow: tg,
                time_stay: ts,
                time_fade: tf,
                time_so_far: 0.0,
            },
        );

        self.c_drawable.insert(
            id,
            CDrawable{
                thing: DrawThing::MeshScale(
                    graphics::Mesh::new_circle(
                        ctx,
                        graphics::DrawMode::stroke(EXPLOSION_STROKE),
                        [0.0, 0.0],
                        size,
                        EXPLOSION_RES,
                        graphics::Color::from(EXPLOSION_COLOR),
                    ).unwrap(),
                    0.0,
                ),
                r: 30.0,
                minsz: 0.0,
            },
        );

        if collidable {
            self.c_collider.insert(
                id,
                CCollider{
                    rad: 0.0,
                    col_action: CollisionType::Explosion(size/1.5, false),
                    stop_col: false,
                },
            );
        }

        return id;
    }

    fn spawn_nuke(&mut self, ctx: &mut Context, px: f64, py: f64, a: f32, vx: f64, vy: f64, thrust: f64, target: Option<IdVal>, explosion_size: f32) {
        let id = self.add_entity();

        self.c_pos.insert(
            id,
            CPos{x: px, y: py, a: a},
        );

        let i = MeshNum::NukeMesh as usize;
        let (_, rad) = self.meshs[i];
        self.c_drawable.insert(
            id,
            CDrawable{
                thing: DrawThing::MeshInd(i),
                r: rad,
                minsz: 0.0,
            },
        );
        self.c_dynamic.insert(
            id,
            CDynamic {
                x_vel: vx,
                y_vel: vy,
                in_ax: 0.0,
                in_ay: 0.0,
            },
        );
        self.c_collides.insert(
            id,
            CCollides{
                rad: rad as f64,
            },
        );
        if thrust != 0.0 {
            self.c_rocket.insert(
                id,
                CRocket{
                    thrust,
                    target,
                }
            );
        }

        self.add_trail(ctx, &id, NUKE_TRAIL_SZ, NUKE_TRAIL_COLOR);

        self.c_collider.insert(
            id,
            CCollider{
                rad: rad as f64,
                col_action: CollisionType::Explosion(explosion_size, true),
                stop_col: false,
            },
        );
    }

    fn get_grav_a(gravs: &HashMap<IdVal, CGrav>, pos: &HashMap<IdVal, CPos>, px: f64, py: f64, id: &IdVal) -> (f64, f64, usize) {
        let mut ax: f64 = 0.0;
        let mut ay: f64 = 0.0;

        let mut count = 0;

        for (gid, g) in gravs {
            if *gid == *id {
                continue;
            }

            let gp = &pos[gid];

            let dx = gp.x - px;
            let dy = gp.y - py;

            if dx == 0.0 && dy == 0.0 {
                continue;
            }

            let r2 = (dx * dx) + (dy * dy);
            if r2 > g.dist2 {
                continue;
            }

            count += 1;

            let r = r2.sqrt();
            let r3 = r2 * r;

            // get accelaration due to this item
            let ga = g.mass / r3;
            ax += ga * dx;
            ay += ga * dy;
        }

        return (ax, ay, count);
    }

    fn raycast(colliders: &HashMap<IdVal, CCollider>, pos: &HashMap<IdVal, CPos>, px1: f64, py1: f64, px2: f64, py2: f64) -> bool {
        // dist from line to point
        // (a * x0 + b * y0 + c) / sqrt((a*a) + (b*b))
        // where a = (y1-y2), b = (x2-x1), c = ((x1-x2)y1 + (y2-y1)x1)
        // point 0 being the circle, points 1 and 2 being the ends of the line

        let b = px2 - px1;
        let a = py1 - py2;
        let c = ((px1 - px2) * py1) + ((py2 - py1) * px1);
        let d = (b * b) + (a * a);

        for (cid, col) in colliders {
            // check if line in circles
            let cp = pos.get(cid).unwrap();
            let mut dist2 = (a * cp.x) + (b * cp.y) + c;
            dist2 *= dist2;
            dist2 /= d;

            if dist2 < (col.rad * col.rad) {
                return false;
            }
        }

        return true;
    }

    fn s_turret(&mut self, ctx: &mut Context, mut dt: f64) {
        self.s_turret_next += dt;
        if self.s_turret_next > TURRET_UPDATE_RATE {
            dt = self.s_turret_next;
            self.s_turret_next = 0.0;
        } else {
            return;
        }

        if self.playerid.is_none() {
            return;
        }

        let pid = self.playerid.unwrap();
        let ppos = self.c_pos.get(&pid).unwrap();

        let mut launch_nuke = false;
        let mut na = 0.0;
        let mut nxv = 0.0;
        let mut nyv = 0.0;
        let mut npx = 0.0;
        let mut npy = 0.0;

        for (id, t) in &mut self.c_turret {
            t.till_next_shot -= dt;
            if t.till_next_shot > 0.0 {
                continue;
            }
            let p = self.c_pos.get(id).unwrap();
            let dx = p.x - ppos.x;
            let dy = p.y - ppos.y;
            let d2 = (dx*dx)+(dy*dy);
            if d2 < TURRET_DIST2 && State::raycast(
                &self.c_collider, &self.c_pos,
                ppos.x, ppos.y,
                p.x, p.y,
            ) {
                t.till_next_shot = t.fire_rate;
                // spawn nuke
                launch_nuke = true;
                na = dy.atan2(dx) as f32;
                let nac = -na.cos() as f64;
                let nas = -na.sin() as f64;
                nxv = nac * TURRET_NUKE_IVEL;
                nyv = nas * TURRET_NUKE_IVEL;
                npx = p.x + (nac * TURRET_NUKE_DIST);
                npy = p.y + (nas * TURRET_NUKE_DIST);

                break; // don't have to all fire at once
            }
        }

        if launch_nuke {
            self.spawn_nuke(
                ctx,
                npx, npy, // pos
                na, // angle
                nxv, // xvel
                nyv, // yvel
                TURRET_NUKE_THRUST, // thrust
                Some(pid), // target
                TURRET_NUKE_SIZE, // explosion size
            );
        }
    }

    fn s_explosion(&mut self, _ctx: &mut Context, dt: f64) {
        for (id, ex) in &mut self.c_explosion {
            ex.time_so_far += dt;
            let d = &mut self.c_drawable.get_mut(id).unwrap();

            if ex.time_so_far <= ex.time_grow {
                // grow
                let mut r = ex.time_so_far / ex.time_grow;
                r = r.sqrt();
                if let DrawThing::MeshScale(_, ref mut sc) = d.thing {
                    *sc = r as f32;
                }
                r *= ex.grow_size as f64;
                if let Some(c) = &mut self.c_collider.get_mut(id) {
                    c.rad = r;
                }
            } else if ex.time_so_far <= ex.time_stay {
                continue;
            } else if ex.time_so_far <= ex.time_fade{
                // fade out?
                let mut r = 1.0 - ((ex.time_so_far - ex.time_stay) / (ex.time_fade - ex.time_stay));
                if let DrawThing::MeshScale(_, ref mut sc) = d.thing {
                    *sc = r as f32;
                }
                r *= ex.grow_size as f64;
                if let Some(c) = &mut self.c_collider.get_mut(id) {
                    c.rad = r;
                }
            } else {
                // remove this explosion
                for e in &mut self.entities {
                    if e.id == *id {
                        e.to_destroy = true;
                        break;
                    }
                }
            }
        }
    }

    fn s_rocket(&mut self, _ctx: &mut Context, _dt: f64) {
        for (id, r) in &mut self.c_rocket {
            // if they have a target, orient them
            let pa;
            if let Some(tid) = &r.target {
                let tpx;
                let tpy;
                if let Some(tp) = self.c_pos.get(tid) {
                    tpx = tp.x;
                    tpy = tp.y;
                } else {
                    // target it probably destroyed
                    r.target = None;
                    continue;
                }
                let p = self.c_pos.get_mut(id).unwrap();
                let dx = p.x - tpx;
                let dy = p.y - tpy;
                p.a = dy.atan2(dx) as f32;
                pa = p.a;
            } else {
                let p = self.c_pos.get_mut(id).unwrap();
                pa = p.a;
            }
            // do thrust
            let d = self.c_dynamic.get_mut(id).unwrap();

            d.in_ax = -pa.cos() as f64 * r.thrust;
            d.in_ay = -pa.sin() as f64 * r.thrust;

        }
    }

    fn s_trail(&mut self, ctx: &mut Context, _dt: f64) {
        for (id, t) in &mut self.c_trail {
            match self.c_pos.get(&t.objid) {
                Some(p) => {
                    let px = p.x as f32;
                    let py = p.y as f32;
                    if t.pts.len() < 3 || (t.pts[1][0] - px).abs() > t.dist  || (t.pts[1][1] - py).abs() > t.dist {
                        t.pts.insert(0, [px, py]);
                        if t.pts.len() > t.max_len {
                            t.pts.pop();
                        }
                    } else {
                        t.pts[0][0] = px;
                        t.pts[0][1] = py;
                    }

                    if let Some(mut d) = self.c_drawable.get_mut(&id) {
                        d.thing = gen_fading_path(ctx, &t.pts[..], t.size, t.color);
                    }
                },
                None => {
                    // object must have been deleted
                    // we should go too
                    for e in &mut self.entities {
                        if e.id == *id {
                            e.to_destroy = true;
                            break;
                        }
                    }
                }
            }
        }
    }

    fn s_predict(&mut self, ctx: &mut Context, dt: f64) {
        // for each dyn object for each gravity object in range
        for (id, p) in &mut self.c_predictable {
            // different rates for different items
            p.till_next -= dt;
            if p.till_next > 0.0 {
                continue;
            }
            p.till_next = p.rate;

            match self.c_dynamic.get(&p.objid) {
                Some(obj) => {
                    let objp = self.c_pos.get(&p.objid).expect("Predictables.objid must have pos");

                    let mut fx = objp.x;
                    let mut fy = objp.y;
                    let mut fvx = obj.x_vel;
                    let mut fvy = obj.y_vel;

                    p.valid_len = 0;
                    let mut hit_something = false;
                    for pt in &mut p.pts {
                        p.valid_len += 1;
                        *pt = [fx as f32, fy as f32];

                        //fill out the points
                        if hit_something {
                            break;
                        }

                        let tmpid = 0;
                        let (ax, ay, _) = State::get_grav_a(
                            &self.c_grav,
                            &self.c_pos,
                            fx,
                            fy,
                            &tmpid,
                        );

                        // apply the accel to the velocity
                        fvx += ax * p.tstep;
                        fvy += ay * p.tstep;

                        if p.valid_len < 2 && (fvx == 0.0 || fvy == 0.0) {
                            break;
                        }

                        // apply the velocity to the position
                        fx += fvx * p.tstep;
                        fy += fvy * p.tstep;

                        if p.collidable {
                            let cobj = self.c_collides.get(&p.objid).unwrap();
                            // check for collision
                            for (cid, col) in &self.c_collider {
                                if !col.stop_col {
                                    continue
                                }
                                let cpos = &self.c_pos.get(cid).unwrap();
                                let dcx = fx - cpos.x;
                                let dcy = fy - cpos.y;
                                let rdist = col.rad + cobj.rad;
                                if (rdist * rdist) >= (dcx * dcx) + (dcy * dcy) {
                                    hit_something = true;
                                }
                            }
                        }
                    }

                    // if we have an assoicated CDrawable, update the mesh based on the points
                    if let Some(mut d) = self.c_drawable.get_mut(&id) {
                        d.thing = gen_fading_path(ctx, &p.pts[..p.valid_len], PRED_SIZE/self.cam.s, p.color);
                    }
                },
                None => {
                    // item must have been destroyed, and we should be too
                    for e in &mut self.entities {
                        if e.id == *id {
                            e.to_destroy = true;
                            break;
                        }
                    }
                },
            }
        }
    }

    fn s_move(&mut self, _ctx: &mut Context, dt: f64) {
        
        // for each dyn object for each gravity object in range
        for (id, d) in &mut self.c_dynamic {
            let p = &self.c_pos[id];

            let (mut ax, mut ay, _) = State::get_grav_a(&self.c_grav, &self.c_pos, p.x, p.y, id);
            //self.grav_count = count;
            ax += d.in_ax;
            ay += d.in_ay;

            // apply the accel to the velocity
            d.x_vel += ax * dt;
            d.y_vel += ay * dt;

            // apply the velocity to the position
            let p = self.c_pos.get_mut(id).unwrap();

            p.x += d.x_vel * dt;
            p.y += d.y_vel * dt;

            // do rotational vel to the rotation as well
            //TODO
        }
    }

    fn s_collision(&mut self, ctx: &mut Context, _dt: f64) {
        let mut qe: Vec<(f64, f64, f32)> = Vec::new();
        for (id, cobj) in &self.c_collides {
            // check position against colliders
            //TODO have different rates at which things check for collision?

            let p = &self.c_pos[id];
            for (cid, c) in &self.c_collider {
                if *cid == *id {
                    continue;
                }
                let colpos = &self.c_pos.get(cid).unwrap();
                
                let dx = colpos.x - p.x;
                let dy = colpos.y - p.y;
                let rdist = c.rad + cobj.rad;

                if (rdist * rdist) >= ((dx * dx) + (dy * dy)) {
                    // collided
                    match c.col_action {
                        CollisionType::Explosion(sz, delself) => {
                            // queue spawn explosion
                            let mut otherdie = false;
                            // if the other collides had a collider
                            // check if it wants an explosion as well
                            if let Some(other_col) = self.c_collider.get(id) {
                                if let CollisionType::Explosion(othersz, ds) = other_col.col_action {
                                    if ds { // if the otherone wanted to go out on a hit, it wants to explode
                                        qe.push((p.x, p.y, othersz));
                                        otherdie = true;
                                    }
                                }
                            }

                            if !otherdie || delself {
                                if delself {
                                    qe.push((colpos.x, colpos.y, sz));
                                } else {
                                    qe.push((p.x, p.y, sz));
                                }
                            }
                            
                            for e in &mut self.entities {
                                if delself && e.id == *cid {
                                    e.to_destroy = true;
                                }
                                if e.id == *id {
                                    e.to_destroy = true;
                                    break;
                                }
                            }
                        },
                        CollisionType::FuelPup(amt) => {
                            if let Some(ship) = &mut self.c_ship.get_mut(id) {
                                ship.fuel += amt;
                                for e in &mut self.entities {
                                    if e.id == *cid {
                                        e.to_destroy = true;
                                        break;
                                    }
                                }
                            }
                        },
                        CollisionType::Portal => {
                            if let Some(_) = &self.c_ship.get_mut(id) {
                                self.finished = true;
                            }
                        },
                        CollisionType::None => (),
                    }

                    break;
                }
            }
        }
        for (px, py, sz) in qe {
            self.spawn_explosion(ctx, px, py, sz, true);
        }
    }

    fn s_player(&mut self, ctx: &mut Context, dt: f64) {
        // apply inputs
        // rotate to follow mouse
        let mut launch_nuke = false;
        let mut npx = 0.0;
        let mut npy = 0.0;
        let mut na = 0.0;
        let mut nxv = 0.0;
        let mut nyv = 0.0;

        if let Some(ref pid) = self.playerid {
            let px;
            let py;
            let pa;
            let (mx, my) = self.cam.cam2world(&graphics::screen_coordinates(ctx), self.input.mx, self.input.my);
            {
                let p = self.c_pos.get_mut(pid).unwrap();
                px = p.x;
                py = p.y;

                // TODO use angular accelaration to rotate, don't just snap to mouse
                // tan = o/a
                let dx = px - mx;
                let dy = py - my;
                p.a = dy.atan2(dx) as f32;
                pa = p.a;
            }


            // accel based on mouse
            let d = self.c_dynamic.get_mut(pid).unwrap();
            let s = self.c_ship.get_mut(pid).unwrap();

            d.in_ax = 0.0;
            d.in_ay = 0.0;

            let tamt = if s.fuel > 0.0 {
                s.thrust
            } else {
                s.empty_thrust
            };
            //TODO taper thrust by mouse position
            if self.input.rmb {
                d.in_ax = -pa.cos() as f64 * tamt;
                d.in_ay = -pa.sin() as f64 * tamt;
            }

            s.fuel -= (d.in_ax + d.in_ay).abs() * dt;
            if s.fuel < 0.0 {
                s.fuel = 0.0;
            }

            if self.input.lmb  && s.ammo > 0 {
                let nd = PLAYER_NUKE_DIST;
                let pa_x = -pa.cos() as f64;
                let pa_y = -pa.sin() as f64;
                npx = px + (pa_x * nd);
                npy = py + (pa_y * nd);
                na = pa;
                nxv = d.x_vel + (pa_x * PLAYER_NUKE_IVEL);
                nyv = d.y_vel + (pa_y * PLAYER_NUKE_IVEL);
                launch_nuke = true;
                
                self.input.lmb = false;
                s.ammo -= 1;
            }
        }

        if launch_nuke {
            self.spawn_nuke(
                ctx,
                npx,
                npy,
                na,
                nxv,
                nyv,
                PLAYER_NUKE_THRUST, // thrust
                None, // target
                PLAYER_NUKE_SIZE, // explosion size
            );
        }
    }

    fn s_player_cam(&mut self) {
        if let Some(ref pid) = self.playerid {
            let p = &self.c_pos.get(pid).unwrap();

            if self.cam.x != p.x || self.cam.y != p.y {
                self.cam.update = true;
                self.cam.x = p.x;
                self.cam.y = p.y;
            }
        }
    }
}

impl ggez::event::EventHandler for State {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        if self.finished {
            self.gen_level(ctx, self.level+1);
        } else if self.input.reset {
            self.gen_level(ctx, 0);
            self.input.reset = false;
        }

        let dt = timer::duration_to_f64(timer::delta(ctx));
        
        //if self.log_time <= timer::ticks(ctx) {
        //    self.log_time = timer::ticks(ctx) + LOG_TICKS;
        //    println!("fps: {}", timer::fps(ctx));
        //    println!("scale: {}", self.cam.s);
        //    println!("grav_count: {}", self.grav_count);
        //    println!(" - ");
        //}

        self.s_player(ctx, dt);
        self.s_move(ctx, dt);
        self.s_collision(ctx, dt);
        self.s_predict(ctx, dt);
        self.s_trail(ctx, dt);
        self.s_turret(ctx, dt);
        self.s_rocket(ctx, dt);
        self.s_explosion(ctx, dt);

        self.s_destroy();

        return Ok(());
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {

        let sc = graphics::screen_coordinates(ctx);
        let dp = graphics::DrawParam::default();

        graphics::clear(ctx, graphics::BLACK);
        
        self.s_player_cam();
        self.cam.do_update(ctx, &sc);

        if !self.started {
            let mut ui = graphics::Text::new(GUIDE);

            ui.set_font(self.font, graphics::Scale{x: 24.0, y: 24.0});

            let (uidx, uidy) = ui.dimensions(ctx);
            let (uidx, uidy) = (uidx as f32, uidy as f32);
            let (uix, uiy) = ((sc.w - uidx)/ 2.0, (sc.h - uidy)/2.0);
            let (uix, uiy) = self.cam.cam2world(&sc, uix * self.cam.s, uiy * self.cam.s);
            graphics::draw(
                ctx,
                &ui,
                dp.dest([uix as f32, uiy as f32]).scale([1.0 / self.cam.s, 1.0 / self.cam.s]),
            ).unwrap();
            graphics::present(ctx)?;
            // yield the CPU?
            timer::yield_now();
            return Ok(())
        }

        for (id, d) in &self.c_drawable {
            let p = &self.c_pos.get(id).expect("Drawables must have a position");
            let mut objr = d.r;
            if let DrawThing::MeshScale(_, ms) = d.thing {
                objr *= ms;
            }

            //don't draw objects off screen
            if !self.cam.is_visible(ctx, &sc, p.x, p.y, objr) {
                continue;
            }
            let item_dp = dp.dest([p.x as f32, p.y as f32]).rotation(p.a);
            d.draw(
                self,
                ctx,
                item_dp,
            )?;
        }

        // draw ui
        if let Some(pid) = self.playerid {
            //let p = self.c_pos.get(&pid).unwrap();
            let s = self.c_ship.get(&pid).unwrap();
            let d = self.c_dynamic.get(&pid).unwrap();
            let mut ui = graphics::Text::new(
                format!(
                    concat!(
                        "/-----------------\\\n",
                        "|   fuel : {:04.0}   |\n",
                        "|   zone : {:02}     |\n",
                        "|    vel : {:04.0}   |\n",
                        "|  nukes : {:02}     |\n",
                        "|  locks : {:02}     |\n",
                        "\\-----------------/\n", 
                    ),
                    s.fuel,
                    self.level,
                    ((d.x_vel * d.x_vel) + (d.y_vel * d.y_vel)).sqrt(),
                    s.ammo,
                    self.locks.len(),
                ),
            );

            ui.set_font(self.font, graphics::Scale{x: 18.0, y: 18.0});

            let (uix, uiy) = self.cam.cam2world(&sc, 15.0 * self.cam.s, 15.0 * self.cam.s);
            graphics::draw(
                ctx,
                &ui,
                dp.dest([uix as f32, uiy as f32]).scale([1.0 / self.cam.s, 1.0 / self.cam.s]),
            ).unwrap();
        }

        graphics::present(ctx)?;

        // yield the CPU?
        timer::yield_now();
        Ok(())
    }

    fn resize_event(&mut self, ctx: &mut Context, w: f32, h: f32) {
        graphics::set_screen_coordinates(ctx,
            graphics::Rect{x: 0.0, y: 0.0, w, h}
        ).unwrap();

        self.cam.update = true;
    }

    fn mouse_motion_event(&mut self, _ctx: &mut Context, x: f32, y: f32, _dx: f32, _dy: f32) {
        self.input.mx = x;
        self.input.my = y;
    }

    fn mouse_wheel_event(&mut self, _ctx: &mut Context, _x: f32, y: f32) {
        let mut s = self.cam.s + (y * ZOOM_AMT);
        s = s * ((y*ZOOM_AMT) + 1.0);

        if s < MIN_CAM_SCALE {
            s = MIN_CAM_SCALE;
        }
        if s > MAX_CAM_SCALE {
            s = MAX_CAM_SCALE;
        }
        self.cam.s = s;
        self.cam.update = true;
    }

    fn mouse_button_down_event(&mut self, _ctx: &mut Context, btn: input::mouse::MouseButton, _x: f32, _y: f32) {
        match btn {
            input::mouse::MouseButton::Left => {
                self.input.lmb = true;
            },
            input::mouse::MouseButton::Right => {
                self.input.rmb = true;
            },
            _ => (),
        }
    }

    fn mouse_button_up_event(&mut self, _ctx: &mut Context, btn: input::mouse::MouseButton, _x: f32, _y: f32) {
        match btn {
            input::mouse::MouseButton::Left => {
                self.input.lmb = false;
            },
            input::mouse::MouseButton::Right => {
                self.input.rmb = false;
            },
            _ => (),
        }
    }

    fn key_down_event(&mut self, ctx: &mut Context, keycode: input::keyboard::KeyCode, _keymods: input::keyboard::KeyMods, _repeat: bool) {
        match keycode {
            input::keyboard::KeyCode::Up |
            input::keyboard::KeyCode::W => {
                self.input.up = true;
            },
            input::keyboard::KeyCode::Down |
            input::keyboard::KeyCode::S => {
                self.input.down = true;
            },
            input::keyboard::KeyCode::Left |
            input::keyboard::KeyCode::A => {
                self.input.left = true;
            },
            input::keyboard::KeyCode::Right |
            input::keyboard::KeyCode::D => {
                self.input.right = true;
            },
            input::keyboard::KeyCode::Q => {
                self.input.ccw = true;
            },
            input::keyboard::KeyCode::E => {
                self.input.cw = true;
            },
            input::keyboard::KeyCode::R => {
                self.input.reset = true;
            },
            input::keyboard::KeyCode::Escape => {
                event::quit(ctx);
            },
            _ => (),
        };
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: input::keyboard::KeyCode, _keymods: input::keyboard::KeyMods) {
        match keycode {
            input::keyboard::KeyCode::Up |
            input::keyboard::KeyCode::W => {
                self.input.up = false;
            },
            input::keyboard::KeyCode::Down |
            input::keyboard::KeyCode::S => {
                self.input.down = false;
            },
            input::keyboard::KeyCode::Left |
            input::keyboard::KeyCode::A => {
                self.input.left = false;
            },
            input::keyboard::KeyCode::Right |
            input::keyboard::KeyCode::D => {
                self.input.right = false;
            },
            input::keyboard::KeyCode::Q => {
                self.input.ccw = false;
            },
            input::keyboard::KeyCode::E => {
                self.input.cw = false;
            },
            input::keyboard::KeyCode::R => {
                self.input.reset = false;
            },
            _ => (),
        };
    }
}

fn gen_fading_path(ctx: &mut Context, pts: &[[f32; 2]], s: f32, color: [f32;4]) -> DrawThing {
    if pts.len() < 3 {
        return DrawThing::Blank;
    }
    let mut verts: Vec<graphics::Vertex> = Vec::new();
    let mut inds: Vec<u32> = Vec::new();

    let mut prev_point = pts[0];
    let mut current_point = pts[0];
    let mut future_point = pts[1];

    let almost_last = pts.len()-1;
    for i in 0..almost_last {
        let c = ((pts.len() - i) as f32) / (pts.len() as f32);
        let mut clr = color;
        clr[3] *= c;

        // how to place points with width
        let mut v1 = graphics::Vertex{
            pos: current_point,
            uv: [0.0, 0.0],
            color: clr,
        };
        let mut v2 = graphics::Vertex{
            pos: current_point,
            uv: [0.0, 0.0],
            color: clr,
        };

        future_point = pts[i+1];

        let dx = future_point[0] - prev_point[0];
        let dy = future_point[1] - prev_point[1];

        let a = dy.atan2(dx);
        let dx = a.cos() * s;
        let dy = a.sin() * s;
        v1.pos[0] += dy;
        v1.pos[1] -= dx;
        v2.pos[0] -= dy;
        v2.pos[1] += dx;

        verts.push(v1);
        verts.push(v2);
        
        prev_point = current_point;
        current_point = future_point;

        let i = i as u32;
        inds.push((i*2)+0);
        inds.push((i*2)+2);
        inds.push((i*2)+1);

        inds.push((i*2)+1);
        inds.push((i*2)+2);
        inds.push((i*2)+3);
    }

    verts.push(graphics::Vertex{
        pos: future_point,
        uv: [0.0, 0.0],
        color: [0.0; 4],
    });
    verts.push(graphics::Vertex{
        pos: future_point,
        uv: [0.0, 0.0],
        color: [0.0; 4],
    });

    let tm = graphics::Mesh::from_raw(
        ctx,
        &verts,
        &inds,
        None,
    );
    return DrawThing::Mesh(tm.unwrap());
}

fn load_mesh(ctx: &mut Context, p: &str, scale: f32, color: [f32; 4]) -> (graphics::Mesh, f32) {
    let f = filesystem::open(ctx, std::path::Path::new(p)).expect("Unable to find mesh file");

    let f = BufReader::new(f);
    // sort of parse a .obj file
    let mut verts: Vec<graphics::Vertex> = Vec::new();
    let mut inds: Vec<u32> = Vec::new();

    let mut uvs: Vec<[f32; 2]> = Vec::new();

    let mut rad = 0.0;

    for line in f.lines() {
        if let Ok(l) = line { 
            let mut i = l.split_ascii_whitespace();
            match i.next() {
                Some("v") => {
                    let x = i.next().unwrap();
                    let y = i.next().unwrap();

                    let x : f32 = x.parse::<f32>().unwrap() * scale;
                    let y : f32 = y.parse::<f32>().unwrap() * scale;

                    let r = ((x*x) + (y*y)).sqrt() as f32;
                    if r > rad {
                        rad = r;
                    }

                    let v = graphics::Vertex{
                        pos: [x, -y],
                        uv: [0.0, 0.0],
                        color: color,
                    };

                    verts.push(v);
                },
                Some("vt") => {
                    let x = i.next().unwrap();
                    let y = i.next().unwrap();

                    let x : f32 = x.parse().unwrap();
                    let y : f32 = y.parse().unwrap();

                    uvs.push([x,y]);
                },
                Some("f") => {
                    let mut i1 = i.next().unwrap().split("/");
                    let i1_v : u32 = i1.next().unwrap().parse::<u32>().expect("Mesh doesn't have uvs") - 1;
                    let i1_uv : u32 = i1.next().unwrap().parse::<u32>().unwrap() - 1;

                    let mut i2 = i.next().unwrap().split("/");
                    let i2_v : u32 = i2.next().unwrap().parse::<u32>().unwrap() - 1;
                    let i2_uv : u32 = i2.next().unwrap().parse::<u32>().unwrap() - 1;

                    let mut i3 = i.next().unwrap().split("/");
                    let i3_v : u32 = i3.next().unwrap().parse::<u32>().unwrap() - 1;
                    let i3_uv :u32 = i3.next().unwrap().parse::<u32>().unwrap() - 1;

                    // add the indicies
                    inds.push(i1_v);
                    inds.push(i2_v);
                    inds.push(i3_v);

                    // associate uvs with positions
                    verts[i1_v as usize].uv = uvs[i1_uv as usize];
                    verts[i2_v as usize].uv = uvs[i2_uv as usize];
                    verts[i3_v as usize].uv = uvs[i3_uv as usize];
                },
                _ => (),
            }
        }
    }

    return (
        graphics::Mesh::from_raw(
            ctx,
            &verts,
            &inds,
            None,
        ).unwrap(),
        rad,
    );
}

pub fn main() {

    println!("Game Starting");

    let c = conf::Conf::new();

    let mut cb = ContextBuilder::new("rusty_raid", "Jordan9001").conf(c);

    if let Ok(rdir) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut path = std::path::PathBuf::from(rdir);
        path.push("resources");
        cb = cb.add_resource_path(path);
    }
    
    let (ref mut ctx, ref mut event_loop) = cb.build().unwrap();

    //graphics::set_default_filter(ctx, graphics::FilterMode::Nearest);
    graphics::set_default_filter(ctx, graphics::FilterMode::Linear);

    graphics::set_mode(ctx, conf::WindowMode {
        width: 1200.0, height: 900.0,
        maximized: false,
        fullscreen_type: conf::FullscreenType::Windowed,
        borderless: false,
        min_width: 0.0, max_width: 0.0, min_height: 0.0, max_height: 0.0,
        resizable: true,
    }).unwrap();

    graphics::set_window_title(ctx, GAME_NAME);

    let mut state = State::new(ctx).unwrap();

    // generate a map

    event::run(ctx, event_loop, &mut state).unwrap();

    println!("Done");
}
