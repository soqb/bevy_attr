//! Automatic management of highly modular, highly parallel values with minimal overhead.
//!
//! Good examples of things to use this crate for are in-game stats, and
//! systems like health management (both maximum health and health could be implemented as [attributes][Attribute]).
//!
//! See the [`Attribute`] trait for a more detailed overview, or view [the examples].
//!
//! [the examples]: https://github.com/istanbul-not-constantinople/bevy_attr/tree/main/examples

use std::{cmp::Ordering, marker::PhantomData};

use bevy::prelude::*;
use bevy_trait_query::RegisterExt;

/// Resets a variable to its default value.
///
/// Implemented for all [`T: Default`][Default],
/// but you can implement the trait manually for control over keeping certain data between resets.
///
/// Required for [`Attribute`].
///
/// # Examples
/// ```rust
/// use bevy_attr::Reset;
///
/// struct Accumulator {
///     value: usize,
///     next: fn(usize) -> usize,   
/// }
///
/// impl Reset for Accumulator {
///     fn reset(&mut self) {
///         self.value = 0;
///         // we don't want to reset the next fn here though!
///     }
/// }
///
/// ```
pub trait Reset {
    /// Resets a variable to its default value.
    fn reset(&mut self);
}

impl<T: Default> Reset for T {
    fn reset(&mut self) {
        *self = Default::default();
    }
}

/// Marker trait for components which act as attributes.
///
/// An attribute has a base value (defined by the [`Reset`] trait)
/// and a number of [modifiers][Modifier] which are attached to the same entity.
///
/// Attribute values are regenerated when one of their modifiers is added, mutated or removed.
/// When a modifier is changed, the attribute is marked with [`DirtyAttr`] and a system runs,
/// first [resetting][Reset] the value of the attribute,
/// and then applying each modifier attached to the same entity,
/// sorted by their [priority][Modifier::priority].
///
/// All attributes should be registered by adding [`AttributePlugin`]s to your app.
///
/// See the [`Modifier`] trait for more information on modifiers.
///
/// # Examples
/// ```rust
/// use bevy::prelude::*;
/// use bevy_attr::{AttributePlugin, ModifierPlugin, Attribute, Modifier};
///
/// // define our attribute component.
/// #[derive(Component, Deref, DerefMut)]
/// struct MaxHealth(usize);
///
/// // implementing default implements the `Reset` trait
/// // which is required for `Attribute`.
/// impl Default for MaxHealth {
///     fn default() -> Self {
///         Self(100)
///     }
/// }
///
/// // implement the marker trait.
/// impl Attribute for MaxHealth {}
///
///
/// // define out modifier.
/// #[derive(Component)]
/// struct ExtraMaxHealth;
///
/// impl Modifier for ExtraMaxHealth {
///     type Attr = MaxHealth;
///
///     fn apply(&self, value: &mut MaxHealth) {
///         **value += 50;
///     }
/// }
///
/// let mut app = App::new();
///
/// // add the relevant plugins to our app.
/// app.add_plugin(AttributePlugin::<MaxHealth>::default());
/// app.add_plugin(ModifierPlugin::<ExtraMaxHealth>::default());
///
/// let id = app
///     .world
///     .spawn((MaxHealth::default(), ExtraMaxHealth))
///     .id();
///
/// app.update();
/// // during this update:
/// // 1. In `CoreStage::Update`, the `ModifierPlugin` notices that the `ExtraMaxHealth` modifier was added
/// //    to an entity with the `MaxHealth` attribute and gives the entity the `DirtyAttr<MaxHealth>` component.
/// // 2. In `CoreStage::PostUpdate`, the `AttributePlugin` notices that the `DirtyAttr` component was added
/// //    and recalculates the attribute. First it resets the attribute value to `MaxHealth(100)`,
/// //    and then it adds the health from the `ExtraMaxHealth` modifier (a total of 150).
/// //    The `DirtyAttr` component is then removed.
///
/// {
///     let mut entity = app.world.get_entity_mut(id).unwrap();
///     let max_health = entity.get::<MaxHealth>().unwrap();
///     assert_eq!(**max_health, 150);
///     entity.remove::<ExtraMaxHealth>();
/// }
///
/// app.update();
///
/// {
///     let max_health = app.world.get::<MaxHealth>(id).unwrap();
///     assert_eq!(**max_health, 100);
/// }
/// ```
pub trait Attribute: Component + Reset {}

