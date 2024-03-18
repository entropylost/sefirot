use luisa::lang::types::vector::Vec4;

use super::*;

pub trait HasPixelStorage: IoTexel {
    fn storage() -> PixelStorage;
}
impl HasPixelStorage for f32 {
    fn storage() -> PixelStorage {
        PixelStorage::Float1
    }
}
impl HasPixelStorage for Vec2<f32> {
    fn storage() -> PixelStorage {
        PixelStorage::Float2
    }
}
impl HasPixelStorage for Vec3<f32> {
    fn storage() -> PixelStorage {
        PixelStorage::Float4
    }
}
impl HasPixelStorage for Vec4<f32> {
    fn storage() -> PixelStorage {
        PixelStorage::Float4
    }
}

impl HasPixelStorage for u32 {
    fn storage() -> PixelStorage {
        PixelStorage::Int1
    }
}
impl HasPixelStorage for Vec2<u32> {
    fn storage() -> PixelStorage {
        PixelStorage::Int2
    }
}
impl HasPixelStorage for Vec4<u32> {
    fn storage() -> PixelStorage {
        PixelStorage::Int4
    }
}

impl HasPixelStorage for i32 {
    fn storage() -> PixelStorage {
        PixelStorage::Int1
    }
}
impl HasPixelStorage for Vec2<i32> {
    fn storage() -> PixelStorage {
        PixelStorage::Int2
    }
}
impl HasPixelStorage for Vec4<i32> {
    fn storage() -> PixelStorage {
        PixelStorage::Int4
    }
}
