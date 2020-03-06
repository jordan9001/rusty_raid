use std::collections::HashMap;
use ggez::*;
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use std::io::{BufReader, BufRead};

// constants

const GAME_NAME: &str = "Falling Up";
const STAR_GRAV_MUL: f64 = 1.5;
const SHIP_SCALE: f32 = 10.0;
const POWERUP_SCALE: f32 = 100.0;
const SHIP_MINSZ: f32 = 1.32;
const GRAV_REACH: f64 = 1.0 / 30000.0;
const MAX_CAM_SCALE: f32 = 90.0;
const MIN_CAM_SCALE: f32 = 0.36;
const ZOOM_AMT: f32 = 0.06;
const LOG_TICKS: usize = 81;

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

// Used for predictions on movement
struct CPredictable {
    objid: IdVal,
    pts: Vec<[f32; 2]>,
    tstep: f64,
    rate: f64,
    till_next: f64,
    max_dist2: f64,
    valid_len: usize,
    collidable: bool,
}

enum CollisionType {
    Explosion(f64),
    FuelPup(f64),
}

struct CCollidable {
    rad2: f64,
    col_action: CollisionType,
    stop_col: bool,
}

enum DrawThing {
    Mesh(graphics::Mesh),
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
            DrawThing::Mesh(ref m) => {
                return graphics::draw(ctx, m, param);
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
    fuel: f64,
}

struct InputState {
    up: bool,
    down: bool,
    right: bool,
    left: bool,
    cw: bool,
    ccw: bool,
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
}

struct State {
    meshs: Vec<(graphics::Mesh, f32)>,
    rng: SmallRng,

    cam: Camera,

    next_id: IdVal,
    entities: Vec<Entity>,
    c_pos: HashMap<IdVal, CPos>,
    c_grav: HashMap<IdVal, CGrav>,
    c_dynamic: HashMap<IdVal, CDynamic>,
    c_collidable: HashMap<IdVal, CCollidable>,
    c_drawable: HashMap<IdVal, CDrawable>,
    c_predictable: HashMap<IdVal, CPredictable>,
    c_ship: HashMap<IdVal, CShip>,

    input: InputState,
    playerid: Option<IdVal>,

    font: graphics::Font,

    //DEBUG
    log_time: usize,
}

impl State {
    fn new(ctx: &mut Context) -> ggez::GameResult<State> {
        let s = State{
            meshs: [
                    ("\\ang.obj", SHIP_SCALE),
                    ("\\A.obj", SHIP_SCALE),
                    ("\\ast.obj", POWERUP_SCALE),
                    ("\\bangv.obj", SHIP_SCALE),
                    ("\\capital.obj", SHIP_SCALE),
                    ("\\pnd.obj", POWERUP_SCALE),
                ].iter().map(
                |x| load_mesh(ctx, x.0, x.1)
            ).collect(),
            rng: SmallRng::from_entropy(),

            cam: Camera{
                x: 0.0,
                y: 0.0,
                s: 1.0,
                update: true,
            },

            next_id: 1,
            entities: Vec::new(),
            c_pos: HashMap::new(),
            c_grav: HashMap::new(),
            c_dynamic: HashMap::new(),
            c_collidable: HashMap::new(),
            c_drawable: HashMap::new(),
            c_predictable: HashMap::new(),
            c_ship: HashMap::new(),

            input: InputState{
                up: false,
                down: false,
                left: false,
                right: false,
                cw: false,
                ccw: false,
                mx: 0.0,
                my: 0.0,
                lmb: false,
                rmb: false,
            },
            playerid: None,

            font: graphics::Font::new(ctx, "\\NovaMono-Regular.ttf").unwrap(),

            //DEBUG
            log_time: 0,
        };

        Ok(s)
    }

    fn gen_level(&mut self, ctx: &mut Context) {
        //TODO make this good

        // add stars
        let mut i = 0;
        'star_loop: while i < 300 {
            let a = self.rng.gen_range(0.0, std::f64::consts::PI * 2.0);
            let d = self.rng.gen_range(900.0, 3300.0);

            let x = d * a.cos();
            let y = d * a.sin();
            let s = self.rng.gen_range(9.0, 90.0);

            for (cid, c) in &self.c_collidable {
                let colpos = &self.c_pos.get(cid).unwrap();
                
                let dx = colpos.x - x;
                let dy = colpos.y - y;

                if (c.rad2 + (s*s)) > ((dx * dx) + (dy * dy)) {
                    // would overlap
                    continue 'star_loop;
                }
            }

            self.add_star(
                ctx,
                x, y,
                s,
            );

            i += 1;
        }

