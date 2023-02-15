use bevy::{
    prelude::*,
    render::{render_resource::WgpuFeatures, settings::WgpuSettings},
};


const PLAYER_SPEED: f32 = 0.05;
const ENEMY_SPEED: f32 = 0.01;
const PROJECTILE_SPEED: f32 = 0.05;
const HIT_THRESHOLD: f32 = 0.1;
const CAMERA_SPEED: f32 = 0.009;

fn main() {
    // enable wireframe rendering
    let mut wgpu_settings = WgpuSettings::default();
    wgpu_settings.features |= WgpuFeatures::POLYGON_MODE_LINE;

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(wgpu_settings)
        .init_resource::<Game>()
        .insert_resource(EnemySpawnTimer(Timer::from_seconds(
            3.,
            TimerMode::Repeating,
        )))
        .add_startup_system(setup_camera)
        .add_startup_system(setup_models)
        .add_startup_system(setup_lights)
        .add_system(player_movement)
        .add_system(spawn_enemy)
        .add_system(enemy_movement)
        .add_system(weapon_movement)
        .add_system(camera_movement)
        .add_system(projectile_movement)
        .add_system(projectile_hit)
        .add_system(weapon_fire)
        .add_system(player_aim)
        .run();
}

#[derive(Resource)]
pub struct Game {
    player: Entity,
    spud_gun: Entity,
    camera: Entity,
    enemies: Vec<Handle<Scene>>,
    aiming_at: Option<Entity>,
    is_aiming: bool,
    projectile: Option<Handle<Scene>>,
    environment: Entity,
}

#[derive(Component)]
pub struct Enemy;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Weapon;

#[derive(Resource)]
struct EnemySpawnTimer(Timer);

#[derive(Component)]
struct Projectile {
    heading: Vec3
}

impl Default for Game {
    fn default() -> Self {
        Self {
            player: Entity::from_bits(0),
            spud_gun: Entity::from_bits(1),
            environment: Entity::from_bits(2),
            camera: Entity::from_bits(3),
            enemies: Vec::new(),
            aiming_at: None,
            is_aiming: false,
            projectile: None,
        }
    }
}

fn setup_camera(mut commands: Commands, mut game: ResMut<Game>) {
    game.camera = commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 2.5, 2.0).looking_at(Vec3::NEG_Z * 2., Vec3::Y),
        ..default()
    }).id();
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

    game.environment = commands
        .spawn(SceneBundle {
            scene: asset_server.load("environment.glb#Scene0"),
            transform: Transform {
                ..default()
            },
            ..default()
        }).id();


    game.player = commands
        .spawn(SceneBundle {
            scene: asset_server.load("carrot.glb#Scene0"),
            ..default()
        })
        .add_child(game.spud_gun)
        .id();
    commands.entity(game.player).insert(Player);

    game.projectile = Some(asset_server.load("pumpkinBasic.glb#Scene0"));

    game.enemies = vec![asset_server.load("beet.glb#Scene0")];
}

