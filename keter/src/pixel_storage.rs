use super::lang::types::vector::{Vec2, Vec3, Vec4};
use super::prelude::*;

pub trait HasPixelStorage: IoTexel {
    fn natural_storage() -> PixelStorage;
}
impl HasPixelStorage for f32 {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Float1
    }
}
impl HasPixelStorage for Vec2<f32> {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Float2
    }
}
impl HasPixelStorage for Vec3<f32> {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Float4
    }
}
impl HasPixelStorage for Vec4<f32> {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Float4
    }
}
impl HasPixelStorage for f16 {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Half1
    }
}
impl HasPixelStorage for Vec2<f16> {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Half2
    }
}
impl HasPixelStorage for Vec3<f16> {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Half4
    }
}
impl HasPixelStorage for Vec4<f16> {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Half4
    }
}

impl HasPixelStorage for u32 {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Int1
    }
}
impl HasPixelStorage for Vec2<u32> {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Int2
    }
}
impl HasPixelStorage for Vec4<u32> {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Int4
    }
}

impl HasPixelStorage for i32 {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Int1
    }
}
impl HasPixelStorage for Vec2<i32> {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Int2
    }
}
impl HasPixelStorage for Vec4<i32> {
    fn natural_storage() -> PixelStorage {
        PixelStorage::Int4
    }
}
