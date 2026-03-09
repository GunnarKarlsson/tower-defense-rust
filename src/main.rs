use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::window::{PrimaryWindow, Window, WindowPlugin};

const WINDOW_WIDTH: f32 = 900.;
const WINDOW_HEIGHT: f32 = 600.;

// Grid: 60 columns x 30 rows; world spans [-400,400] x [-200,200]
const GRID_WIDTH: u32 = 60;
const GRID_HEIGHT: u32 = 30;
const WORLD_WIDTH: f32 = 800.;
const WORLD_HEIGHT: f32 = 400.;

fn cell_width() -> f32 {
    WORLD_WIDTH / GRID_WIDTH as f32
}
fn cell_height() -> f32 {
    WORLD_HEIGHT / GRID_HEIGHT as f32
}

/// World position to grid cell (col, row). Clamped to grid bounds.
fn world_to_cell(world_pos: Vec2) -> (u32, u32) {
    let ox = world_pos.x + WORLD_WIDTH / 2.0;
    let oy = world_pos.y + WORLD_HEIGHT / 2.0;
    let cw = cell_width();
    let ch = cell_height();
    let col = (ox / cw).floor() as i32;
    let row = (oy / ch).floor() as i32;
    let col = col.clamp(0, GRID_WIDTH as i32 - 1) as u32;
    let row = row.clamp(0, GRID_HEIGHT as i32 - 1) as u32;
    (col, row)
}

/// Center of the cell in world coordinates.
fn cell_center(col: u32, row: u32) -> Vec2 {
    let cw = cell_width();
    let ch = cell_height();
    Vec2::new(
        -WORLD_WIDTH / 2.0 + (col as f32 + 0.5) * cw,
        -WORLD_HEIGHT / 2.0 + (row as f32 + 0.5) * ch,
    )
}

const TOWER_RANGE: f32 = 120.;
const TOWER_FIRE_RATE: f32 = 0.7; // seconds
const BULLET_SPEED: f32 = 400.;
const ENEMY_SPEED: f32 = 80.;

#[derive(Component)]
struct Enemy {
    hp: f32,
    path_idx: usize,
}

#[derive(Component)]
struct Tower {
    timer: Timer,
}

#[derive(Component)]
struct TowerCell(u32, u32);

#[derive(Component)]
struct Bullet {
    target: Entity,
    damage: f32,
}

#[derive(Resource)]
struct Path {
    /// Orthogonal path as grid cells (consecutive cells differ by 1 in col or row).
    cells: Vec<(u32, u32)>,
    /// World positions of path cells (derived from cells for drawing and movement).
    world_points: Vec<Vec2>,
}

#[derive(Resource)]
struct ShowGrid(bool);

#[derive(Component)]
struct GridLine;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::rgb(0.06, 0.06, 0.08)))
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Tower Defense Demo".into(),
                        resolution: (WINDOW_WIDTH, WINDOW_HEIGHT).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource({
            // Orthogonal path: row 15 right, then down, then right to end (fits 60x30 grid)
            let cells: Vec<(u32, u32)> = (0..=30).map(|c| (c, 15)).chain((16..=22).map(|r| (30, r))).chain((31..=59).map(|c| (c, 22))).collect();
            let world_points: Vec<Vec2> = cells.iter().map(|&(c, r)| cell_center(c, r)).collect();
            Path { cells, world_points }
        })
        .insert_resource(ShowGrid(true))
        .add_systems(Startup, setup)
        .add_systems(Startup, spawn_grid_lines)
        .add_systems(Update, (
            toggle_grid_visibility,
            spawn_enemy_periodic,
            enemy_follow_path,
            tower_shooting,
            move_bullets,
            bullet_hit_detection,
            place_tower_on_click,
        ))
        .run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<ColorMaterial>>, path: Res<Path>) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Draw path lines (simple circles at points + lines)
    for p in &path.world_points {
        commands.spawn(MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(8.).into()).into(),
            material: mats.add(ColorMaterial::from(Color::rgb(0.9, 0.8, 0.4))),
            transform: Transform::from_translation(Vec3::new(p.x, p.y, 0.)),
            ..default()
        });
    }
    for window in path.world_points.windows(2) {
        let a = window[0];
        let b = window[1];
        let dir = b - a;
        let len = dir.length();
        let angle = dir.y.atan2(dir.x);
        let rect = Mesh::from(shape::Quad::new(Vec2::new(len, 6.)));
        commands.spawn(MaterialMesh2dBundle {
            mesh: meshes.add(rect).into(),
            material: mats.add(ColorMaterial::from(Color::rgb(0.9, 0.8, 0.4))),
            transform: Transform {
                translation: Vec3::new((a.x + b.x) / 2., (a.y + b.y) / 2., 0.),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            },
            ..default()
        });
    }

    // UI hint: instructions
    commands.spawn(TextBundle::from_section(
        "Click to place tower. G: toggle grid.",
        TextStyle {
            font_size: 16.0,
            color: Color::WHITE,
            ..default()
        },
    ).with_style(Style {
        position_type: PositionType::Absolute,
        left: Val::Px(8.),
        top: Val::Px(8.),
        ..default()
    }));
}

