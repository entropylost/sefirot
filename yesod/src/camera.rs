use std::collections::HashSet;

use glam::{Mat3 as FMat3, Vec2 as FVec2, Vec3 as FVec3};
use keter::lang::types::vector::{Mat3, Vec2, Vec3};
use keter::prelude::*;
use winit::keyboard::KeyCode;

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub screen_size: FVec2,
    pub yaw: f32,
    pub pitch: f32,
    pub pos: FVec3,
    pub fov: f32,
}
impl Camera {
    pub fn orbit(self, radius: f32) -> Self {
        Self {
            pos: self.pos + self.rotation() * FVec3::Z * radius,
            ..self
        }
    }
    pub fn rotation(&self) -> FMat3 {
        let view = FMat3::from_euler(glam::EulerRot::YXZ, self.yaw, self.pitch, 0.0).transpose();
        FMat3::from_cols(view.x_axis, view.z_axis, view.y_axis).transpose()
    }
    pub fn view(&self) -> View {
        let view = self.rotation();
        let ratio = (self.fov / 2.0).tan() / (self.screen_size.y / 2.0);
        let transform = FMat3::from_cols(view.x_axis, -view.y_axis, view.z_axis);

        View {
            screen_size: self.screen_size.into(),
            scaling: ratio,
            pos: self.pos.into(),
            transform: transform.into(),
        }
    }
    pub fn forward(&self) -> FVec3 {
        self.rotation().z_axis
    }
    pub fn forward_horiz(&self) -> FVec3 {
        FVec3::new(self.yaw.sin(), self.yaw.cos(), 0.0)
    }
    pub fn right(&self) -> FVec3 {
        self.rotation().x_axis
    }
    pub fn apply_movement(
        &mut self,
        keys: &HashSet<KeyCode>,
        rot_speed: f32,
        move_speed: f32,
    ) -> bool {
        let mut updated = false;
        for key in keys {
            let mut key_updated = true;
            match key {
                KeyCode::ArrowLeft => self.yaw -= rot_speed,
                KeyCode::ArrowRight => self.yaw += rot_speed,
                KeyCode::ArrowUp => self.pitch -= rot_speed,
                KeyCode::ArrowDown => self.pitch += rot_speed,
                KeyCode::KeyW => self.pos += self.forward_horiz() * move_speed,
                KeyCode::KeyS => self.pos -= self.forward_horiz() * move_speed,
                KeyCode::KeyA => self.pos -= self.right() * move_speed,
                KeyCode::KeyD => self.pos += self.right() * move_speed,
                KeyCode::Space => self.pos += FVec3::new(0.0, 0.0, 1.0) * move_speed,
                KeyCode::ShiftLeft => self.pos -= FVec3::new(0.0, 0.0, 1.0) * move_speed,
                _ => {
                    key_updated = false;
                }
            }
            updated |= key_updated;
        }
        updated
    }
}

#[derive(Debug, Clone, Copy, Value)]
#[repr(C)]
pub struct View {
    pub screen_size: Vec2<f32>,
    pub scaling: f32,
    pub pos: Vec3<f32>,
    pub transform: Mat3,
}
impl ViewExpr {
    #[tracked]
    pub fn facing(&self) -> Expr<Vec3<f32>> {
        self.transform.col(2)
    }
    #[tracked]
    pub fn ray_dir(&self, pixel: Expr<Vec2<f32>>) -> Expr<Vec3<f32>> {
        self.transform * ((pixel - self.screen_size / 2.0) * self.scaling).extend(1.0)
    }
    #[tracked]
    pub fn project3(&self, world_pos: Expr<Vec3<f32>>) -> Expr<Vec3<f32>> {
        let local_pos = world_pos - self.pos;
        // TODO: A little bit inefficient.
        let view_pos = self.transform.transpose() * local_pos;
        ((view_pos.xy() / view_pos.z) / self.scaling + self.screen_size / 2.0).extend(view_pos.z)
    }
    #[tracked]
    pub fn project(&self, world_pos: Expr<Vec3<f32>>) -> Expr<Vec2<f32>> {
        let local_pos = world_pos - self.pos;
        // TODO: A little bit inefficient.
        let view_pos = self.transform.transpose() * local_pos;
        (view_pos.xy() / view_pos.z) / self.scaling + self.screen_size / 2.0
    }
}
