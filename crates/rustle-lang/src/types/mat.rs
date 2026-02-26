//! Pure matrix math helpers — no Value types, no RuntimeError.
//! Row-major storage: element at (row, col) = data[row * N + col].
//!
//! Used by both types/registry.rs (method impls) and types/binop_registry.rs (operator impls).

use crate::error::RuntimeError;

// ─── Mat3 ─────────────────────────────────────────────────────────────────────

pub type M3 = [f64; 9];

#[inline] pub fn m3_at(m: &M3, row: usize, col: usize) -> f64 { m[row * 3 + col] }

pub fn m3_identity() -> M3 {
    [1., 0., 0.,
     0., 1., 0.,
     0., 0., 1.]
}

pub fn m3_mul(a: &M3, b: &M3) -> M3 {
    let mut c = [0.0f64; 9];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                c[i * 3 + j] += a[i * 3 + k] * b[k * 3 + j];
            }
        }
    }
    c
}

pub fn m3_mul_vec(m: &M3, (vx, vy, vz): (f64, f64, f64)) -> (f64, f64, f64) {
    (
        m[0] * vx + m[1] * vy + m[2] * vz,
        m[3] * vx + m[4] * vy + m[5] * vz,
        m[6] * vx + m[7] * vy + m[8] * vz,
    )
}

pub fn m3_scale(m: &M3, s: f64) -> M3 {
    let mut r = *m;
    for v in r.iter_mut() { *v *= s; }
    r
}

pub fn m3_transpose(m: &M3) -> M3 {
    [m[0], m[3], m[6],
     m[1], m[4], m[7],
     m[2], m[5], m[8]]
}

pub fn m3_det(m: &M3) -> f64 {
    m[0] * (m[4] * m[8] - m[5] * m[7])
  - m[1] * (m[3] * m[8] - m[5] * m[6])
  + m[2] * (m[3] * m[7] - m[4] * m[6])
}

pub fn m3_inverse(m: &M3, line: usize) -> Result<M3, RuntimeError> {
    let det = m3_det(m);
    if det.abs() < 1e-15 {
        return Err(RuntimeError::new(line, "mat3 is singular (not invertible)"));
    }
    let d = 1.0 / det;
    Ok([
         (m[4]*m[8] - m[5]*m[7]) * d,  -(m[1]*m[8] - m[2]*m[7]) * d,  (m[1]*m[5] - m[2]*m[4]) * d,
        -(m[3]*m[8] - m[5]*m[6]) * d,   (m[0]*m[8] - m[2]*m[6]) * d, -(m[0]*m[5] - m[2]*m[3]) * d,
         (m[3]*m[7] - m[4]*m[6]) * d,  -(m[0]*m[7] - m[1]*m[6]) * d,  (m[0]*m[4] - m[1]*m[3]) * d,
    ])
}

// ─── Mat4 ─────────────────────────────────────────────────────────────────────

pub type M4 = [f64; 16];

#[inline] pub fn m4_at(m: &M4, row: usize, col: usize) -> f64 { m[row * 4 + col] }

pub fn m4_identity() -> M4 {
    [1., 0., 0., 0.,
     0., 1., 0., 0.,
     0., 0., 1., 0.,
     0., 0., 0., 1.]
}

pub fn m4_mul(a: &M4, b: &M4) -> M4 {
    let mut c = [0.0f64; 16];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                c[i * 4 + j] += a[i * 4 + k] * b[k * 4 + j];
            }
        }
    }
    c
}

