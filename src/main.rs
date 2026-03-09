use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::window::{PrimaryWindow, Window, WindowPlugin};

const WINDOW_WIDTH: f32 = 900.;
const WINDOW_HEIGHT: f32 = 600.;

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
struct Bullet {
    target: Entity,
    damage: f32,
}

#[derive(Resource)]
struct Path {
    points: Vec<Vec2>,
}

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
        .insert_resource(Path {
            points: vec![
                Vec2::new(-400., -200.),
                Vec2::new(-200., 0.),
                Vec2::new(0., 80.),
                Vec2::new(200., 0.),
                Vec2::new(400., 200.),
            ],
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (
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
    for p in &path.points {
        commands.spawn(MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(8.).into()).into(),
            material: mats.add(ColorMaterial::from(Color::rgb(0.9, 0.8, 0.4))),
            transform: Transform::from_translation(Vec3::new(p.x, p.y, 0.)),
            ..default()
        });
    }
    for window in path.points.windows(2) {
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
        "Click to place tower. Towers auto-shoot nearest enemy.",
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

struct SpawnTimer(Timer);

impl Default for SpawnTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.0, TimerMode::Repeating))
    }
}

fn spawn_enemy_periodic(mut commands: Commands, time: Res<Time>, mut timer: Local<SpawnTimer>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<ColorMaterial>>, path: Res<Path>) {
    if timer.0.tick(time.delta()).finished() {
        // spawn enemy at first path point
        let start = path.points[0];
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
        if enemy.path_idx + 1 >= path.points.len() {
            // reached end — despawn
            commands.entity(entity).despawn_recursive();
            continue;
        }
        let current = Vec2::new(transform.translation.x, transform.translation.y);
        let target = path.points[enemy.path_idx + 1];
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

// Mouse click places a tower at cursor position
fn place_tower_on_click(
    mut commands: Commands,
    buttons: Res<Input<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<&Transform, With<Camera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<ColorMaterial>>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(wnd) = q_window.get_single() else { return };
    if let Some(screen_pos) = wnd.cursor_position() {
        let cam_transform = q_camera.single();
        // Convert screen -> world
        let window_size = Vec2::new(wnd.width(), wnd.height());
        let ndc = (screen_pos / window_size) * 2.0 - Vec2::ONE;
        let world_pos = cam_transform.translation.truncate() + ndc * Vec2::new(window_size.x / 2.0, window_size.y / 2.0);
        commands.spawn((
            MaterialMesh2dBundle {
                mesh: meshes.add(shape::Circle::new(14.).into()).into(),
                material: mats.add(ColorMaterial::from(Color::rgb(0.2, 0.6, 0.9))),
                transform: Transform::from_translation(Vec3::new(world_pos.x, world_pos.y, 2.)),
                ..default()
            },
            Tower { timer: Timer::from_seconds(TOWER_FIRE_RATE, TimerMode::Repeating) },
        ));
    }
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