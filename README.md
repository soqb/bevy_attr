# bevy_attr
[![Crates.io](https://img.shields.io/crates/v/bevy_attr.svg)](https://crates.io/crates/bevy_attr)
![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)
[![Docs.rs](https://img.shields.io/badge/docs.rs-bevy_attr-ffffff)](https://docs.rs/bevy_attr/latest/bevy_attr)
[![GitHub](https://img.shields.io/badge/github-soqb/bevy_attr-8da0cb?&logo=github)](https://github.com/soqb/bevy_attr)

Automatic management of highly modular, highly parallel values in [Bevy](https://bevyengine.org) with minimal overhead.

|bevy version|crate version|
|------------|-------------|
|0.9.0       |0.1.0        |

This crate consists of attributes which are components that contain simple values and modifiers (also components) which automatically edit the values of attributes.

The relationship between the [`Attribute`](https://docs.rs/bevy_attr/latest/bevy_attr/struct.Attribute.html) and [`Modifier`](https://docs.rs/bevy_attr/latest/bevy_attr/struct.Modifier.html) traits can be thought of as similar to that between Bevy's [`GlobalTransform`](https://docs.rs/bevy/latest/bevy/transform/components/struct.GlobalTransform.html) and [`Transform`](https://docs.rs/bevy/latest/bevy/transform/components/struct.Transform.html) components. Once per frame, any changed, added, or removed modifiers will force a refresh on the attribute in the same entity. All of the modifiers on the entity will be collected and sorted and sequentially re-applied to the default value of the attribute. Although impractical, `Transform` *could* be implemented as a modifier on a `GlobalTransform` which copies the data in the `Transform` into the `GlobalTransform`, but this crate works best with many concurrent modifiers.

In a Bevy app, example uses of attributes could be stat systems where `Strength` and `Endurance` both implement the `Attribute` trait and whose modifiers are updated infrequently, or more dynamic systems where `Health` and `MaxHealth` implement it. Since both attributes and modifiers are just components, in that example, it may be a good idea to have `MaxHealth` be a modifier for `Health`.

[View the examples here](https://github.com/soqb/bevy_attr/tree/main/examples)