pub fn m4_mul_vec(m: &M4, (vx, vy, vz, vw): (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    (
        m[ 0]*vx + m[ 1]*vy + m[ 2]*vz + m[ 3]*vw,
        m[ 4]*vx + m[ 5]*vy + m[ 6]*vz + m[ 7]*vw,
        m[ 8]*vx + m[ 9]*vy + m[10]*vz + m[11]*vw,
        m[12]*vx + m[13]*vy + m[14]*vz + m[15]*vw,
    )
}

pub fn m4_scale(m: &M4, s: f64) -> M4 {
    let mut r = *m;
    for v in r.iter_mut() { *v *= s; }
    r
}

pub fn m4_transpose(m: &M4) -> M4 {
    let mut t = [0.0f64; 16];
    for i in 0..4 {
        for j in 0..4 {
            t[i * 4 + j] = m[j * 4 + i];
        }
    }
    t
}

/// Determinant of the 3×3 minor obtained by deleting `skip_row` and `skip_col`.
fn minor3(m: &M4, skip_row: usize, skip_col: usize) -> f64 {
    let rows = {
        let mut r = [0usize; 3]; let mut idx = 0;
        for i in 0..4 { if i != skip_row { r[idx] = i; idx += 1; } }
        r
    };
    let cols = {
        let mut c = [0usize; 3]; let mut idx = 0;
        for j in 0..4 { if j != skip_col { c[idx] = j; idx += 1; } }
        c
    };
    let a = |r, c| m[rows[r] * 4 + cols[c]];
    a(0,0) * (a(1,1)*a(2,2) - a(1,2)*a(2,1))
  - a(0,1) * (a(1,0)*a(2,2) - a(1,2)*a(2,0))
  + a(0,2) * (a(1,0)*a(2,1) - a(1,1)*a(2,0))
}

pub fn m4_det(m: &M4) -> f64 {
    (0..4).map(|j| {
        let sign = if j % 2 == 0 { 1.0 } else { -1.0 };
        sign * m[j] * minor3(m, 0, j)
    }).sum()
}

pub fn m4_inverse(m: &M4, line: usize) -> Result<M4, RuntimeError> {
    let det = m4_det(m);
    if det.abs() < 1e-15 {
        return Err(RuntimeError::new(line, "mat4 is singular (not invertible)"));
    }
    let d = 1.0 / det;
    let mut inv = [0.0f64; 16];
    for i in 0..4 {
        for j in 0..4 {
            let sign = if (i + j) % 2 == 0 { 1.0 } else { -1.0 };
            // Transpose the cofactor matrix (adjugate)
            inv[j * 4 + i] = sign * minor3(m, i, j) * d;
        }
    }
    Ok(inv)
}

// ─── Mat3 graphics constructors ──────────────────────────────────────────────

pub fn m3_translate2d(tx: f64, ty: f64) -> M3 {
    [1., 0., tx,
     0., 1., ty,
     0., 0.,  1.]
}

pub fn m3_rotate2d(angle_rad: f64) -> M3 {
    let (s, c) = angle_rad.sin_cos();
    [ c, -s, 0.,
      s,  c, 0.,
      0., 0., 1.]
}

pub fn m3_scale2d(sx: f64, sy: f64) -> M3 {
    [sx,  0., 0.,
      0., sy, 0.,
      0., 0., 1.]
}

// ─── Mat4 graphics constructors ──────────────────────────────────────────────

pub fn m4_translate(tx: f64, ty: f64, tz: f64) -> M4 {
    [1., 0., 0., tx,
     0., 1., 0., ty,
     0., 0., 1., tz,
     0., 0., 0., 1.]
}

pub fn m4_scale_xyz(sx: f64, sy: f64, sz: f64) -> M4 {
    [sx,  0.,  0.,  0.,
     0.,  sy,  0.,  0.,
     0.,  0.,  sz,  0.,
     0.,  0.,  0.,  1.]
}

pub fn m4_rotate_x(angle: f64) -> M4 {
    let (s, c) = angle.sin_cos();
    [1., 0.,  0., 0.,
     0., c,  -s,  0.,
     0., s,   c,  0.,
     0., 0.,  0., 1.]
}

pub fn m4_rotate_y(angle: f64) -> M4 {
    let (s, c) = angle.sin_cos();
    [ c,  0., s,  0.,
      0., 1., 0., 0.,
     -s,  0., c,  0.,
      0., 0., 0., 1.]
}

pub fn m4_rotate_z(angle: f64) -> M4 {
    let (s, c) = angle.sin_cos();
    [c, -s,  0., 0.,
     s,  c,  0., 0.,
     0., 0., 1., 0.,
     0., 0., 0., 1.]
}
