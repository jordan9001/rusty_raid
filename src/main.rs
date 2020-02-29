use std::collections::HashMap;
use ggez::*;
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use std::io::{self, BufReader, BufRead};

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
    a_vel: f32,
}

struct CCollidable {
    rad: f64,
}

enum DrawThing {
    Mesh(graphics::Mesh),
    MeshInd(usize),
}

struct CDrawable {
    thing: DrawThing,
}

impl CDrawable {
    fn draw(&self, st: &State, ctx: &mut Context, param: graphics::DrawParam) -> error::GameResult {
        return graphics::draw(
            ctx,
            match self.thing {
                DrawThing::Mesh(ref m) => m,
                DrawThing::MeshInd(i) => &st.meshs[i],
            },
            param,
        );
    }
}

struct InputState {
    up: bool,
    down: bool,
    right: bool,
    left: bool,
    cw: bool,
    ccw: bool,
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

        //TODO figureout how to apply roataion?

        graphics::apply_transformations(ctx).unwrap();
        self.update = false;

        return true;
    }

    fn is_visible(&self, _ctx: &mut Context, sc: &graphics::Rect, x: f64, y: f64) -> bool {
        let (cx, cy) = Camera::world2cam(self, sc, x, y);

        //TODO take in radius as well, to not have thing pop in on the edges
        
        if cx < 0.0 || cx > sc.w || cy < 0.0 || cy > sc.h {
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
    meshs: Vec<graphics::Mesh>,
    rng: SmallRng,

    cam: Camera,

    next_id: IdVal,
    entities: Vec<Entity>,
    c_pos: HashMap<IdVal, CPos>,
    c_grav: HashMap<IdVal, CGrav>,
    c_dynamic: HashMap<IdVal, CDynamic>,
    c_collidable: HashMap<IdVal, CCollidable>,
    c_drawable: HashMap<IdVal, CDrawable>,

    input: InputState,

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

            input: InputState{
                up: false,
                down: false,
                left: false,
                right: false,
                cw: false,
                ccw: false,
            },

            //DEBUG
            log_time: 0,
        };

        Ok(s)
    }

    fn add_entity(&mut self) -> IdVal {
        let id = self.next_id;
        self.next_id += 1;

        self.entities.push(
            Entity{id}
        );

        return id;
    }

    fn gen_level(&mut self, ctx: &mut Context) {
        //TODO make this good

        // for now here are 2 suns, and a ship floating between them
        
        self.add_star(ctx, -20.0, 0.0, 10.0);
        self.add_star(ctx, 20.0, 0.0, 10.0);

        self.add_ship(ctx, MeshNum::AngMesh, 0.0, 10.0);
        //TODO add player control
    }

    fn add_star(&mut self, ctx: &mut Context, x: f64, y: f64, size: f64) -> IdVal {
        let id = self.add_entity();

        self.c_pos.insert(
            id,
            CPos{x, y, a: 0.0},
        );
        self.c_grav.insert(
            id,
            CGrav{mass: 1000.0, dist2: std::f64::INFINITY},
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
        self.c_drawable.insert(
            id,
            CDrawable{
                thing: DrawThing::MeshInd(m as usize),
            },
        );
        self.c_dynamic.insert(
            id,
            CDynamic {
                mass: 10.0,
                x_vel: 0.0,
                y_vel: 0.0,
                a_vel: 0.0,
            },
        );

        return id;
    }

    fn s_move(&mut self, _ctx: &mut Context, dt: f64) {
        // for each dyn object for each gravity object in range
        for (id, d) in &mut self.c_dynamic {
            let p = &self.c_pos[id];

            let mut ax: f64 = 0.0;
            let mut ay: f64 = 0.0;

            for (gid, g) in &self.c_grav {
                if gid == id {
                    continue;
                }

                let gp = &self.c_pos[gid];

                let dx = gp.x - p.x;
                let dy = gp.y - p.y;

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
}

impl ggez::event::EventHandler for State {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        // TODO state machine here in each of these to call only the appropriate ones
        let dt = timer::duration_to_f64(timer::delta(ctx));
        
        if self.log_time <= timer::ticks(ctx) {
            self.log_time = timer::ticks(ctx) + 100;
            println!("fps: {}", timer::fps(ctx));

            let shipid = self.entities[2].id;
            let CPos {x: psx, y: psy, a: _} = self.c_pos[&shipid];
            println!("{} at {}, {}", shipid, psx, psy);
        }

        self.s_move(ctx, dt);

        //DEBUG move view
        let k = 15.0 * dt;

        if self.input.left {
            self.cam.x -= k;
            self.cam.update = true;
        } else if self.input.right {
            self.cam.x += k;
            self.cam.update = true;
        }

        if self.input.up {
            self.cam.y -= k;
            self.cam.update = true;
        } else if self.input.down {
            self.cam.y += k;
            self.cam.update = true;
        }

        if self.input.cw {
            self.cam.a += 0.1;
            self.cam.update = true;
        } else if self.input.ccw {
            self.cam.a -= 0.1;
            self.cam.update = true;
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {

        let sc = graphics::screen_coordinates(ctx);
        let dp = graphics::DrawParam::default();

        graphics::clear(ctx, graphics::BLACK);

        for (id, d) in &self.c_drawable {
            let p = &self.c_pos[id];

            //don't draw objects off screen
            if !self.cam.is_visible(ctx, &sc, p.x, p.y) {
                continue;
            }
                        
            d.draw(
                self,
                ctx,
                dp.dest([p.x as f32, p.y as f32]).rotation(p.a),
            )?;
        }

        self.cam.do_update(ctx, &sc);
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

    fn mouse_wheel_event(&mut self, _ctx: &mut Context, _x: f32, y: f32) {
        let mut s = self.cam.s + (y * 0.1);
        if s < 0.1 {
            s = 0.1;
        }
        s = s * ((y*0.1) + 1.0);
        self.cam.s = s;
        self.cam.update = true;
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

fn load_mesh(ctx: &mut Context, p: &str) -> graphics::Mesh {
    let f = filesystem::open(ctx, std::path::Path::new(p)).unwrap();
    let f = BufReader::new(f);
    // sort of parse a .obj file
    let mut verts: Vec<graphics::Vertex> = Vec::new();
    let mut inds: Vec<u32> = Vec::new();

    let mut uvs: Vec<[f32; 2]> = Vec::new();

    for line in f.lines() {
        if let Ok(l) = line { 
            let mut i = l.split_ascii_whitespace();
            match i.next() {
                Some("v") => {
                    let x = i.next().unwrap();
                    let y = i.next().unwrap();

                    let x : f32 = x.parse().unwrap();
                    let y : f32 = y.parse().unwrap();

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

    graphics::Mesh::from_raw(
        ctx,
        &verts,
        &inds,
        None,
    ).unwrap()
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
