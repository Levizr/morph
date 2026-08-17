#pragma once
#include <string>
#include <vector>
#include <cctype>
#include <cmath>
#include <cstdlib>
#include "../../core/mat4.h"

#ifdef MORPH_FEATURE_TRANSFORM

// Style extension carrying the resolved transform matrix.  A transform that
// is `none` or absent leaves transformSet == false (identity).
struct TransformStyle {
    bool transformSet = false;
    float matrix[16] = {1, 0, 0, 0,
                        0, 1, 0, 0,
                        0, 0, 1, 0,
                        0, 0, 0, 1};
    // transform-origin as fractions of the element's own border box
    // (CSS default 50% 50% = center).  originSet is true when the style
    // explicitly set the property.
    bool originSet = false;
    float originX = 0.5f, originY = 0.5f;
};

namespace morph {

// ── Runtime CSS `transform` value parser ───────────────────────
// Mirrors morph/style/transforms.py: full function list, comma- or
// space-separated args, unitless angles accepted as degrees, unitless
// lengths accepted as px, `%` resolved against the element's own
// border-box size.  `none` and the global keywords reset the transform;
// invalid values are ignored (the style is left unchanged).

namespace detail {

inline void tmatIdentity(float m[16]) { mat4Identity(m); }

inline void tmatMultiply(float out[16], const float a[16], const float b[16]) {
    mat4Multiply(out, a, b);
}

inline bool tLength(const std::string& tok, float& value, bool& is_pct) {
    // Parse a length token: `<number>px`, `<number>%`, or bare `<number>` (px).
    const char* s = tok.c_str();
    char* end = nullptr;
    double v = std::strtod(s, &end);
    if (end == s) return false;
    std::string unit = end;
    if (unit.empty() || unit == "px") {
        value = (float)v;
        is_pct = false;
        return true;
    }
    if (unit == "%") {
        value = (float)v;
        is_pct = true;
        return true;
    }
    return false;
}

inline bool tAngle(const std::string& tok, float& deg) {
    const char* s = tok.c_str();
    char* end = nullptr;
    double v = std::strtod(s, &end);
    if (end == s) return false;
    std::string unit = end;
    if (unit.empty() || unit == "deg") {
        deg = (float)v;
        return true;
    }
    if (unit == "rad") { deg = (float)(v * 180.0 / 3.14159265358979323846); return true; }
    if (unit == "turn") { deg = (float)(v * 360.0); return true; }
    if (unit == "grad") { deg = (float)(v * 0.9); return true; }
    return false;
}

inline bool tNumber(const std::string& tok, float& v) {
    const char* s = tok.c_str();
    char* end = nullptr;
    double d = std::strtod(s, &end);
    if (end == s || *end != '\0') return false;
    v = (float)d;
    return true;
}

// Split a function's argument list on commas or whitespace (CSS allows both
// `translate(10px, 20px)` and `translate(10px 20px)`).
inline bool tSplitArgs(const std::string& inner,
                       std::vector<std::string>& out) {
    out.clear();
    if (inner.find(',') != std::string::npos) {
        size_t start = 0;
        while (start < inner.size()) {
            size_t comma = inner.find(',', start);
            std::string part = inner.substr(
                start, comma == std::string::npos ? std::string::npos
                                                  : comma - start);
            // trim
            size_t b = part.find_first_not_of(" \t\n\r");
            size_t e = part.find_last_not_of(" \t\n\r");
            if (b != std::string::npos) out.push_back(part.substr(b, e - b + 1));
            if (comma == std::string::npos) break;
            start = comma + 1;
        }
        return true;
    }
    size_t start = 0;
    while (start < inner.size()) {
        while (start < inner.size() &&
               (inner[start] == ' ' || inner[start] == '\t' ||
                inner[start] == '\n' || inner[start] == '\r'))
            start++;
        if (start >= inner.size()) break;
        size_t e = start;
        while (e < inner.size() &&
               inner[e] != ' ' && inner[e] != '\t' &&
               inner[e] != '\n' && inner[e] != '\r')
            e++;
        out.push_back(inner.substr(start, e - start));
        start = e;
    }
    return true;
}

// Build a single function's matrix.  Returns false if the function is
// invalid (the whole property must then be ignored).
inline bool tOpMatrix(const std::string& name,
                      const std::vector<std::string>& args,
                      float own_w, float own_h, float m[16]) {
    mat4Identity(m);
    auto need = [&](size_t n) { return args.size() == n; };

    if (name == "matrix") {
        if (!need(6)) return false;
        float v[6];
        for (int i = 0; i < 6; i++)
            if (!tNumber(args[i], v[i])) return false;
        mat4Identity(m);
        m[0] = v[0]; m[1] = v[1];
        m[4] = v[2]; m[5] = v[3];
        m[12] = v[4]; m[13] = v[5];
        return true;
    }
    if (name == "matrix3d") {
        if (!need(16)) return false;
        for (int i = 0; i < 16; i++)
            if (!tNumber(args[i], m[i])) return false;
        return true;
    }
    if (name == "perspective") {
        if (!need(1)) return false;
        float v; bool pct;
        if (!tLength(args[0], v, pct) || pct) return false;
        if (v <= 0.0f) { mat4Identity(m); return true; }
        m[11] = -1.0f / v;
        return true;
    }
    if (name == "rotate") {
        if (!need(1)) return false;
        float a; if (!tAngle(args[0], a)) return false;
        float r = a * 3.14159265358979323846f / 180.0f;
        float c = std::cos(r), s = std::sin(r);
        m[0] = c; m[1] = s; m[4] = -s; m[5] = c;
        return true;
    }
    if (name == "rotatex" || name == "rotatey" || name == "rotatez") {
        if (!need(1)) return false;
        float a; if (!tAngle(args[0], a)) return false;
        float r = a * 3.14159265358979323846f / 180.0f;
        float c = std::cos(r), s = std::sin(r);
        if (name == "rotatex") {
            m[5] = c; m[6] = s; m[9] = -s; m[10] = c;
        } else if (name == "rotatey") {
            m[0] = c; m[2] = -s; m[8] = s; m[10] = c;
        } else {
            m[0] = c; m[1] = s; m[4] = -s; m[5] = c;
        }
        return true;
    }
    if (name == "rotate3d") {
        if (!need(4)) return false;
        float axis[3], a;
        if (!tNumber(args[0], axis[0]) || !tNumber(args[1], axis[1]) ||
            !tNumber(args[2], axis[2]) || !tAngle(args[3], a))
            return false;
        float len = std::sqrt(axis[0] * axis[0] + axis[1] * axis[1] +
                              axis[2] * axis[2]);
        if (len < 1e-12f) { mat4Identity(m); return true; }
        float x = axis[0] / len, y = axis[1] / len, z = axis[2] / len;
        float r = a * 3.14159265358979323846f / 180.0f;
        float c = std::cos(r), s = std::sin(r), t = 1.0f - c;
        m[0] = t * x * x + c;     m[1] = t * x * y + s * z; m[2] = t * x * z - s * y;
        m[4] = t * x * y - s * z; m[5] = t * y * y + c;     m[6] = t * y * z + s * x;
        m[8] = t * x * z + s * y; m[9] = t * y * z - s * x; m[10] = t * z * z + c;
        return true;
    }
    if (name == "translate") {
        if (args.size() < 1 || args.size() > 2) return false;
        float tx, ty;
        bool px_, py_;
        if (!tLength(args[0], tx, px_)) return false;
        if (args.size() == 2) {
            if (!tLength(args[1], ty, py_)) return false;
        } else {
            ty = 0.0f; py_ = false;
        }
        m[12] = px_ ? tx / 100.0f * own_w : tx;
        m[13] = py_ ? ty / 100.0f * own_h : ty;
        return true;
    }
    if (name == "translate3d") {
        if (!need(3)) return false;
        float tx, ty, tz;
        bool px_, py_, pz_;
        if (!tLength(args[0], tx, px_) || !tLength(args[1], ty, py_) ||
            !tLength(args[2], tz, pz_) || pz_)
            return false;
        m[12] = px_ ? tx / 100.0f * own_w : tx;
        m[13] = py_ ? ty / 100.0f * own_h : ty;
        m[14] = tz;
        return true;
    }
    if (name == "translatex") {
        if (!need(1)) return false;
        float tx; bool pct;
        if (!tLength(args[0], tx, pct)) return false;
        m[12] = pct ? tx / 100.0f * own_w : tx;
        return true;
    }
    if (name == "translatey") {
        if (!need(1)) return false;
        float ty; bool pct;
        if (!tLength(args[0], ty, pct)) return false;
        m[13] = pct ? ty / 100.0f * own_h : ty;
        return true;
    }
    if (name == "translatez") {
        if (!need(1)) return false;
        float tz; bool pct;
        if (!tLength(args[0], tz, pct) || pct) return false;
        m[14] = tz;
        return true;
    }
    if (name == "scale") {
        if (args.size() < 1 || args.size() > 2) return false;
        float sx, sy;
        if (!tNumber(args[0], sx)) return false;
        sy = sx;
        if (args.size() == 2 && !tNumber(args[1], sy)) return false;
        m[0] = sx; m[5] = sy;
        return true;
    }
    if (name == "scale3d") {
        if (!need(3)) return false;
        float sx, sy, sz;
        if (!tNumber(args[0], sx) || !tNumber(args[1], sy) ||
            !tNumber(args[2], sz))
            return false;
        m[0] = sx; m[5] = sy; m[10] = sz;
        return true;
    }
    if (name == "scalex") {
        if (!need(1)) return false;
        float sx; if (!tNumber(args[0], sx)) return false;
        m[0] = sx;
        return true;
    }
    if (name == "scaley") {
        if (!need(1)) return false;
        float sy; if (!tNumber(args[0], sy)) return false;
        m[5] = sy;
        return true;
    }
    if (name == "scalez") {
        if (!need(1)) return false;
        float sz; if (!tNumber(args[0], sz)) return false;
        m[10] = sz;
        return true;
    }
    if (name == "skew") {
        if (args.size() < 1 || args.size() > 2) return false;
        float ax, ay;
        if (!tAngle(args[0], ax)) return false;
        ay = 0.0f;
        if (args.size() == 2 && !tAngle(args[1], ay)) return false;
        float tx_ = std::tan(ax * 3.14159265358979323846f / 180.0f);
        float ty_ = std::tan(ay * 3.14159265358979323846f / 180.0f);
        // skew(ax, ay) = skewX(ax) * skewY(ay)
        float sx[16], sy_[16], tmp[16];
        mat4Identity(sx); sx[4] = tx_;
        mat4Identity(sy_); sy_[1] = ty_;
        mat4Multiply(tmp, sx, sy_);
        mat4Copy(m, tmp);
        return true;
    }
    if (name == "skewx") {
        if (!need(1)) return false;
        float a; if (!tAngle(args[0], a)) return false;
        m[4] = std::tan(a * 3.14159265358979323846f / 180.0f);
        return true;
    }
    if (name == "skewy") {
        if (!need(1)) return false;
        float a; if (!tAngle(args[0], a)) return false;
        m[1] = std::tan(a * 3.14159265358979323846f / 180.0f);
        return true;
    }
    return false;  // unknown function
}

// ── Transform op-list parsing & interpolation ──────────────────
// Keyframe interpolation needs the *function list*, not just the composed
// matrix: rotate(0deg) → rotate(360deg) both decompose to the identity
// matrix, so matrix interpolation never rotates.  Browsers interpolate the
// op lists numerically (CSS Transforms §7); we do the same when the two
// sides have the same function structure, falling back to matrix
// interpolation otherwise.

struct TransformOp {
    std::string name;
    std::vector<float> args;  // units resolved: lengths→px, angles→deg
};

// Parse a CSS transform value into numeric ops (mirrors tOpMatrix but keeps
// the per-function scalars instead of composing).  Returns false when the
// value is invalid; `none`/global keywords yield an empty op list.
inline bool tParseTransformOps(const std::string& value, float own_w,
                               float own_h, std::vector<TransformOp>& ops) {
    ops.clear();
    size_t i = 0, n = value.size();
    while (i < n && (value[i] == ' ' || value[i] == '\t' ||
                     value[i] == '\n' || value[i] == '\r'))
        i++;
    if (i >= n) return false;
    size_t start = i;
    while (i < n && (isalnum((unsigned char)value[i]) || value[i] == '-')) i++;
    std::string first = value.substr(start, i - start);
    for (char& c : first) c = (char)tolower((unsigned char)c);
    if (first == "none" || first == "inherit" || first == "initial" ||
        first == "revert" || first == "revert-layer" || first == "unset")
        return true;
    // Rewind so the op loop below parses the first function (the token
    // above was only consumed for the keyword check).
    i = start;

    while (i < n) {
        while (i < n && (value[i] == ' ' || value[i] == '\t' ||
                         value[i] == '\n' || value[i] == '\r'))
            i++;
        if (i >= n) break;
        start = i;
        while (i < n && (isalnum((unsigned char)value[i]) || value[i] == '-')) i++;
        std::string name = value.substr(start, i - start);
        for (char& c : name) c = (char)tolower((unsigned char)c);
        while (i < n && (value[i] == ' ' || value[i] == '\t' ||
                         value[i] == '\n' || value[i] == '\r'))
            i++;
        if (i >= n || value[i] != '(') return false;
        int depth = 1;
        size_t j = i + 1;
        while (j < n && depth > 0) {
            if (value[j] == '(') depth++;
            else if (value[j] == ')') depth--;
            j++;
        }
        if (depth != 0) return false;
        std::string inner = value.substr(i + 1, j - i - 2);
        std::vector<std::string> args;
        tSplitArgs(inner, args);

        TransformOp op;
        op.name = name;
        auto need = [&](size_t k) { return args.size() == k; };
        auto num = [&](size_t k) -> bool {
            float v; if (!tNumber(args[k], v)) return false;
            op.args.push_back(v); return true;
        };
        auto ang = [&](size_t k) -> bool {
            float v; if (!tAngle(args[k], v)) return false;
            op.args.push_back(v); return true;
        };
        auto len = [&](size_t k, float own) -> bool {
            float v; bool pct;
            if (!tLength(args[k], v, pct)) return false;
            op.args.push_back(pct ? v / 100.0f * own : v);
            return true;
        };
        auto lenNoPct = [&](size_t k) -> bool {
            float v; bool pct;
            if (!tLength(args[k], v, pct) || pct) return false;
            op.args.push_back(v); return true;
        };

        if (name == "matrix") {
            if (!need(6)) return false;
            for (size_t k = 0; k < 6; k++) if (!num(k)) return false;
        } else if (name == "matrix3d") {
            if (!need(16)) return false;
            for (size_t k = 0; k < 16; k++) if (!num(k)) return false;
        } else if (name == "perspective") {
            if (!need(1) || !lenNoPct(0)) return false;
        } else if (name == "rotate" || name == "rotatex" ||
                   name == "rotatey" || name == "rotatez") {
            if (!need(1) || !ang(0)) return false;
        } else if (name == "rotate3d") {
            if (!need(4)) return false;
            if (!num(0) || !num(1) || !num(2) || !ang(3)) return false;
        } else if (name == "translate") {
            if (args.size() < 1 || args.size() > 2) return false;
            if (!len(0, own_w)) return false;
            if (args.size() == 2) { if (!len(1, own_h)) return false; }
            else op.args.push_back(0.0f);
        } else if (name == "translate3d") {
            if (!need(3)) return false;
            if (!len(0, own_w) || !len(1, own_h) || !lenNoPct(2)) return false;
        } else if (name == "scale") {
            if (args.size() < 1 || args.size() > 2) return false;
            if (!num(0)) return false;
            if (args.size() == 2) { if (!num(1)) return false; }
            else op.args.push_back(op.args[0]);
        } else if (name == "scale3d") {
            if (!need(3)) return false;
            for (size_t k = 0; k < 3; k++) if (!num(k)) return false;
        } else if (name == "skew") {
            if (args.size() < 1 || args.size() > 2) return false;
            if (!ang(0)) return false;
            if (args.size() == 2) { if (!ang(1)) return false; }
            else op.args.push_back(0.0f);
        } else if (name == "translatex") {
            if (!need(1) || !len(0, own_w)) return false;
            op.name = "translate";
            op.args.push_back(0.0f);
        } else if (name == "translatey") {
            if (!need(1)) return false;
            float v; bool pct;
            if (!tLength(args[0], v, pct)) return false;
            op.name = "translate";
            op.args.push_back(0.0f);
            op.args.push_back(pct ? v / 100.0f * own_h : v);
        } else if (name == "translatez") {
            if (!need(1) || !lenNoPct(0)) return false;
            op.name = "translate3d";
            op.args.insert(op.args.begin(), 0.0f);
            op.args.insert(op.args.begin(), 0.0f);
        } else if (name == "scalex") {
            if (!need(1) || !num(0)) return false;
            op.name = "scale";
            op.args.push_back(1.0f);
        } else if (name == "scaley") {
            if (!need(1)) return false;
            float v; if (!tNumber(args[0], v)) return false;
            op.name = "scale";
            op.args.push_back(1.0f);
            op.args.push_back(v);
        } else if (name == "scalez") {
            if (!need(1) || !num(0)) return false;
            op.name = "scale3d";
            op.args.insert(op.args.begin(), 1.0f);
            op.args.insert(op.args.begin(), 1.0f);
        } else if (name == "skewx") {
            if (!need(1) || !ang(0)) return false;
            op.name = "skew";
            op.args.push_back(0.0f);
        } else if (name == "skewy") {
            if (!need(1)) return false;
            float v; if (!tAngle(args[0], v)) return false;
            op.name = "skew";
            op.args.push_back(0.0f);
            op.args.push_back(v);
        } else {
            return false;  // unknown function
        }
        ops.push_back(std::move(op));
        i = j;
    }
    return true;
}

// Compose a numeric op list into a matrix (translate/scale normalized to
// fixed arity by tParseTransformOps).
inline bool tComposeTransformOps(const std::vector<TransformOp>& ops,
                                 float out[16]) {
    float acc[16];
    mat4Identity(acc);
    for (const auto& op : ops) {
        float opm[16];
        const float* v = op.args.data();
        size_t n = op.args.size();
        mat4Identity(opm);
        if (op.name == "matrix") {
            if (n != 6) return false;
            opm[0] = v[0]; opm[1] = v[1];
            opm[4] = v[2]; opm[5] = v[3];
            opm[12] = v[4]; opm[13] = v[5];
        } else if (op.name == "matrix3d") {
            if (n != 16) return false;
            for (size_t k = 0; k < 16; k++) opm[k] = v[k];
        } else if (op.name == "perspective") {
            if (n != 1) return false;
            if (v[0] > 0.0f) opm[11] = -1.0f / v[0];
        } else if (op.name == "rotate" || op.name == "rotatez") {
            if (n != 1) return false;
            float r = v[0] * 3.14159265358979323846f / 180.0f;
            float c = std::cos(r), s = std::sin(r);
            opm[0] = c; opm[1] = s; opm[4] = -s; opm[5] = c;
        } else if (op.name == "rotatex") {
            if (n != 1) return false;
            float r = v[0] * 3.14159265358979323846f / 180.0f;
            float c = std::cos(r), s = std::sin(r);
            opm[5] = c; opm[6] = s; opm[9] = -s; opm[10] = c;
        } else if (op.name == "rotatey") {
            if (n != 1) return false;
            float r = v[0] * 3.14159265358979323846f / 180.0f;
            float c = std::cos(r), s = std::sin(r);
            opm[0] = c; opm[2] = -s; opm[8] = s; opm[10] = c;
        } else if (op.name == "rotate3d") {
            if (n != 4) return false;
            float len = std::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
            if (len > 1e-12f) {
                float x = v[0] / len, y = v[1] / len, z = v[2] / len;
                float r = v[3] * 3.14159265358979323846f / 180.0f;
                float c = std::cos(r), s = std::sin(r), t = 1.0f - c;
                opm[0] = t * x * x + c; opm[1] = t * x * y + s * z; opm[2] = t * x * z - s * y;
                opm[4] = t * x * y - s * z; opm[5] = t * y * y + c; opm[6] = t * y * z + s * x;
                opm[8] = t * x * z + s * y; opm[9] = t * y * z - s * x; opm[10] = t * z * z + c;
            }
        } else if (op.name == "translate") {
            if (n != 2) return false;
            opm[12] = v[0]; opm[13] = v[1];
        } else if (op.name == "translate3d") {
            if (n != 3) return false;
            opm[12] = v[0]; opm[13] = v[1]; opm[14] = v[2];
        } else if (op.name == "scale") {
            if (n != 2) return false;
            opm[0] = v[0]; opm[5] = v[1];
        } else if (op.name == "scale3d") {
            if (n != 3) return false;
            opm[0] = v[0]; opm[5] = v[1]; opm[10] = v[2];
        } else if (op.name == "skew") {
            if (n != 2) return false;
            opm[4] = std::tan(v[0] * 3.14159265358979323846f / 180.0f);
            opm[1] = std::tan(v[1] * 3.14159265358979323846f / 180.0f);
        } else {
            return false;
        }
        float tmp[16];
        mat4Multiply(tmp, acc, opm);
        mat4Copy(acc, tmp);
    }
    mat4Copy(out, acc);
    return true;
}

// Interpolate two transform values.  When both sides parse to structurally
// identical function lists, interpolates the scalars numerically (correct
// rotation wraparound); otherwise falls back to matrix interpolation.
// Returns false only when either value is invalid.
inline bool tInterpolateTransformOps(const std::vector<TransformOp>& a,
                                     const std::vector<TransformOp>& b,
                                     float t, float out[16]) {
    bool match = a.size() == b.size();
    if (match) {
        for (size_t i = 0; i < a.size() && match; i++)
            match = a[i].name == b[i].name &&
                    a[i].args.size() == b[i].args.size();
    }
    if (match && !a.empty()) {
        std::vector<TransformOp> r = a;
        for (size_t i = 0; i < a.size(); i++)
            for (size_t k = 0; k < a[i].args.size(); k++)
                r[i].args[k] = a[i].args[k] +
                               (b[i].args[k] - a[i].args[k]) * t;
        return tComposeTransformOps(r, out);
    }
    // Structural mismatch → matrix interpolation fallback.
    float ma[16], mb[16];
    if (!tComposeTransformOps(a, ma) || !tComposeTransformOps(b, mb))
        return false;
    mat4Interpolate(ma, mb, t, out);
    return true;
}

} // namespace detail

} // namespace morph

#endif // MORPH_FEATURE_TRANSFORM