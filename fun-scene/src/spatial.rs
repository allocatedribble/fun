use fun_ecs::{Component, Entity, FunFromTemplate};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };

    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[must_use]
    pub const fn splat(value: f32) -> Self {
        Self {
            x: value,
            y: value,
            z: value,
        }
    }

    #[must_use]
    pub const fn from_array(value: [f32; 3]) -> Self {
        Self {
            x: value[0],
            y: value[1],
            z: value[2],
        }
    }

    #[must_use]
    pub const fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    #[must_use]
    pub fn add_components(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }

    #[must_use]
    pub fn mul_scalar(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }

    #[must_use]
    pub fn mul_components(self, rhs: Self) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
            z: self.z * rhs.z,
        }
    }

    #[must_use]
    pub fn cross(self, rhs: Self) -> Self {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    #[must_use]
    pub const fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    #[must_use]
    pub fn from_rotation_z(radians: f32) -> Self {
        let half = radians * 0.5;
        Self {
            x: 0.0,
            y: 0.0,
            z: half.sin(),
            w: half.cos(),
        }
    }

    #[must_use]
    pub fn normalized(self) -> Self {
        let length_squared = self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w;
        if length_squared <= f32::EPSILON {
            return Self::IDENTITY;
        }
        let inv_length = 1.0 / length_squared.sqrt();
        Self {
            x: self.x * inv_length,
            y: self.y * inv_length,
            z: self.z * inv_length,
            w: self.w * inv_length,
        }
    }

    #[must_use]
    pub fn mul_quat(self, rhs: Self) -> Self {
        Self {
            x: self.w * rhs.x + self.x * rhs.w + self.y * rhs.z - self.z * rhs.y,
            y: self.w * rhs.y - self.x * rhs.z + self.y * rhs.w + self.z * rhs.x,
            z: self.w * rhs.z + self.x * rhs.y - self.y * rhs.x + self.z * rhs.w,
            w: self.w * rhs.w - self.x * rhs.x - self.y * rhs.y - self.z * rhs.z,
        }
    }

    #[must_use]
    pub fn rotate_vec3(self, value: Vec3) -> Vec3 {
        let q = self.normalized();
        let axis = Vec3::new(q.x, q.y, q.z);
        let t = axis.cross(value).mul_scalar(2.0);
        value
            .add_components(t.mul_scalar(q.w))
            .add_components(axis.cross(t))
    }
}

