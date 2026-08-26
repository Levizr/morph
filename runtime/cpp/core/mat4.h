#pragma once
#include <cmath>
#include <algorithm>

// Column-major 4x4 matrix math: m[col * 4 + row].
//
// Matches the semantics of the Python composer in morph/style/transforms.py:
// transforms compose left-to-right (v' = M·v with M = T1·T2·...), and CSS
// `matrix3d` values are stored as-is (already column-major).
//
// All functions are inline and header-only so unused code is eliminated by
// -ffunction-sections / --gc-sections when MORPH_FEATURE_TRANSFORM is off.

namespace morph {

inline void mat4Identity(float m[16]) {
    for (int i = 0; i < 16; i++) m[i] = 0.0f;
    m[0] = m[5] = m[10] = m[15] = 1.0f;
}

inline bool mat4IsIdentity(const float m[16]) {
    for (int i = 0; i < 16; i++) {
        float expected = (i % 5 == 0) ? 1.0f : 0.0f;
        if (std::fabs(m[i] - expected) > 1e-6f) return false;
    }
    return true;
}

inline void mat4Copy(float out[16], const float a[16]) {
    for (int i = 0; i < 16; i++) out[i] = a[i];
}

// out = a * b  (apply a first, then b — same as the Python `multiply`)
inline void mat4Multiply(float out[16], const float a[16], const float b[16]) {
    float tmp[16];
    for (int col = 0; col < 4; col++) {
        for (int row = 0; row < 4; row++) {
            float acc = 0.0f;
            for (int k = 0; k < 4; k++) acc += a[k * 4 + row] * b[col * 4 + k];
            tmp[col * 4 + row] = acc;
        }
    }
    for (int i = 0; i < 16; i++) out[i] = tmp[i];
}

// Transform a point with full perspective divide. Returns false if w == 0.
inline bool mat4TransformPoint(const float m[16], float x, float y, float z,
                               float& ox, float& oy, float& oz) {
    float w = m[3] * x + m[7] * y + m[11] * z + m[15];
    if (std::fabs(w) < 1e-12f) return false;
    float inv = 1.0f / w;
    ox = (m[0] * x + m[4] * y + m[8] * z + m[12]) * inv;
    oy = (m[1] * x + m[5] * y + m[9] * z + m[13]) * inv;
    oz = (m[2] * x + m[6] * y + m[10] * z + m[14]) * inv;
    return true;
}

// General 4x4 inverse via Gauss-Jordan with partial pivoting.
inline bool mat4Inverse(const float m[16], float out[16]) {
    float a[16];
    float inv[16];
    for (int i = 0; i < 16; i++) a[i] = m[i];
    mat4Identity(inv);
    for (int col = 0; col < 4; col++) {
        int pivot = col;
        float best = std::fabs(a[col * 4 + col]);
        for (int r = col + 1; r < 4; r++) {
            float v = std::fabs(a[col * 4 + r]);
            if (v > best) { best = v; pivot = r; }
        }
        if (best < 1e-12f) return false;
        if (pivot != col) {
            for (int c = 0; c < 4; c++) {
                std::swap(a[c * 4 + col], a[c * 4 + pivot]);
                std::swap(inv[c * 4 + col], inv[c * 4 + pivot]);
            }
        }
        float d = a[col * 4 + col];
        for (int c = 0; c < 4; c++) {
            a[c * 4 + col] /= d;
            inv[c * 4 + col] /= d;
        }
        for (int r = 0; r < 4; r++) {
            if (r == col) continue;
            float f = a[col * 4 + r];
            if (f == 0.0f) continue;
            for (int c = 0; c < 4; c++) {
                a[c * 4 + r] -= f * a[c * 4 + col];
                inv[c * 4 + r] -= f * inv[c * 4 + col];
            }
        }
    }
    for (int i = 0; i < 16; i++) out[i] = inv[i];
    return true;
}

// ═══════════════════════════════════════════════════════════════
//  Decompose / recompose (W3C css-transforms-2 §6.1)
// ═══════════════════════════════════════════════════════════════

struct Mat4Decomposed2D {
    float translate[2];
    float scale[2];
    float angle;            // degrees (informational; m11..m22 carry the rotation)
    float m11, m12, m21, m22;  // normalized 2x2 (rotation + shear)
};

struct Mat4Decomposed {
    float translate[3];
    float scale[3];
    float skew[3];
    float perspective[4];
    float quaternion[4];    // (x, y, z, w)
};

// ── 2D QR decompose ────────────────────────────────────────────
// Detects the exact zero-structure of a 2D transform matrix and
// produces an exact round-trip with mat4Recompose2D.
inline bool mat4Decompose2D(const float m[16], Mat4Decomposed2D& d) {
    if (!(m[2] == 0.0f && m[6] == 0.0f && m[8] == 0.0f && m[9] == 0.0f &&
          m[11] == 0.0f && m[14] == 0.0f && m[3] == 0.0f && m[7] == 0.0f &&
          m[10] == 1.0f && m[15] == 1.0f))
        return false;
    float x1 = m[0], y1 = m[1];  // column 1 (x-axis)
    float x2 = m[4], y2 = m[5];  // column 2 (y-axis)
    float s0 = std::sqrt(x1 * x1 + y1 * y1);
    float s1 = std::sqrt(x2 * x2 + y2 * y2);
    if (s0 < 1e-12f || s1 < 1e-12f) return false;
    x1 /= s0; y1 /= s0;
    x2 /= s1; y2 /= s1;
    // Ensure right-handed: flip the x-axis (and its scale) if det < 0.
    if (x1 * y2 - x2 * y1 < 0.0f) {
        s0 = -s0;
        x1 = -x1;
        y1 = -y1;
    }
    d.translate[0] = m[12];
    d.translate[1] = m[13];
    d.scale[0] = s0;
    d.scale[1] = s1;
    d.angle = std::atan2(y1, x1) * 180.0f / 3.14159265358979323846f;
    d.m11 = x1; d.m21 = y1;
    d.m12 = x2; d.m22 = y2;
    return true;
}

inline void mat4Recompose2D(const Mat4Decomposed2D& d, float out[16]) {
    mat4Identity(out);
    out[0] = d.m11 * d.scale[0];
    out[1] = d.m21 * d.scale[0];
    out[4] = d.m12 * d.scale[1];
    out[5] = d.m22 * d.scale[1];
    out[12] = d.translate[0];
    out[13] = d.translate[1];
}

// ── 3D decompose (W3C algorithm) ───────────────────────────────
inline bool mat4Decompose3D(const float m[16], Mat4Decomposed& d) {
    float mm[16];
    for (int i = 0; i < 16; i++) mm[i] = m[i];

    // Normalize by m34 when non-zero (CSS perspective convention).
    if (mm[14] != 0.0f) {
        for (int i = 0; i < 16; i++) mm[i] /= mm[14];
    }

    // Perspective lives in the bottom row (m41, m42, m43, m44).
    float persp[4] = { mm[3], mm[7], mm[11], mm[15] };
    float perspective[4] = { 0.0f, 0.0f, 0.0f, 1.0f };
    if (persp[0] != 0.0f || persp[1] != 0.0f || persp[2] != 0.0f ||
        persp[3] != 1.0f) {
        float pm[16];
        for (int i = 0; i < 16; i++) pm[i] = mm[i];
        pm[3] = pm[7] = pm[11] = 0.0f;
        pm[15] = 1.0f;
        float inv[16];
        if (!mat4Inverse(pm, inv)) return false;
        // perspective = transpose(inverse(pm)) * persp
        for (int i = 0; i < 4; i++) {
            float acc = 0.0f;
            for (int j = 0; j < 4; j++) acc += inv[j * 4 + i] * persp[j];
            perspective[i] = acc;
        }
    }

    float row[3][3] = {
        { mm[0], mm[4], mm[8] },
        { mm[1], mm[5], mm[9] },
        { mm[2], mm[6], mm[10] },
    };
    float scale[3], skew[3];

    scale[0] = std::sqrt(row[0][0] * row[0][0] + row[0][1] * row[0][1] +
                         row[0][2] * row[0][2]);
    if (scale[0] < 1e-12f) return false;
    for (int i = 0; i < 3; i++) row[0][i] /= scale[0];

    skew[0] = row[0][0] * row[1][0] + row[0][1] * row[1][1] +
              row[0][2] * row[1][2];
    for (int i = 0; i < 3; i++) row[1][i] -= skew[0] * row[0][i];

    scale[1] = std::sqrt(row[1][0] * row[1][0] + row[1][1] * row[1][1] +
                         row[1][2] * row[1][2]);
    if (scale[1] < 1e-12f) return false;
    for (int i = 0; i < 3; i++) row[1][i] /= scale[1];
    skew[0] /= scale[1];

    skew[1] = row[0][0] * row[2][0] + row[0][1] * row[2][1] +
              row[0][2] * row[2][2];
    for (int i = 0; i < 3; i++) row[2][i] -= skew[1] * row[0][i];

    skew[2] = row[1][0] * row[2][0] + row[1][1] * row[2][1] +
              row[1][2] * row[2][2];
    for (int i = 0; i < 3; i++) row[2][i] -= skew[2] * row[1][i];

    scale[2] = std::sqrt(row[2][0] * row[2][0] + row[2][1] * row[2][1] +
                         row[2][2] * row[2][2]);
    if (scale[2] < 1e-12f) return false;
    for (int i = 0; i < 3; i++) row[2][i] /= scale[2];
    skew[1] /= scale[2];
    skew[2] /= scale[2];

    // Detect a coordinate-system flip (left-handed) and correct it.
    float pdum[3] = {
        row[1][1] * row[2][2] - row[1][2] * row[2][1],
        row[1][2] * row[2][0] - row[1][0] * row[2][2],
        row[1][0] * row[2][1] - row[1][1] * row[2][0],
    };
    if (row[0][0] * pdum[0] + row[0][1] * pdum[1] + row[0][2] * pdum[2] < 0.0f) {
        for (int i = 0; i < 3; i++) {
            scale[i] *= -1.0f;
            for (int j = 0; j < 3; j++) row[i][j] *= -1.0f;
        }
    }

    // Quaternion extraction (w = 0.5·sqrt(1 + trace) >= 0; signs chosen so
    // that recompose reproduces the matrix exactly).
    float q[4];
    q[0] = 0.5f * std::sqrt(std::max(1.0f + row[0][0] - row[1][1] - row[2][2], 0.0f));
    q[1] = 0.5f * std::sqrt(std::max(1.0f - row[0][0] + row[1][1] - row[2][2], 0.0f));
    q[2] = 0.5f * std::sqrt(std::max(1.0f - row[0][0] - row[1][1] + row[2][2], 0.0f));
    q[3] = 0.5f * std::sqrt(std::max(1.0f + row[0][0] + row[1][1] + row[2][2], 0.0f));
    // Sign rules: x ∝ R21 − R12, y ∝ R02 − R20, z ∝ R10 − R01 (w >= 0).
    if (row[2][1] - row[1][2] < 0.0f) q[0] = -q[0];
    if (row[0][2] - row[2][0] < 0.0f) q[1] = -q[1];
    if (row[1][0] - row[0][1] < 0.0f) q[2] = -q[2];

    for (int i = 0; i < 3; i++) {
        d.translate[i] = mm[12 + i];
        d.scale[i] = scale[i];
        d.skew[i] = skew[i];
    }
    for (int i = 0; i < 4; i++) {
        d.perspective[i] = perspective[i];
        d.quaternion[i] = q[i];
    }
    return true;
}

inline void mat4Recompose3D(const Mat4Decomposed& d, float out[16]) {
    mat4Identity(out);

    // Bottom row = perspective.
    out[3] = d.perspective[0];
    out[7] = d.perspective[1];
    out[11] = d.perspective[2];
    out[15] = d.perspective[3];

    // 4th column += translation (upper 3x3 is identity at this point).
    for (int i = 0; i < 3; i++) {
        float acc = 0.0f;
        for (int j = 0; j < 3; j++) acc += d.translate[j] * out[j * 4 + i];
        out[12 + i] += acc;
    }

    // Rotation from quaternion (x, y, z, w).
    float x = d.quaternion[0], y = d.quaternion[1];
    float z = d.quaternion[2], w = d.quaternion[3];
    float r[16];
    mat4Identity(r);
    r[0] = 1.0f - 2.0f * (y * y + z * z);
    r[4] = 2.0f * (x * y - z * w);
    r[8] = 2.0f * (x * z + y * w);
    r[1] = 2.0f * (x * y + z * w);
    r[5] = 1.0f - 2.0f * (x * x + z * z);
    r[9] = 2.0f * (y * z - x * w);
    r[2] = 2.0f * (x * z - y * w);
    r[6] = 2.0f * (y * z + x * w);
    r[10] = 1.0f - 2.0f * (x * x + y * y);
    float tmp[16];
    mat4Multiply(tmp, out, r);
    mat4Copy(out, tmp);

    // Shears (applied in reverse order: YZ, XZ, XY).
    if (d.skew[2] != 0.0f) {
        mat4Identity(r);
        r[6] = d.skew[2];
        mat4Multiply(tmp, out, r);
        mat4Copy(out, tmp);
    }
    if (d.skew[1] != 0.0f) {
        mat4Identity(r);
        r[2] = d.skew[1];
        mat4Multiply(tmp, out, r);
        mat4Copy(out, tmp);
    }
    if (d.skew[0] != 0.0f) {
        mat4Identity(r);
        r[1] = d.skew[0];
        mat4Multiply(tmp, out, r);
        mat4Copy(out, tmp);
    }

    // Scale columns of the 3x3 part.
    for (int i = 0; i < 3; i++) {
        for (int j = 0; j < 3; j++) out[i * 4 + j] *= d.scale[i];
    }
}

// ── Quaternion slerp ───────────────────────────────────────────
inline void mat4QuatSlerp(const float qa[4], const float qb[4], float t,
                          float out[4]) {
    float dot = qa[0] * qb[0] + qa[1] * qb[1] + qa[2] * qb[2] + qa[3] * qb[3];
    float q2[4] = { qb[0], qb[1], qb[2], qb[3] };
    if (dot < 0.0f) {
        dot = -dot;
        for (int i = 0; i < 4; i++) q2[i] = -q2[i];
    }
    if (dot > 0.9995f) {
        for (int i = 0; i < 4; i++) out[i] = qa[i] + t * (q2[i] - qa[i]);
    } else {
        float theta = std::acos(std::min(1.0f, std::max(-1.0f, dot)));
        float sinT = std::sin(theta);
        float wa = std::sin((1.0f - t) * theta) / sinT;
        float wb = std::sin(t * theta) / sinT;
        for (int i = 0; i < 4; i++) out[i] = wa * qa[i] + wb * q2[i];
    }
    float len = std::sqrt(out[0] * out[0] + out[1] * out[1] +
                          out[2] * out[2] + out[3] * out[3]);
    if (len > 1e-12f) {
        for (int i = 0; i < 4; i++) out[i] /= len;
    }
}

// Interpolate two matrices. If both are 2D, uses the 2D path; otherwise
// the full 3D decompose → interpolate → recompose (W3C §6.1.2).
inline void mat4Interpolate(const float a[16], const float b[16], float t,
                            float out[16]) {
    Mat4Decomposed2D d2a, d2b;
    if (mat4Decompose2D(a, d2a) && mat4Decompose2D(b, d2b)) {
        Mat4Decomposed2D r;
        for (int i = 0; i < 2; i++) {
            r.translate[i] = d2a.translate[i] + t * (d2b.translate[i] - d2a.translate[i]);
            r.scale[i] = d2a.scale[i] + t * (d2b.scale[i] - d2a.scale[i]);
        }
        r.angle = d2a.angle + t * (d2b.angle - d2a.angle);
        r.m11 = d2a.m11 + t * (d2b.m11 - d2a.m11);
        r.m12 = d2a.m12 + t * (d2b.m12 - d2a.m12);
        r.m21 = d2a.m21 + t * (d2b.m21 - d2a.m21);
        r.m22 = d2a.m22 + t * (d2b.m22 - d2a.m22);
        mat4Recompose2D(r, out);
        return;
    }

    Mat4Decomposed da, db;
    if (!mat4Decompose3D(a, da) || !mat4Decompose3D(b, db)) {
        mat4Copy(out, a);  // cannot decompose — stay on the source value
        return;
    }
    Mat4Decomposed r;
    for (int i = 0; i < 3; i++) {
        r.translate[i] = da.translate[i] + t * (db.translate[i] - da.translate[i]);
        r.scale[i] = da.scale[i] + t * (db.scale[i] - da.scale[i]);
        r.skew[i] = da.skew[i] + t * (db.skew[i] - da.skew[i]);
    }
    for (int i = 0; i < 4; i++) {
        r.perspective[i] = da.perspective[i] + t * (db.perspective[i] - da.perspective[i]);
    }
    mat4QuatSlerp(da.quaternion, db.quaternion, t, r.quaternion);
    mat4Recompose3D(r, out);
}

} // namespace morph