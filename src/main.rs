use std::collections::HashMap;
use ggez::*;
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use std::io::{BufReader, BufRead};

// Doing a ECS organization

type IdVal = usize;

struct Entity {
    id: IdVal,
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
    mass: f64,
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
}

struct CCollidable {
    rad: f64,
    action: fn(IdVal),
}

enum DrawThing {
    Mesh(graphics::Mesh),
    MeshInd(usize),
}

struct CDrawable {
    thing: DrawThing,
    r: f32, // radius for culling
}

impl CDrawable {
    fn draw(&self, st: &State, ctx: &mut Context, param: graphics::DrawParam) -> error::GameResult {
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
    a: f32,

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

    //DEBUG
    log_time: usize,
}

impl State {
    fn new(ctx: &mut Context) -> ggez::GameResult<State> {
        let s = State{
            meshs: [
                    "\\ang.obj",
                    "\\A.obj",
                    "\\ast.obj",
                    "\\bangv.obj",
                    "\\capital.obj",
                    "\\pnd.obj",
                ].iter().map(
                |x| load_mesh(ctx, x)
            ).collect(),
            rng: SmallRng::from_entropy(),

            cam: Camera{
                x: 0.0,
                y: 0.0,
                s: 1.0,
                a: 0.0,
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

            //DEBUG
            log_time: 0,
        };

        Ok(s)
    }

    fn gen_level(&mut self, ctx: &mut Context) {
        //TODO make this good

        for _ in 0..100 {
            let x = self.rng.gen_range(-900.0, 900.0);
            let y = self.rng.gen_range(-900.0, 900.0);
            let s = self.rng.gen_range(9.0, 45.0);

            self.add_star(
                ctx,
                x, y,
                s,
            );
        }
        self.add_star(ctx, -20.0, 0.0, 10.0);
        self.add_star(ctx, 20.0, 0.0, 10.0);

        let shipid = self.add_ship(ctx, MeshNum::AngMesh, 1200.0, 0.0);
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
            Entity{id}
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
        self.c_grav.insert(
            id,
            CGrav{mass:  2.1 * size * size * size, dist2: std::f64::INFINITY},
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
            },
        );

        return id;
    }

    fn add_ship(&mut self, _ctx: &mut Context, m: MeshNum, x: f64, y: f64) -> IdVal {
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
            },
        );
        self.c_dynamic.insert(
            id,
            CDynamic {
                mass: 10.0,
                x_vel: 0.0,
                y_vel: 0.0,
                in_ax: 0.0,
                in_ay: 0.0,
            },
        );
        self.c_ship.insert(
            id,
            CShip {
                thrust: 9.0,
            }
        );

        return id;
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

            let obj = self.c_dynamic.get(&p.objid).expect("Predictables.objid must have dynamic");
            let objp = self.c_pos.get(&p.objid).expect("Predictables.objid must have pos");

            let mut fx = objp.x;
            let mut fy = objp.y;
            let mut fvx = obj.x_vel;
            let mut fvy = obj.y_vel;

            p.valid_len = 0;
            for pt in &mut p.pts {
                //fill out the points

                let (ax, ay) = State::get_grav_a(&self.c_grav, &self.c_pos, fx, fy, id);

                // apply the accel to the velocity
                fvx += ax * p.tstep;
                fvy += ay * p.tstep;

                // apply the velocity to the position
                fx += fvx * p.tstep;
                fy += fvy * p.tstep;

                let dx = fx - objp.x;
                let dy = fy - objp.y;

                if p.valid_len >= 2 && (dx * dx) + (dy * dy) > p.max_dist2 {
                    break;
                }

                p.valid_len += 1;
                *pt = [fx as f32, fy as f32];
            }

            // if we have an assoicated CDrawable, update the mesh based on the points
            if let Some(d) = self.c_drawable.get_mut(&id) {
                if let DrawThing::Mesh(ref mut m) = d.thing {
                    //TODO update the mesh with the points
                    *m = graphics::Mesh::new_line(
                        ctx,
                        &p.pts[..p.valid_len],
                        1.0 / self.cam.s,
                        graphics::WHITE,
                    ).unwrap();
                }
            }
        }
    }

    fn get_grav_a(gravs: &HashMap<IdVal, CGrav>, pos: &HashMap<IdVal, CPos>, px: f64, py: f64, id: &IdVal) -> (f64, f64) {
        let mut ax: f64 = 0.0;
        let mut ay: f64 = 0.0;

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

            let r = r2.sqrt();
            let r3 = r2 * r;

            // get accelaration due to this item
            let ga = g.mass / r3;
            ax += ga * dx;
            ay += ga * dy;
        }

        return (ax, ay);
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
            
        }
    }

    fn s_player(&mut self, ctx: &mut Context) {
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
            let s = self.c_ship.get(pid).unwrap();
            //TODO taper thrust by mouse position
            if self.input.rmb {
                d.in_ax = -p.a.cos() as f64 * s.thrust;
                d.in_ay = -p.a.sin() as f64 * s.thrust;
            } else {
                d.in_ax = 0.0;
                d.in_ay = 0.0;
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
}

impl ggez::event::EventHandler for State {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        // TODO state machine here in each of these to call only the appropriate ones
        let dt = timer::duration_to_f64(timer::delta(ctx));
        
        if self.log_time <= timer::ticks(ctx) {
            self.log_time = timer::ticks(ctx) + 100;
            println!("fps: {}", timer::fps(ctx));
            if let Some(pid) = self.playerid {
                let p = self.c_pos.get(&pid).unwrap();
                println!("player       : {},{}", p.x, p.y);
            }
        }

        self.s_player(ctx);
        self.s_move(ctx, dt);
        self.s_predict(ctx, dt);

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
                        
            d.draw(
                self,
                ctx,
                dp.dest([p.x as f32, p.y as f32]).rotation(p.a),
            )?;
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
        let mut s = self.cam.s + (y * 0.1);
        if s < 0.1 {
            s = 0.1;
        }
        s = s * ((y*0.1) + 1.0);
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

fn load_mesh(ctx: &mut Context, p: &str) -> (graphics::Mesh, f32) {
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

                    let x : f32 = x.parse().unwrap();
                    let y : f32 = y.parse().unwrap();

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
        println!("Adding resource path: {:?}", path);
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

    graphics::set_window_title(ctx, "Falling Up");

    let mut state = State::new(ctx).unwrap();

    // generate a map
    state.gen_level(ctx);

    event::run(ctx, event_loop, &mut state).unwrap();

    println!("Done");
}
