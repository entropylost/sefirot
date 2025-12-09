// Taken from https://www.shadertoy.com/view/cd3XWr
// and https://iolite-engine.com/blog_posts/minimal_agx_implementation

// Also take a look at
// https://www.shadertoy.com/view/Dt3XDr
// and https://www.shadertoy.com/view/clGGWG

// MIT License
//
// Copyright (c) 2024 Missing Deadlines (Benjamin Wrensch)
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

// All values used to derive this implementation are sourced from Troy’s initial AgX implementation/OCIO config file available here:
//   https://github.com/sobotka/AgX

use keter::lang::types::vector::{Mat3, Vec3};
use keter::prelude::*;

// Mean error^2: 3.6705141e-06
#[tracked]
fn agx_default_contrast_approx(x: Expr<Vec3<f32>>) -> Expr<Vec3<f32>> {
    let x2 = x * x;
    let x4 = x2 * x2;

    15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x
        - 0.00232
}

#[tracked]
pub fn agx(color: Expr<Vec3<f32>>) -> Expr<Vec3<f32>> {
    #[allow(clippy::excessive_precision)]
    let agx_mat = Mat3::from_column_array(&[
        [0.842479062253094, 0.0423282422610123, 0.0423756549057051],
        [0.0784335999999992, 0.878468636469772, 0.0784336],
        [0.0792237451477643, 0.0791661274605434, 0.879142973793104],
    ]);
    let min_ev = -12.47393_f32;
    let max_ev = 4.026069_f32;

    let color = agx_mat.expr() * color;
    let color = color.log2().clamp(min_ev, max_ev);
    let color = (color - min_ev) / (max_ev - min_ev);
    agx_default_contrast_approx(color)
}

#[tracked]
pub fn agx_eotf(color: Expr<Vec3<f32>>) -> Expr<Vec3<f32>> {
    #[allow(clippy::excessive_precision)]
    let agx_mat_inv = Mat3::from_column_array(&[
        [1.19687900512017, -0.0528968517574562, -0.0529716355144438],
        [-0.0980208811401368, 1.15190312990417, -0.0980434501171241],
        [-0.0990297440797205, -0.0989611768448433, 1.15107367264116],
    ]);

    agx_mat_inv.expr() * color
    // No need to linearize since outputting to sRGB.
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Value, PartialEq)]
pub struct AgXParameters {
    pub offset: Vec3<f32>,
    pub slope: Vec3<f32>,
    pub power: Vec3<f32>,
    pub saturation: f32,
}
impl Default for AgXParameters {
    fn default() -> Self {
        Self {
            offset: Vec3::splat(0.0),
            slope: Vec3::splat(1.0),
            power: Vec3::splat(1.0),
            saturation: 1.0,
        }
    }
}
impl AgXParameters {
    pub fn golden() -> Self {
        Self {
            offset: Vec3::splat(0.0),
            slope: Vec3::new(1.0, 0.9, 0.5),
            power: Vec3::splat(0.8),
            saturation: 0.8,
        }
    }
    pub fn punchy() -> Self {
        Self {
            offset: Vec3::splat(0.0),
            slope: Vec3::splat(1.0),
            power: Vec3::splat(1.35),
            saturation: 1.4,
        }
    }
}

#[tracked]
pub fn agx_look(color: Expr<Vec3<f32>>, args: AgXParameters) -> Expr<Vec3<f32>> {
    let lw = Vec3::new(0.2126, 0.7152, 0.0722);
    let luma = color.dot(lw);

    let color = (color * args.slope + args.offset).powf(args.power);
    luma + args.saturation * (color - luma)
}

#[tracked]
pub fn agx_tonemap(color: Expr<Vec3<f32>>, params: Option<AgXParameters>) -> Expr<Vec3<f32>> {
    let color = agx(color);
    let color = if let Some(params) = params {
        agx_look(color, params)
    } else {
        color
    };
    agx_eotf(color)
}