const GRID_LINE_THICKNESS: f32 = 2.0;

fn spawn_grid_lines(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<ColorMaterial>>,
) {
    let cw = cell_width();
    let ch = cell_height();
    let blue = mats.add(ColorMaterial::from(Color::rgb(0.2, 0.4, 0.8)));
    let z = -1.0;

    // Vertical lines: GRID_WIDTH + 1
    let v_line = meshes.add(Mesh::from(shape::Quad::new(Vec2::new(GRID_LINE_THICKNESS, WORLD_HEIGHT))));
    for c in 0..=GRID_WIDTH {
        let x = -WORLD_WIDTH / 2.0 + c as f32 * cw;
        commands.spawn((
            MaterialMesh2dBundle {
                mesh: v_line.clone().into(),
                material: blue.clone(),
                transform: Transform::from_translation(Vec3::new(x, 0., z)),
                ..default()
            },
            GridLine,
        ));
    }

    // Horizontal lines: GRID_HEIGHT + 1
    let h_line = meshes.add(Mesh::from(shape::Quad::new(Vec2::new(WORLD_WIDTH, GRID_LINE_THICKNESS))));
    for r in 0..=GRID_HEIGHT {
        let y = -WORLD_HEIGHT / 2.0 + r as f32 * ch;
        commands.spawn((
            MaterialMesh2dBundle {
                mesh: h_line.clone().into(),
                material: blue.clone(),
                transform: Transform::from_translation(Vec3::new(0., y, z)),
                ..default()
            },
            GridLine,
        ));
    }
}

fn toggle_grid_visibility(
    mut show_grid: ResMut<ShowGrid>,
    mut query: Query<&mut Visibility, With<GridLine>>,
    keys: Res<Input<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::G) {
        show_grid.0 = !show_grid.0;
        let vis = if show_grid.0 { Visibility::Visible } else { Visibility::Hidden };
        for mut v in query.iter_mut() {
            *v = vis;
        }
    }
}

struct SpawnTimer(Timer);

impl Default for SpawnTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.0, TimerMode::Repeating))
    }
}

fn spawn_enemy_periodic(mut commands: Commands, time: Res<Time>, mut timer: Local<SpawnTimer>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<ColorMaterial>>, path: Res<Path>) {
    if timer.0.tick(time.delta()).finished() {
        // spawn enemy at first path point
        let start = path.world_points[0];
        commands.spawn((
            MaterialMesh2dBundle {
                mesh: meshes.add(shape::Circle::new(12.).into()).into(),
                material: mats.add(ColorMaterial::from(Color::rgb(0.8, 0.2, 0.2))),
                transform: Transform::from_translation(Vec3::new(start.x, start.y, 1.)),
                ..default()
            },
            Enemy { hp: 10.0, path_idx: 0 },
        ));
    }
}

// Enemies follow the path points sequentially
fn enemy_follow_path(mut query: Query<(Entity, &mut Transform, &mut Enemy)>, time: Res<Time>, path: Res<Path>, mut commands: Commands) {
    for (entity, mut transform, mut enemy) in query.iter_mut() {
        if enemy.path_idx + 1 >= path.world_points.len() {
            // reached end — despawn
            commands.entity(entity).despawn_recursive();
            continue;
        }
        let current = Vec2::new(transform.translation.x, transform.translation.y);
        let target = path.world_points[enemy.path_idx + 1];
        let dir = (target - current).normalize_or_zero();
        let move_delta = dir * ENEMY_SPEED * time.delta_seconds();
        let new_pos = current + move_delta;
        transform.translation.x = new_pos.x;
        transform.translation.y = new_pos.y;

        // if close to target, advance index
        if new_pos.distance(target) < 6.0 {
            enemy.path_idx += 1;
        }
    }
}

