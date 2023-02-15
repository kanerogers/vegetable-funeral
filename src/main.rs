use bevy::{
    prelude::*,
    render::{render_resource::WgpuFeatures, settings::WgpuSettings},
};

use bevy_editor_pls::prelude::*;

const INPUT_SPEED: f32 = 0.1;
const ENEMY_SPEED: f32 = 0.01;

fn main() {
    // enable wireframe rendering
    let mut wgpu_settings = WgpuSettings::default();
    wgpu_settings.features |= WgpuFeatures::POLYGON_MODE_LINE;

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EditorPlugin)
        .insert_resource(wgpu_settings)
        .init_resource::<Game>()
        .insert_resource(EnemySpawnTimer(Timer::from_seconds(
            1.,
            TimerMode::Repeating,
        )))
        .add_startup_system(setup_camera)
        .add_startup_system(setup_models)
        .add_startup_system(setup_lights)
        .add_system(player_movement)
        .add_system(spawn_enemy)
        .add_system(enemy_movement)
        .add_system(weapon_movement)
        .add_system(kill_enemy)
        .add_system(player_aim)
        .run();
}

#[derive(Resource)]
pub struct Game {
    player: Entity,
    spud_gun: Entity,
    enemies: Vec<Handle<Scene>>,
    aiming_at: Option<Entity>,
}

#[derive(Component)]
pub struct Enemy;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Weapon;

#[derive(Resource)]
struct EnemySpawnTimer(Timer);

impl Default for Game {
    fn default() -> Self {
        Self {
            player: Entity::from_bits(0),
            spud_gun: Entity::from_bits(1),
            enemies: Vec::new(),
            aiming_at: None,
        }
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 2.5, 2.0).looking_at(Vec3::NEG_Z * 2., Vec3::Y),
        ..default()
    });
}

fn setup_models(mut commands: Commands, asset_server: Res<AssetServer>, mut game: ResMut<Game>) {
    game.spud_gun = commands
        .spawn(SceneBundle {
            scene: asset_server.load("launcher.glb#Scene0"),
            transform: Transform {
                translation: [0.07, 0.25, 0.].into(),
                ..default()
            },
            ..default()
        })
        .id();
    commands.entity(game.player).insert(Weapon);


    game.player = commands
        .spawn(SceneBundle {
            scene: asset_server.load("carrot.glb#Scene0"),
            ..default()
        })
        .add_child(game.spud_gun)
        .id();
    commands.entity(game.player).insert(Player);

    game.enemies = vec![asset_server.load("beet.glb#Scene0")];
}

fn setup_lights(mut commands: Commands) {
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });
}

fn player_movement(
    game: ResMut<Game>,
    axes: Res<Axis<GamepadAxis>>,
    gamepads: Res<Gamepads>,
    mut transforms: Query<&mut Transform, With<Player>>,
) {
    let Some(gamepad) = gamepads.iter().next() else { return} ;
    let player_translation = &mut transforms.get_mut(game.player).unwrap().translation;
    let mut movement = Vec2::ZERO;
    let left_stick_x = axes
        .get(GamepadAxis::new(gamepad, GamepadAxisType::LeftStickX))
        .unwrap();

    if left_stick_x.abs() > 0.01 {
        movement.x = left_stick_x * INPUT_SPEED;
        println!("Player movement: {movement:?}");
    }

    player_translation.x += movement.x;
    player_translation.y += movement.y;
}

fn spawn_enemy(
    game: Res<Game>,
    mut timer: ResMut<EnemySpawnTimer>,
    time: Res<Time>,
    mut commands: Commands,
) {
    if !timer.0.tick(time.delta()).finished() {
        return;
    };

    // Pick the kind of enemy to spawn
    let enemy_kind = game.enemies[0].clone();
    let x_position = (rand::random::<f32>() * 4.0) - 2.0;

    let enemy = commands
        .spawn(SceneBundle {
            scene: enemy_kind,
            transform: Transform {
                translation: [x_position, 0., -2.].into(),
                ..default()
            },
            ..default()
        })
        .id();

    commands.entity(enemy).insert(Enemy);
}

fn enemy_movement(
    mut enemy_transforms: Query<&mut Transform, With<Enemy>>,
    game: Res<Game>,
    player_transform: Query<&Transform, (Without<Enemy>, With<Player>)>,
) {
    let player_position = player_transform.get(game.player).unwrap().translation;
    for mut transform in enemy_transforms.iter_mut() {
        let enemy_position = &mut transform.translation;
        let to_player = (player_position - *enemy_position).normalize() * ENEMY_SPEED;
        *enemy_position += to_player;
    }
}

fn kill_enemy(
    gamepads: Res<Gamepads>,
    gamepad_button: Res<Input<GamepadButton>>,
    mut commands: Commands,
    mut game: ResMut<Game>,
) {
    let Some(gamepad) = gamepads.iter().next() else { return};
    let pressed = gamepad_button.just_pressed(GamepadButton::new(
        gamepad,
        GamepadButtonType::RightTrigger2,
    ));

    if !pressed {
        return;
    }

    let Some(enemy) = game.aiming_at else { return };

    commands.entity(enemy).despawn_recursive();
    game.aiming_at = None;
}

fn player_aim(
    gamepads: Res<Gamepads>,
    axes: Res<Axis<GamepadAxis>>,
    enemy_transforms: Query<(Entity, &Transform), With<Enemy>>,
    mut game: ResMut<Game>,
) {
    let Some(gamepad) = gamepads.iter().next() else { return} ;
    let right_stick_x = axes
        .get(GamepadAxis::new(gamepad, GamepadAxisType::RightStickX))
        .unwrap();

    if right_stick_x.abs() < 0.1 {
        return;
    }

    let index_increment: i32 = if right_stick_x > 0.0 {
        1
    } else { -1 };

    // First, get a list of enemies in order from left to right
    let mut ordered_enemy_list = enemy_transforms.iter().collect::<Vec<_>>();
    if ordered_enemy_list.is_empty() {
        return;
    };

    ordered_enemy_list
        .sort_by(|(_, t_a), (_, t_b)| (t_a.translation.x).partial_cmp(&t_b.translation.x).unwrap());

    // If the player isn't currently aiming at an enemy, then take the first one from the left
    let Some(enemy) = game.aiming_at else { 
        game.aiming_at = Some(ordered_enemy_list.first().unwrap().0);
        return 
    };
    
    // If the player *is* currently aiming at an enemy, find its index in the sort order
    let Some(index) = ordered_enemy_list.iter().position(|(entity, _)| *entity == enemy) else {
        println!("Player is aiming at an entity that does not exist");
        game.aiming_at = None;
        return;
    };

    let next_enemy_index = (index as i32 + index_increment) as usize % (ordered_enemy_list.len());
    game.aiming_at = Some(ordered_enemy_list[next_enemy_index].0);
}

fn weapon_movement(
    game: Res<Game>,
    mut weapon_transform: Query<&mut Transform, (With<Weapon>, Without<Enemy>)>,
    enemy_transforms: Query<&Transform, With<Enemy>>
) {
    // If we're aiming at an enemy, that's the target - otherwise just aim straight ahead
    let target = if let Some(enemy) = game.aiming_at { 
        enemy_transforms.get(enemy).unwrap().translation
    } else {
        Vec3::NEG_Z
    };

    weapon_transform.get_mut(game.spud_gun).unwrap().look_at(target, Vec3::Y);
}