fn setup_lights(mut commands: Commands) {
    commands.spawn(DirectionalLightBundle { 
        directional_light: DirectionalLight {
            color: Color::Rgba { red: 0.5, green: 0., blue: 0., alpha: 1.},
            shadows_enabled: true,
            illuminance: 15_000.,
            ..default()
        },
        transform: Transform {
            rotation: Quat::from_euler(EulerRot::XYZ, -0.8, -0.3, 0.),
            ..default()
        },
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
        movement.x = left_stick_x * PLAYER_SPEED;
    }

    let left_stick_y = axes
        .get(GamepadAxis::new(gamepad, GamepadAxisType::LeftStickY))
        .unwrap();
    
    if left_stick_y.abs() > 0.01 {
        movement.y = left_stick_y * PLAYER_SPEED;
    }

    player_translation.x += movement.x;
    player_translation.z -= movement.y;
}

fn projectile_movement(
    mut projectiles: Query<(&mut Transform, &Projectile)>
) {
    for (mut transform, projectile) in projectiles.iter_mut() {
        transform.translation += projectile.heading * PROJECTILE_SPEED;
        transform.rotate_x(PROJECTILE_SPEED);
    }
}

fn camera_movement(mut transforms: Query<&mut Transform>, game: Res<Game>) {
    transforms.get_mut(game.camera).unwrap().translation.z -= CAMERA_SPEED;
}


fn projectile_hit(
    mut game: ResMut<Game>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
    projectiles: Query<(Entity, &Transform), (With<Projectile>, Without<Enemy>)>,
    mut commands: Commands,
) {
    for (projectile_entity, projectile_transform) in projectiles.iter() {
        for (enemy_entity, enemy_transform) in enemies.iter() {
            let distance = (projectile_transform.translation - enemy_transform.translation).length().abs();
            if distance <= HIT_THRESHOLD {
                // It's a hit!
                if game.aiming_at == Some(enemy_entity) { game.aiming_at = None};
                commands.entity(projectile_entity).despawn_recursive();
                commands.entity(enemy_entity).despawn_recursive();
            }
        }
    }
}


fn spawn_enemy(
    game: Res<Game>,
    mut timer: ResMut<EnemySpawnTimer>,
    time: Res<Time>,
    mut commands: Commands,
    transforms: Query<&Transform>,
) {
    if !timer.0.tick(time.delta()).finished() {
        return;
    };

    // Pick the kind of enemy to spawn
    let enemy_kind = game.enemies[0].clone();
    let x_position = (rand::random::<f32>() * 4.0) - 2.0;
    let camera_z = transforms.get(game.camera).unwrap().translation.z;

    let enemy = commands
        .spawn(SceneBundle {
            scene: enemy_kind,
            transform: Transform {
                translation: [x_position, 0., camera_z -10.].into(),
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

fn weapon_fire(
    gamepads: Res<Gamepads>,
    gamepad_button: Res<Input<GamepadButton>>,
    mut commands: Commands,
    game: Res<Game>,
    transforms: Query<&GlobalTransform>,
) {
    let Some(projectile_asset) = &game.projectile else { return };
    let Some(gamepad) = gamepads.iter().next() else { return};
    let pressed = gamepad_button.just_pressed(GamepadButton::new(
        gamepad,
        GamepadButtonType::RightTrigger2,
    ));

    if !pressed {
        return;
    }

    let Some(enemy) = game.aiming_at else { return };
    let origin = transforms.get(game.spud_gun).unwrap().translation();
    let target = transforms.get(enemy).unwrap().translation();
    let heading = (target - origin).normalize();

    commands
        .spawn(SceneBundle {
            scene: projectile_asset.clone(),
            transform: Transform {
                translation: origin,
                ..default()
            },
            ..default()
        })
        .insert(Projectile { heading });

}

enum AimDirection {
    Left,
    Right
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



    // We only want to change the aim once the stick has left the dead zone
    if right_stick_x.abs() < 0.1 {
        game.is_aiming = false;
        return;
    }

    // But if we've already left the dead zone, we want to wait until the stick is back
    if game.is_aiming { return };

    // Okay, now we're aiming
    game.is_aiming = true;

    let aim_direction = if right_stick_x > 0.0 {
        AimDirection::Right
    } else { AimDirection::Left };

    // First, get a list of enemies in order from left to right
    let mut ordered_enemy_list = enemy_transforms.iter().collect::<Vec<_>>();
    if ordered_enemy_list.is_empty() {
        return;
    };

    ordered_enemy_list
        .sort_by(|(_, t_a), (_, t_b)| (t_a.translation.x).partial_cmp(&t_b.translation.x).unwrap());

    // If the player isn't currently aiming at an enemy, then take the first one from the left
    let Some(enemy) = game.aiming_at else { 
        let enemy = match aim_direction {
            AimDirection::Left => ordered_enemy_list.first().unwrap().0,
            AimDirection::Right => ordered_enemy_list.last().unwrap().0
        };
        game.aiming_at = Some(enemy);
        return 
    };
    
    // If the player *is* currently aiming at an enemy, find its index in the sort order
    let Some(index) = ordered_enemy_list.iter().position(|(entity, _)| *entity == enemy) else {
        println!("Player is aiming at an entity that does not exist");
        game.aiming_at = None;
        return;
    };

    // If the player is aiming in a direction, and the enemy is already the one that is most in that direction, do nothing
    match aim_direction {
        AimDirection::Left => if index == 0 { return },
        AimDirection::Right => if index == ordered_enemy_list.len()- 1 { return},
    };

    // Otherwise, aim at the next enemy along in the direction the player is aiming
    let index_increment: i32 = match aim_direction {
        AimDirection::Left => -1,
        AimDirection::Right => 1
    };

    let next_enemy_index = (index as i32 + index_increment) as usize % (ordered_enemy_list.len());
    game.aiming_at = Some(ordered_enemy_list[next_enemy_index].0);
}

// This is buggy. I need to remember how to do trigonometry again.
fn weapon_movement(
    game: Res<Game>,
    mut transforms: Query<&mut Transform>
) {
    // If we're aiming at an enemy, that's the target - otherwise just aim straight ahead
    let target = if let Some(enemy) = game.aiming_at { 
        transforms.get(enemy).unwrap().translation
    } else {
        Vec3::NEG_Z
    };

    transforms.get_mut(game.spud_gun).unwrap().look_at(target, Vec3::Y);
}