//! Automatic management of highly modular, highly parallel values with minimal overhead.
//!
//! Good examples of things to use this crate for are in-game stats, and
//! systems like health management (both maximum health and health could be implemented as [attributes][Attribute]).
//!
//! See the [`Attribute`] trait for a more detailed overview, or view [the examples].
//!
//! [the examples]: https://github.com/istanbul-not-constantinople/bevy_attr/tree/main/examples

use core::fmt;
use std::{cmp::Ordering, marker::PhantomData, any::TypeId};

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
/// use bevy::log::{Level, LogPlugin};
/// use bevy_attr::{AttributePlugin, ModifierPlugin, Attribute, Modifier, ModifierPriority};
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
///
///     const PRIORITY: ModifierPriority<MaxHealth> = ModifierPriority::ZERO;
/// }
///
/// let mut app = App::new();
/// app.add_plugins(MinimalPlugins).add_plugin(LogPlugin {
///     level: Level::TRACE,
///     ..Default::default()
/// });
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
///
///     assert_eq!(**max_health, 150);
///     entity.remove::<ExtraMaxHealth>();
/// }
///
/// app.update();
/// app.update();
///
/// {
///     let max_health = app.world.get::<MaxHealth>(id).unwrap();
///     assert_eq!(**max_health, 100);
/// }
/// ```
pub trait Attribute: Component + Reset {}

pub struct ModifierPriority<A: Attribute> {
    index: i32,
    _marker: PhantomData<A>,
}

impl<A: Attribute> fmt::Debug for ModifierPriority<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModifierPriority")
            .field("index", &self.index)
            .finish()
    }
}

impl<A: Attribute> ModifierPriority<A> {
    pub(self) const fn new(index: i32) -> Self {
        Self {
            index,
            _marker: PhantomData,
        }
    }

    pub const ZERO: Self = Self::new(0);

    pub const fn after(self) -> Self {
        Self::new(self.index + 1)
    }

    pub const fn before(self) -> Self {
        Self::new(self.index - 1)
    }

    pub(crate) fn cmp_with(self, b: ModifierPriority<A>, type_id_a: TypeId, type_id_b: TypeId) -> Ordering {
        match self.index.cmp(&b.index) {
            Ordering::Equal => {},
            ord => return ord,
        }

        type_id_a.cmp(&type_id_b)
    }
}

pub trait HasModifierPriority<A: Attribute> {
    const PRIORITY: ModifierPriority<A>;
}

#[bevy_trait_query::queryable]
pub trait ModifierGeneric<A: Attribute>: Send + Sync + 'static {
    /// Returns the signed priority of the modifier.
    ///
    /// This method should be implemented for most modifiers
    /// since most sequences of operations are order-dependent.
    ///
    /// By default the priority is zero.
    fn priority(&self) -> ModifierPriority<A>;

    /// Applies the modifier to an instance of its associated attribute.
    fn apply(&self, attr: &mut A);
}

/// A modifier on an [`Attribute`].
///
/// Modifiers should always be components.
///
/// All modifiers should be registered by adding a [`ModifierPlugin`] to your app.
///
/// See the [`Attribute`] trait for a more detailed overview.
pub trait Modifier: Send + Sync + 'static {
    /// The attribute that this modifier modifies.
    type Attr: Attribute;

    /// Returns the signed priority of the modifier.
    ///
    /// This method should be implemented for most modifiers
    /// since most sequences of operations are order-dependent.
    ///
    /// By default the priority is zero.
    const PRIORITY: ModifierPriority<Self::Attr>;

    /// Applies the modifier to an instance of its associated attribute.
    fn apply(&self, attr: &mut Self::Attr);
}

impl<M: Modifier> ModifierGeneric<M::Attr> for M {
    fn priority(&self) -> ModifierPriority<M::Attr>  {
        M::PRIORITY
    }

    fn apply(&self, attr: &mut M::Attr) {
        <M as Modifier>::apply(self, attr)
    }
}

trait ModifierExt<A: Attribute>: ModifierGeneric<A> {
    #[cfg(debug_assertions)]
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}
impl<M: ModifierGeneric<A> + ?Sized + 'static, A: Attribute> ModifierExt<A> for M {}

/// Registers the required information for an [`Attribute`].
///
/// The relevant [`ModifierPlugin`]s should also be added to your app.
#[derive(Default)]
pub struct AttributePlugin<A: Attribute>(PhantomData<A>);

fn refresh_dirty_attr<A: Attribute>(
    mut attrs: Query<(Entity, &mut A, Option<&dyn ModifierGeneric<A>>), With<DirtyAttr<A>>>,
    mut commands: Commands,
) {
    for (dirty, mut attr, mods) in attrs.iter_mut() {
        debug!("some modifiers have changed!");
        let mut mods: Vec<_> = mods.map_or_else(Vec::new, |mods| mods.iter().collect());
        mods.sort_unstable_by(|a, b| {
            let order = a.priority().cmp_with(b.priority(), a.type_id(), b.type_id());
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
        app.add_system_to_stage(CoreStage::PostUpdate, refresh_dirty_attr::<A>);
    }
}

/// Registers the required information for a [`Modifier`].
///
/// The relevant [`AttributePlugin`] should also be added to your app.
pub struct ModifierGenericPlugin<M: ModifierGeneric<A>, A: Attribute>(PhantomData<(M, A)>);

impl<M: ModifierGeneric<A>, A: Attribute> Default for ModifierGenericPlugin<M, A> {
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

fn modifier_changed<M: ModifierGeneric<A> + Component, A: Attribute>(
    changed: Query<Entity, (Changed<M>, Without<DirtyAttr<A>>)>,
    mut commands: Commands,
) {
    for entity in &changed {
        #[cfg(debug_assertions)]
        trace!(
            "modifier {} changed on {:?}",
            std::any::type_name::<M>(),
            entity
        );
        let mut commands = commands.entity(entity);
        commands.insert(DirtyAttr::<A>::default());
    }
}

fn modifier_removed<M: ModifierGeneric<A> + Component, A: Attribute>(
    removed: RemovedComponents<M>,
    mut commands: Commands,
) {
    for entity in &removed {
        #[cfg(debug_assertions)]
        trace!(
            "modifier {} removed from {:?}",
            std::any::type_name::<M>(),
            entity
        );
        let Some(mut commands) = commands.get_entity(entity) else {
            continue;
        };
        commands.insert(DirtyAttr::<A>::default());
    }
}

impl<M: ModifierGeneric<A> + Component, A: Attribute> Plugin for ModifierGenericPlugin<M, A> {
    fn build(&self, app: &mut App) {
        app.add_system_set_to_stage(
            CoreStage::PostUpdate,
            SystemSet::new()
                .before(refresh_dirty_attr::<A>)
                .with_system(modifier_changed::<M, A>)
                .with_system(modifier_removed::<M, A>),
        );
        app.register_component_as::<dyn ModifierGeneric<A>, M>();
    }
}

pub type ModifierPlugin<M> = ModifierGenericPlugin<M, <M as Modifier>::Attr>;

#[cfg(test)]
mod tests {}