impl Default for Quat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FunFromTemplate, Component)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    #[must_use]
    pub const fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    #[must_use]
    pub const fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        Self::from_translation(Vec3::new(x, y, z))
    }

    #[must_use]
    pub const fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    #[must_use]
    pub const fn with_scale(mut self, scale: Vec3) -> Self {
        self.scale = scale;
        self
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct GlobalTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl GlobalTransform {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    #[must_use]
    pub const fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        Self {
            translation: Vec3::new(x, y, z),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    #[must_use]
    pub const fn from_transform(transform: Transform) -> Self {
        Self {
            translation: transform.translation,
            rotation: transform.rotation,
            scale: transform.scale,
        }
    }

    #[must_use]
    pub fn propagate(parent: Self, local: Transform) -> Self {
        let scaled_translation = local.translation.mul_components(parent.scale);
        Self {
            translation: parent
                .translation
                .add_components(parent.rotation.rotate_vec3(scaled_translation)),
            rotation: parent.rotation.mul_quat(local.rotation).normalized(),
            scale: parent.scale.mul_components(local.scale),
        }
    }

    #[must_use]
    pub const fn to_transform(self) -> Transform {
        Transform {
            translation: self.translation,
            rotation: self.rotation,
            scale: self.scale,
        }
    }

    #[must_use]
    pub const fn translation(self) -> Vec3 {
        self.translation
    }

    #[must_use]
    pub fn matrix_columns(self) -> [[f32; 4]; 4] {
        let q = self.rotation.normalized();
        let x2 = q.x + q.x;
        let y2 = q.y + q.y;
        let z2 = q.z + q.z;
        let xx = q.x * x2;
        let yy = q.y * y2;
        let zz = q.z * z2;
        let xy = q.x * y2;
        let xz = q.x * z2;
        let yz = q.y * z2;
        let wx = q.w * x2;
        let wy = q.w * y2;
        let wz = q.w * z2;
        [
            [
                (1.0 - (yy + zz)) * self.scale.x,
                (xy + wz) * self.scale.x,
                (xz - wy) * self.scale.x,
                0.0,
            ],
            [
                (xy - wz) * self.scale.y,
                (1.0 - (xx + zz)) * self.scale.y,
                (yz + wx) * self.scale.y,
                0.0,
            ],
            [
                (xz + wy) * self.scale.z,
                (yz - wx) * self.scale.z,
                (1.0 - (xx + yy)) * self.scale.z,
                0.0,
            ],
            [
                self.translation.x,
                self.translation.y,
                self.translation.z,
                1.0,
            ],
        ]
    }
}

impl Default for GlobalTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct Parent {
    pub entity: Entity,
}

impl Parent {
    #[must_use]
    pub const fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Component)]
pub struct Children {
    entities: Vec<Entity>,
}

impl Children {
    #[must_use]
    pub fn from_entities(entities: impl IntoIterator<Item = Entity>) -> Self {
        let mut children = Self::default();
        for entity in entities {
            children.push(entity);
        }
        children
    }

    pub fn push(&mut self, entity: Entity) -> bool {
        if !entity.is_valid() || self.entities.contains(&entity) {
            return false;
        }
        self.entities.push(entity);
        true
    }

    pub fn remove(&mut self, entity: Entity) -> bool {
        let Some(index) = self
            .entities
            .iter()
            .position(|candidate| *candidate == entity)
        else {
            return false;
        };
        self.entities.remove(index);
        true
    }

    #[must_use]
    pub fn as_slice(&self) -> &[Entity] {
        &self.entities
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entities.len()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, FunFromTemplate, Component)]
pub enum Visibility {
    #[default]
    Inherited,
    Visible,
    Hidden,
}

impl Visibility {
    #[must_use]
    pub const fn locally_visible(self) -> bool {
        !matches!(self, Self::Hidden)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct InheritedVisibility {
    pub visible: bool,
}

impl InheritedVisibility {
    pub const VISIBLE: Self = Self { visible: true };
    pub const HIDDEN: Self = Self { visible: false };
}

impl Default for InheritedVisibility {
    fn default() -> Self {
        Self::VISIBLE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct ViewVisibility {
    pub visible: bool,
}

impl ViewVisibility {
    pub const VISIBLE: Self = Self { visible: true };
    pub const CULLED: Self = Self { visible: false };
}

impl Default for ViewVisibility {
    fn default() -> Self {
        Self::VISIBLE
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, FunFromTemplate, Component)]
pub struct SceneNode {
    pub flags: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, FunFromTemplate, Component)]
pub struct SceneRoot;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TransformPropagationStats {
    pub root_count: u32,
    pub propagated_count: u32,
}

#[must_use]
pub fn propagate_child_transform(parent: GlobalTransform, child: Transform) -> GlobalTransform {
    GlobalTransform::propagate(parent, child)
}

#[cfg(test)]
mod tests {
    use fun_ecs::World;

    use super::*;

    #[test]
    fn child_transform_propagates_through_parent_transform() {
        let parent = GlobalTransform::from_transform(
            Transform::from_xyz(10.0, 0.0, 0.0).with_scale(Vec3::splat(2.0)),
        );
        let child = Transform::from_xyz(1.0, 2.0, 3.0);

        let global = propagate_child_transform(parent, child);

        assert_eq!(global.translation, Vec3::new(12.0, 4.0, 6.0));
        assert_eq!(global.scale, Vec3::splat(2.0));
    }

    #[test]
    fn parent_child_components_preserve_relationships() {
        let mut world = World::new();
        let parent = world.spawn(SceneRoot).id();
        world
            .entity_mut(parent)
            .insert(SceneNode::default())
            .insert(Children::default());
        let child = world.spawn(SceneNode::default()).id();
        world
            .entity_mut(child)
            .insert(Parent::new(parent))
            .insert(Transform::default());

        let children = world
            .get_mut::<Children>(parent)
            .expect("root should own a children component");

        assert!(children.push(child));
        assert!(!children.push(child));
        assert_eq!(children.as_slice(), &[child]);

        let parent_component = world
            .get::<Parent>(child)
            .expect("child should record its parent");
        assert_eq!(parent_component.entity, parent);
    }

    #[test]
    fn visibility_tracks_local_inherited_and_view_state() {
        assert!(Visibility::Visible.locally_visible());
        assert!(Visibility::Inherited.locally_visible());
        assert!(!Visibility::Hidden.locally_visible());
        const { assert!(InheritedVisibility::VISIBLE.visible) };
        const { assert!(!ViewVisibility::CULLED.visible) };
    }
}