        let mut i = 0;
        'fuel_loop: while i < 30 {
            let a = self.rng.gen_range(0.0, std::f64::consts::PI * 2.0);
            let d = self.rng.gen_range(0.0, 2400.0);

            let x = d * a.cos();
            let y = d * a.sin();
            let s = self.rng.gen_range(9.0, 90.0);

            for (cid, c) in &self.c_collidable {
                let colpos = &self.c_pos.get(cid).unwrap();
                
                let dx = colpos.x - x;
                let dy = colpos.y - y;

                if c.rad2 > ((dx * dx) + (dy * dy)) {
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

        let shipid = self.add_ship(ctx, MeshNum::AngMesh, 3300.0, 1500.0, 30.0, 300.0);
        self.make_player(shipid);

        // add prediction on the player ship
        self.add_prediction(ctx, shipid, true);
        
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

    fn add_fuel_powerup(&mut self, ctx: &mut Context, x: f64, y: f64) -> IdVal {
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
        self.c_collidable.insert(
            id,
            CCollidable{
                rad2: (r*r) as f64,
                col_action: CollisionType::FuelPup(100.0),
                stop_col: false,
            },
        );
        self.c_pos.insert(
            id,
            CPos{x, y, a: 0.0},
        );

        return id;
    }

    fn add_prediction(&mut self, ctx: &mut Context, objid: IdVal, drawable: bool) -> IdVal {
        let p_id = self.add_entity();

        let mut ptsvec = Vec::new();
        //Add points
        for _ in 0..100 {
            ptsvec.push([0.0,0.0]);
        }
        self.c_predictable.insert(
            p_id,
            CPredictable{
                objid,
                pts: ptsvec,
                tstep: 0.21,
                rate: 0.0,
                till_next: 0.0,
                max_dist2: 3000000.0,
                valid_len: 0,
                collidable: true,
            },
        );
        if drawable {
            self.c_drawable.insert(
                p_id,
                CDrawable{
                    thing: DrawThing::Mesh( //TODO is there a better way to allocate space? Use heap?
                        graphics::Mesh::new_rectangle(
                            ctx,
                            graphics::DrawMode::fill(),
                            graphics::Rect{x: -0.0, y: -0.0, w: 0.0, h: 0.0},
                            graphics::BLACK,
                        ).unwrap(),
                    ),
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

    fn add_star(&mut self, ctx: &mut Context, x: f64, y: f64, size: f64) -> IdVal {
        let id = self.add_entity();

        self.c_pos.insert(
            id,
            CPos{x, y, a: 0.0},
        );
        let mass = STAR_GRAV_MUL * size * size * size;
        self.c_grav.insert(
            id,
            CGrav{mass, dist2: mass * mass * GRAV_REACH},
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
                        0.1,
                        graphics::WHITE,
                    ).unwrap()
                ),
                r: size as f32,
                minsz: 0.0,
            },
        );
        self.c_collidable.insert(
            id,
            CCollidable{
                rad2: size * size,
                col_action: CollisionType::Explosion(100.0),
                stop_col: true,
            },
        );

        return id;
    }

    fn add_ship(&mut self, _ctx: &mut Context, m: MeshNum, x: f64, y: f64, thrust: f64, fuel: f64) -> IdVal {
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
        self.c_ship.insert(
            id,
            CShip {
                thrust,
                fuel,
            }
        );

        return id;
    }

    fn get_grav_a(gravs: &HashMap<IdVal, CGrav>, pos: &HashMap<IdVal, CPos>, px: f64, py: f64, id: &IdVal) -> (f64, f64) {
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

        return (ax, ay);
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
                        let (ax, ay) = State::get_grav_a(
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
                            // check for collision
                            for (cid, col) in &self.c_collidable {
                                if !col.stop_col {
                                    continue
                                }
                                let cpos = &self.c_pos.get(cid).unwrap();
                                let dcx = fx - cpos.x;
                                let dcy = fy - cpos.y;
                                if col.rad2 >= (dcx * dcx) + (dcy * dcy) {
                                    hit_something = true;
                                }
                            }
                        }

                        let dx = fx - objp.x;
                        let dy = fy - objp.y;

                        if (dx * dx) + (dy * dy) > p.max_dist2 {
                            break;
                        }
                    }

                    // if we have an assoicated CDrawable, update the mesh based on the points
                    let mut updated_mesh = false;
                    if p.valid_len >= 2 {
                        if let Some(d) = self.c_drawable.get_mut(&id) {
                            if let DrawThing::Mesh(ref mut m) = d.thing {
                                //TODO update the mesh with the points
                                let tm = graphics::Mesh::new_line(
                                    ctx,
                                    &p.pts[..p.valid_len],
                                    1.0 / self.cam.s,
                                    graphics::WHITE,
                                );
                                if let Ok(tmm) = tm {
                                    *m = tmm;
                                    updated_mesh = true;
                                }
                            }
                        }
                    }
                    if !updated_mesh {
                        //TODO get rid of it?
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

            let (mut ax, mut ay) = State::get_grav_a(&self.c_grav, &self.c_pos, p.x, p.y, id);
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
            
            // check position against collidables
            //TODO have different rates at which things check for collision?

            let p = &self.c_pos[id];
            for (cid, c) in &self.c_collidable {
                let colpos = &self.c_pos.get(cid).unwrap();
                
                let dx = colpos.x - p.x;
                let dy = colpos.y - p.y;

                if c.rad2 >= ((dx * dx) + (dy * dy)) {
                    // collided
                    match c.col_action {
                        CollisionType::Explosion(sz) => {
                            //TODO spawn explosion
                            for e in &mut self.entities {
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
                        }
                    }

                    break;
                }
            }
        }
    }

    fn s_player(&mut self, ctx: &mut Context, dt: f64) {
        // apply inputs
        // rotate to follow mouse
        if let Some(ref pid) = self.playerid {
            let p = self.c_pos.get_mut(pid).unwrap();

            // TODO use angular accelaration to rotate, don't just snap to mouse
            // tan = o/a
            let (mx, my) = self.cam.cam2world(&graphics::screen_coordinates(ctx), self.input.mx, self.input.my);
            let dx = p.x - mx;
            let dy = p.y - my;
            p.a = dy.atan2(dx) as f32;


            // accel based on mouse
            let d = self.c_dynamic.get_mut(pid).unwrap();
            let s = self.c_ship.get_mut(pid).unwrap();

            d.in_ax = 0.0;
            d.in_ay = 0.0;

            if s.fuel > 0.0 {
                //TODO taper thrust by mouse position
                if self.input.rmb {
                    d.in_ax = -p.a.cos() as f64 * s.thrust;
                    d.in_ay = -p.a.sin() as f64 * s.thrust;
                }

                s.fuel -= (d.in_ax + d.in_ay).abs() * dt;
                if s.fuel < 0.0 {
                    s.fuel = 0.0;
                }
            }
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

            self.c_pos.remove(&e.id);
            self.c_grav.remove(&e.id);
            self.c_dynamic.remove(&e.id);
            self.c_collidable.remove(&e.id);
            self.c_drawable.remove(&e.id);
            self.c_predictable.remove(&e.id);
            self.c_ship.remove(&e.id);

            self.entities.remove(i);
        }
    }
}

impl ggez::event::EventHandler for State {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        // TODO state machine here in each of these to call only the appropriate ones
        let dt = timer::duration_to_f64(timer::delta(ctx));
        
        if self.log_time <= timer::ticks(ctx) {
            self.log_time = timer::ticks(ctx) + LOG_TICKS;
            println!("fps: {}", timer::fps(ctx));
            println!("scale: {}", self.cam.s);
        }

        self.s_player(ctx, dt);
        self.s_move(ctx, dt);
        self.s_predict(ctx, dt);

        self.s_destroy();

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {

        let sc = graphics::screen_coordinates(ctx);
        let dp = graphics::DrawParam::default();

        graphics::clear(ctx, graphics::BLACK);

        self.s_player_cam();
        self.cam.do_update(ctx, &sc);

        for (id, d) in &self.c_drawable {
            let p = &self.c_pos.get(id).expect("Drawables must have a position");

            //don't draw objects off screen
            if !self.cam.is_visible(ctx, &sc, p.x, p.y, d.r) {
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
            let mut ui = graphics::Text::new(
                format!(
                    concat!(
                        "/--------------\\\n",
                        "|  fuel : {:04.0} |\n",
                        "\\--------------/\n", 
                    ),
                    s.fuel,
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

    fn key_down_event(&mut self, _ctx: &mut Context, keycode: input::keyboard::KeyCode, _keymods: input::keyboard::KeyMods, _repeat: bool) {
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
            _ => (),
        };
    }
}

fn load_mesh(ctx: &mut Context, p: &str, scale: f32) -> (graphics::Mesh, f32) {
    let f = filesystem::open(ctx, std::path::Path::new(p)).unwrap();
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
                        color: [1.0; 4],
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
                    let i1_v : u32 = i1.next().unwrap().parse::<u32>().unwrap() - 1;
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

    graphics::set_default_filter(ctx, graphics::FilterMode::Nearest);

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
    state.gen_level(ctx);

    event::run(ctx, event_loop, &mut state).unwrap();

    println!("Done");
}
