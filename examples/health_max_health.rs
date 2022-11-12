use std::num::NonZeroUsize;

use bevy::{
    log::{Level, LogPlugin},
    prelude::*,
    time::FixedTimestep,
};

use bevy_attr::{Attribute, AttributePlugin, Modifier, ModifierPlugin, ModifierPriority};

#[derive(Component, Deref, DerefMut, Default)]
struct Health(usize);
impl Attribute for Health {}

#[derive(Component, Deref, DerefMut)]
struct MaxHealth(usize);
impl Attribute for MaxHealth {}

impl Default for MaxHealth {
    fn default() -> Self {
        MaxHealth(20)
    }
}

#[derive(Component, Default)]
struct ExtraMaxHealthCharm;

impl Modifier for ExtraMaxHealthCharm {
    type Attr = MaxHealth;

    fn apply(&self, max_health: &mut MaxHealth) {
        **max_health += 10;
    }

    const PRIORITY: ModifierPriority<Self::Attr> = ModifierPriority::ZERO;
}

// since `Health` and `MaxHealth` will both be inserted onto the same entity,
// we can make `MaxHealth` a modifier for `Health`
// instead of implementing redundant code on the `Reset` impl for `Health`.
impl Modifier for MaxHealth {
    type Attr = Health;

    fn apply(&self, health: &mut Health) {
        **health += **self
    }

    const PRIORITY: ModifierPriority<Self::Attr> = ModifierPriority::ZERO;
}

#[derive(Component, Deref, DerefMut)]
struct Damage(usize);

impl Modifier for Damage {
    type Attr = Health;

    fn apply(&self, health: &mut Health) {
        // set to zero if would be negative.
        **health = health.saturating_sub(**self);
    }

    const PRIORITY: ModifierPriority<Self::Attr> = MaxHealth::PRIORITY.after();
}

#[derive(Component, Deref, DerefMut)]
struct RegenRate(usize);

#[derive(Component)]
struct Actor {
    pub name: &'static str,
}

// take damage event.
struct Hit {
    actor: Entity,
    damage: usize,
}

fn take_damage(
    mut damaged: Query<(&Actor, &mut Damage)>,
    undamaged: Query<(Entity, &Actor), Without<Damage>>,
    mut hits: EventReader<Hit>,
    mut commands: Commands,
) {
    for hit in hits.iter() {
        if let Ok((actor, mut damage)) = damaged.get_mut(hit.actor) {
            info!("ouch! {} just took {} more damage!", actor.name, hit.damage);
            **damage += hit.damage;
        } else if let Ok((entity, actor)) = undamaged.get(hit.actor) {
            info!("ouch! {} just took {} damage!", actor.name, hit.damage);
            commands.entity(entity).insert(Damage(hit.damage));
        }
    }
}

fn regenerate(
    mut damaged: Query<(Entity, &Actor, &mut Damage, &RegenRate)>,
    mut commands: Commands,
) {
    for (entity, actor, mut damage, regen_rate) in damaged.iter_mut() {
        info!("phew! {} is healing {} damage!", actor.name, **regen_rate);
        if let Some(nonzero) = NonZeroUsize::new(damage.saturating_sub(**regen_rate)) {
            **damage = nonzero.into();
        } else {
            info!("phew! {} is fully healed!", actor.name);
            commands.entity(entity).remove::<Damage>();
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Actor { name: "Mike" },
        MaxHealth::default(),
        Health::default(),
        RegenRate(10),
    ));

    commands.spawn((
        Actor { name: "Paul" },
        MaxHealth::default(),
        ExtraMaxHealthCharm::default(),
        Health::default(),
        RegenRate(2),
    ));
}

fn hit_everyone(everyone: Query<(Entity, &Actor)>, mut hits: EventWriter<Hit>) {
    let batch = everyone.iter().map(|(entity, actor)| {
        const DAMAGE: usize = 5;
        info!("oops! hitting {} for {DAMAGE} damage!", actor.name);
        Hit {
            actor: entity,
            damage: DAMAGE,
        }
    });
    hits.send_batch(batch);
}

fn kill_dying(dying: Query<(Entity, &Actor, &Health)>, mut commands: Commands) {
    for (entity, actor, health) in dying.iter() {
        if **health == 0 {
            info!("ohno! {} has died!", actor.name);
            commands.entity(entity).despawn();
        }
    }
}

fn log_health(actors: Query<(&Actor, &Health, &MaxHealth)>) {
    for (actor, health, max_health) in actors.iter() {
        info!(
            "info! {} is at {}/{} health",
            actor.name, **health, **max_health
        );
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugin(LogPlugin {
        level: Level::TRACE,
        ..Default::default()
    });

    app.add_plugin(AttributePlugin::<MaxHealth>::default())
        .add_plugin(ModifierPlugin::<ExtraMaxHealthCharm>::default())
        .add_plugin(AttributePlugin::<Health>::default())
        .add_plugin(ModifierPlugin::<MaxHealth>::default())
        .add_plugin(ModifierPlugin::<Damage>::default());

    app.add_event::<Hit>();

    app.add_startup_system(setup);
    app.add_system(take_damage);
    app.add_system_to_stage(CoreStage::Last, kill_dying);
    app.add_system_set(
        SystemSet::new()
            .before(take_damage)
            .with_run_criteria(FixedTimestep::step(2.))
            .with_system(hit_everyone),
    );
    app.add_system_set(
        SystemSet::new()
            .before(take_damage)
            .with_run_criteria(FixedTimestep::step(1.5))
            .with_system(regenerate),
    );
    app.add_system_set(
        SystemSet::new()
            .before(kill_dying)
            .with_run_criteria(FixedTimestep::step(0.25))
            .with_system(log_health),
    );

    app.run();
}