// Mouse click places a tower at the grid cell under the cursor (path cells and occupied cells blocked).
fn place_tower_on_click(
    mut commands: Commands,
    buttons: Res<Input<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<&Transform, With<Camera>>,
    path: Res<Path>,
    towers: Query<&TowerCell>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<ColorMaterial>>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(wnd) = q_window.get_single() else { return };
    let Some(screen_pos) = wnd.cursor_position() else { return };
    let cam_transform = q_camera.single();
    let window_size = Vec2::new(wnd.width(), wnd.height());
    let ndc = (screen_pos / window_size) * 2.0 - Vec2::ONE;
    // Cursor Y is top-down (0 at top); flip so screen top = high world Y (Bevy Y-up)
    let world_pos = Vec2::new(
        cam_transform.translation.x + ndc.x * (window_size.x / 2.0),
        cam_transform.translation.y - ndc.y * (window_size.y / 2.0),
    );

    let (col, row) = world_to_cell(world_pos);
    if path.cells.contains(&(col, row)) {
        return; // no towers on path
    }
    if towers.iter().any(|tc| tc.0 == col && tc.1 == row) {
        return; // cell already occupied
    }

    let center = cell_center(col, row);
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(14.).into()).into(),
            material: mats.add(ColorMaterial::from(Color::rgb(0.2, 0.6, 0.9))),
            transform: Transform::from_translation(Vec3::new(center.x, center.y, 2.)),
            ..default()
        },
        Tower { timer: Timer::from_seconds(TOWER_FIRE_RATE, TimerMode::Repeating) },
        TowerCell(col, row),
    ));
}

// Towers find nearest enemy in range and spawn bullets
fn tower_shooting(
    mut commands: Commands,
    time: Res<Time>,
    mut towers: Query<(Entity, &Transform, &mut Tower)>,
    enemies: Query<(Entity, &Transform, &Enemy)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<ColorMaterial>>,
) {
    for (_tower_entity, tower_transform, mut tower) in towers.iter_mut() {
        tower.timer.tick(time.delta());
        if !tower.timer.finished() {
            continue;
        }
        let tower_pos = tower_transform.translation.truncate();
        // find nearest enemy within range
        let mut nearest: Option<(Entity, f32, Vec2)> = None;
        for (e, t_transform, _e_data) in enemies.iter() {
            let pos = t_transform.translation.truncate();
            let dist = pos.distance(tower_pos);
            if dist <= TOWER_RANGE {
                if nearest.is_none() || dist < nearest.as_ref().unwrap().1 {
                    nearest = Some((e, dist, pos));
                }
            }
        }
        if let Some((target_entity, _dist, target_pos)) = nearest {
            let dir = (target_pos - tower_pos).normalize_or_zero();
            // spawn bullet
            let mut bullet = commands.spawn((
                MaterialMesh2dBundle {
                    mesh: meshes.add(shape::Circle::new(4.).into()).into(),
                    material: mats.add(ColorMaterial::from(Color::rgb(0.95, 0.95, 0.45))),
                    transform: Transform::from_translation(Vec3::new(tower_pos.x, tower_pos.y, 3.)),
                    ..default()
                },
                Bullet { target: target_entity, damage: 5.0 },
            ));
            // store direction as rotation for move_bullets (must keep translation so bullet starts at tower)
            let angle = dir.y.atan2(dir.x);
            bullet.insert(Transform {
                translation: Vec3::new(tower_pos.x, tower_pos.y, 3.),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            });
        }
    }
}

fn move_bullets(mut bullets: Query<(&mut Transform, &Bullet)>, time: Res<Time>) {
    for (mut transform, _bullet) in bullets.iter_mut() {
        // read rotation as direction
        let angle = transform.rotation.to_euler(EulerRot::XYZ).2;
        let dir = Vec2::new(angle.cos(), angle.sin());
        transform.translation.x += dir.x * BULLET_SPEED * time.delta_seconds();
        transform.translation.y += dir.y * BULLET_SPEED * time.delta_seconds();
    }
}

// Bullet hit detection: if bullet is near its target entity, deal damage and despawn bullet
fn bullet_hit_detection(
    mut commands: Commands,
    mut bullets: Query<(Entity, &Transform, &Bullet)>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform)>,
) {
    for (b_ent, b_tf, bullet) in bullets.iter_mut() {
        // check if target exists
        if let Ok((e_ent, mut enemy, e_tf)) = enemies.get_mut(bullet.target) {
            let bpos = b_tf.translation.truncate();
            let epos = e_tf.translation.truncate();
            if bpos.distance(epos) < 12.0 {
                enemy.hp -= bullet.damage;
                commands.entity(b_ent).despawn_recursive();
                if enemy.hp <= 0.0 {
                    commands.entity(e_ent).despawn_recursive();
                }
            }
        } else {
            // target gone; despawn bullet
            commands.entity(b_ent).despawn_recursive();
        }
    }
}