/// A modifier on an [`Attribute`].
///
/// Modifiers should always be components.
///
/// All modifiers should be registered by adding a [`ModifierPlugin`] to your app.
///
/// See the [`Attribute`] trait for a more detailed overview.
#[bevy_trait_query::queryable]
pub trait Modifier: Send + Sync + 'static {
    /// The attribute that this modifier modifies.
    type Attr: Attribute;

    /// Returns the signed priority of the modifier.
    ///
    /// This method should be implemented for most modifiers
    /// since most sequences of operations are order-dependent.
    ///
    /// By default the priority is zero.
    fn priority(&self) -> isize {
        0
    }

    /// Applies the modifier to an instance of its associated attribute.
    fn apply(&self, attr: &mut Self::Attr);
}

trait ModifierExt: Modifier {
    #[cfg(debug_assertions)]
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
impl<M: Modifier + ?Sized + 'static> ModifierExt for M {}

/// Registers the required information for an [`Attribute`].
///
/// The relevant [`ModifierPlugin`]s should also be added to your app.
#[derive(Default)]
pub struct AttributePlugin<A: Attribute>(PhantomData<A>);

fn refresh_dirty_attr<A: Attribute>(
    mut attrs: Query<(Entity, &mut A, Option<&dyn Modifier<Attr = A>>), With<DirtyAttr<A>>>,
    mut commands: Commands,
) {
    for (dirty, mut attr, mods) in attrs.iter_mut() {
        let mut mods: Vec<_> = mods.map_or_else(Vec::new, |mods| mods.iter().collect());
        mods.sort_unstable_by(|a, b| {
            let order = a.priority().cmp(&b.priority());
            #[cfg(debug_assertions)]
            if let Ordering::Equal = order {
                warn!(
                    "ambiguity between the order of two modifiers ({} and {} have the same priority)",
                    a.type_name(),
                    b.type_name(),
                );
            }
            order
        });

        Reset::reset(&mut *attr);

        for modifier in mods.iter() {
            modifier.apply(&mut attr);
        }

        commands.get_entity(dirty).unwrap().remove::<DirtyAttr<A>>();
    }
}

impl<A: Attribute> Plugin for AttributePlugin<A> {
    fn build(&self, app: &mut App) {
        /// necessary without [bevy-trait-query#11]
        ///
        /// [bevy-trait-query#11]: https://github.com/JoJoJet/bevy-trait-query/pull/11
        #[derive(Component)]
        struct TraitQueryWorkaround<B: Attribute>(PhantomData<B>);
        impl<B: Attribute> Modifier for TraitQueryWorkaround<B> {
            type Attr = B;

            fn apply(&self, _: &mut B) {}
        }
        // app.register_component_as::<dyn ModifierQueryable<A>, TraitQueryWorkaround<A>>();
        app.add_system_to_stage(CoreStage::PostUpdate, refresh_dirty_attr::<A>);
    }
}

/// Registers the required information for a [`Modifier`].
///
/// The relevant [`AttributePlugin`] should also be added to your app.
pub struct ModifierPlugin<M: Modifier>(PhantomData<M>);

impl<M: Modifier> Default for ModifierPlugin<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

/// Marker component to indicates that an [`Attribute`]'s modifiers have changed since the last update.
#[derive(Component)]
pub struct DirtyAttr<A: Attribute>(PhantomData<A>);

impl<A: Attribute> Default for DirtyAttr<A> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

fn modifier_changed<M: Modifier + Component>(
    changed: Query<Entity, (Changed<M>, Without<DirtyAttr<M::Attr>>)>,
    mut commands: Commands,
) {
    for entity in &changed {
        let mut commands = commands.entity(entity);
        commands.insert(DirtyAttr::<M::Attr>::default());
    }
}

fn modifier_removed<M: Modifier + Component>(
    removed: RemovedComponents<M>,

    mut commands: Commands,
) {
    for entity in &removed {
        info!("{} removed from {:?}", std::any::type_name::<M>(), entity);
        let Some(mut commands) = commands.get_entity(entity) else {
            continue;
        };
        commands.insert(DirtyAttr::<M::Attr>::default());
    }
}

impl<M: Modifier + Component> Plugin for ModifierPlugin<M> {
    fn build(&self, app: &mut App) {
        app.add_system_set_to_stage(
            CoreStage::PostUpdate,
            SystemSet::new()
                .before(refresh_dirty_attr::<M::Attr>)
                .with_system(modifier_changed::<M>)
                .with_system(modifier_removed::<M>),
        );
        app.register_component_as::<dyn Modifier<Attr = M::Attr>, M>();
    }
}

#[cfg(test)]
mod tests {}
