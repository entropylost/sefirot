use keter::lang::types::vector::Vec2;
use keter::prelude::*;
use nalgebra::SVector as Vector;

use crate::utils::{iter_grid, to_linear};

pub fn index_matrix<const D: usize>(n: u32, ordering: &[u32]) -> Vec<u32> {
    assert!(n.is_power_of_two());
    if n == 0 {
        panic!("Bayer matrix of order 0 is not defined");
    } else if n == 1 {
        return vec![0];
    }
    let n2 = n / 2;
    let prev_matrix = index_matrix::<D>(n2, ordering);
    let mut next_matrix = vec![0; n.pow(D as u32) as usize];
    for idx in iter_grid(Vector::<_, D>::repeat(n2)) {
        let v = prev_matrix[to_linear(idx, Vector::repeat(n2))] << D;
        for offset in iter_grid(Vector::repeat(2)) {
            let add = ordering[to_linear(offset, Vector::repeat(2))];
            next_matrix[to_linear(offset * n2 + idx, Vector::repeat(n))] = v + add;
        }
    }
    next_matrix
}

// apparently can be done non-recursively: https://en.wikipedia.org/wiki/Ordered_dithering
pub fn bayer2(n: u32) -> Vec<u32> {
    index_matrix::<2>(n, &[0, 2, 3, 1])
}

// 3d: baker matrix or something; https://jbaker.graphics/writings/bayer.html.
pub fn bayer3(n: u32) -> Vec<u32> {
    index_matrix::<3>(n, &[0, 2, 6, 4, 5, 7, 3, 1])
}

#[tracked]
pub fn ign(pixel: Expr<Vec2<u32>>) -> Expr<f32> {
    (52.982918 * (0.06711056 * pixel.x.cast_f32() + 0.00583715 * pixel.y.cast_f32()).fract())
        .fract()
}

// JBaker's ordering seems to work better than my attempts; could try enumerating all possible orderings.
/*
#[test]
fn test_indexing() {
    const N: usize = 16;
    fn get_reverse_indices(arr: &[u32]) -> Vec<u32> {
        let mut rev = vec![0; arr.len()];
        for (i, &v) in arr.iter().enumerate() {
            rev[v as usize] = i as u32;
        }
        rev
    }
    fn get_total_length(arr: &[u32]) -> f64 {
        arr.array_windows::<2>()
            .map(|points| {
                let points = points.map(|p| {
                    from_linear(p as usize, Vector::<u32, 3>::repeat(N as u32)).cast::<f64>()
                });
                (points[0] - points[1]).norm()
            })
            .sum()
    }
    let jb_ordering = vec![0, 2, 6, 4, 5, 7, 3, 1];
    let baker_matrix = index_matrix::<3>(N as u32, &jb_ordering);
    let bayer_matrix = bayer::<3>(N as u32);
    let rev_baker = get_reverse_indices(&baker_matrix);
    let rev_bayer = get_reverse_indices(&bayer_matrix);
    let baker_length = get_total_length(&rev_baker);
    let bayer_length = get_total_length(&rev_bayer);
    panic!("Baker length: {baker_length}, Bayer length: {bayer_length}");
}
*/
