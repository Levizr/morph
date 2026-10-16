#pragma once

#ifdef MORPH_FEATURE_GRID
#include <string>
#include <vector>

#include "base.h"

// CSS Grid track model. Styles ride canonical CSS text (emitted by
// codegen, mirrored from `crates/morph-ir/src/grid.rs`); everything here
// parses that text at layout time, exactly like `flex-basis` strings.
// Unresolved lengths keep their unit and resolve against the container
// via UnitEnv during track sizing.

struct GridTrackSize
{
    enum Kind : uint8_t
    {
        Fixed,
        Flex,
        Auto,
        MinContent,
        MaxContent,
        FitContent,
        MinMax
    };
    Kind kind = Fixed;
    // Fixed px value or flex count (kind decides).
    CssLength len;
    float flex = 0.0f;
    // minmax() bounds. minKind ∈ {Fixed, Auto, MinContent, MaxContent};
    // maxKind adds Flex. Unused kinds stay Fixed/0.
    Kind minKind = Fixed;
    CssLength minLen;
    Kind maxKind = Fixed;
    CssLength maxLen;
    float maxFlex = 0.0f;
};

struct GridRepeat
{
    // Index in the track list where expansion inserts.
    int at = 0;
    // true = auto-fill, false = auto-fit (fixed counts expand in Rust).
    bool fill = true;
    std::vector<GridTrackSize> inner;
};

struct GridTrackList
{
    std::vector<GridTrackSize> tracks;
    // Line names: names[i] sits before track i, plus one trailing entry.
    std::vector<std::vector<std::string>> lineNames;
    std::vector<GridRepeat> repeats;
};

struct GridLine
{
    enum Kind : uint8_t
    {
        Auto,
        Line,
        Span,
        Named,
        SpanNamed
    };
    Kind kind = Auto;
    int line = 0;
    int span = 1;
    std::string name;
};

struct GridAreaRect
{
    std::string name;
    int r0 = 0, c0 = 0, r1 = 0, c1 = 0;
};

namespace morph
{

namespace grid_detail
{

inline void skipWs(const char*& p)
{
    while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r')
    {
        p++;
    }
}

// Length with CSS units for track sizes. Bare numbers are px (matches the
// Rust `Length::to_css` convention); `%` stays symbolic.
inline bool parseGridLength(const char*& p, CssLength& out)
{
    skipWs(p);
    char* end = nullptr;
    double v = std::strtod(p, &end);
    if (end == p)
    {
        return false;
    }
    p = end;
    skipWs(p);
    out.value = (float)v;
    out.unit = LengthUnit::Px;
    // Longest suffixes first.
    if ((p[0] == 'v' || p[0] == 'V') && (p[1] == 'm' || p[1] == 'M') &&
        (p[2] == 'i' || p[2] == 'I') && (p[3] == 'n' || p[3] == 'N'))
    {
        out.unit = LengthUnit::Vmin;
        p += 4;
    }
    else if ((p[0] == 'v' || p[0] == 'V') && (p[1] == 'm' || p[1] == 'M') &&
             (p[2] == 'a' || p[2] == 'A') && (p[3] == 'x' || p[3] == 'X'))
    {
        out.unit = LengthUnit::Vmax;
        p += 4;
    }
    else if ((p[0] == 'r' || p[0] == 'R') && (p[1] == 'e' || p[1] == 'E') &&
             (p[2] == 'm' || p[2] == 'M'))
    {
        out.unit = LengthUnit::Rem;
        p += 3;
    }
    else if ((p[0] == 'v' || p[0] == 'V') && (p[1] == 'h' || p[1] == 'H'))
    {
        out.unit = LengthUnit::Vh;
        p += 2;
    }
    else if ((p[0] == 'v' || p[0] == 'V') && (p[1] == 'w' || p[1] == 'W'))
    {
        out.unit = LengthUnit::Vw;
        p += 2;
    }
    else if ((p[0] == 'e' || p[0] == 'E') && (p[1] == 'm' || p[1] == 'M'))
    {
        out.unit = LengthUnit::Em;
        p += 2;
    }
    else if ((p[0] == 'e' || p[0] == 'E') && (p[1] == 'x' || p[1] == 'X'))
    {
        out.unit = LengthUnit::Ex;
        p += 2;
    }
    else if ((p[0] == 'c' || p[0] == 'C') && (p[1] == 'h' || p[1] == 'H'))
    {
        out.unit = LengthUnit::Ch;
        p += 2;
    }
    else if ((p[0] == 'p' || p[0] == 'P') && (p[1] == 'x' || p[1] == 'X'))
    {
        p += 2;
    }
    else if (p[0] == '%')
    {
        out.unit = LengthUnit::Pct;
        p += 1;
    }
    else if ((p[0] == 'i' || p[0] == 'I') && (p[1] == 'n' || p[1] == 'N'))
    {
        out.value *= 96.0f;
        p += 2;
    }
    else if ((p[0] == 'c' || p[0] == 'C') && (p[1] == 'm' || p[1] == 'M'))
    {
        out.value *= 96.0f / 2.54f;
        p += 2;
    }
    else if ((p[0] == 'm' || p[0] == 'M') && (p[1] == 'm' || p[1] == 'M'))
    {
        out.value *= 96.0f / 25.4f;
        p += 2;
    }
    else if ((p[0] == 'p' || p[0] == 'P') && (p[1] == 't' || p[1] == 'T'))
    {
        out.value *= 96.0f / 72.0f;
        p += 2;
    }
    else if ((p[0] == 'p' || p[0] == 'P') && (p[1] == 'c' || p[1] == 'C'))
    {
        out.value *= 16.0f;
        p += 2;
    }
    return true;
}

inline bool parseTrackSize(const char*& p, GridTrackSize& out);

// Forward: minmax/fit-content/repeat share the word reader.
inline bool readIdent(const char*& p, std::string& word)
{
    skipWs(p);
    const char* start = p;
    while ((*p >= 'a' && *p <= 'z') || (*p >= 'A' && *p <= 'Z') || (*p >= '0' && *p <= '9') ||
           *p == '-' || *p == '_' || *p == '.')
    {
        p++;
    }
    if (p == start)
    {
        return false;
    }
    word.assign(start, p);
    for (char& ch : word)
    {
        ch = (char)std::tolower((unsigned char)ch);
    }
    return true;
}

inline bool parseTrackMin(const char*& p, GridTrackSize::Kind& kind, CssLength& len)
{
    const char* save = p;
    std::string w;
    if (readIdent(p, w))
    {
        if (w == "auto")
        {
            kind = GridTrackSize::Auto;
            return true;
        }
        if (w == "min-content")
        {
            kind = GridTrackSize::MinContent;
            return true;
        }
        if (w == "max-content")
        {
            kind = GridTrackSize::MaxContent;
            return true;
        }
        p = save;
    }
    if (parseGridLength(p, len))
    {
        kind = GridTrackSize::Fixed;
        return true;
    }
    return false;
}

inline bool parseTrackMax(const char*& p, GridTrackSize::Kind& kind, CssLength& len, float& flex)
{
    const char* save = p;
    std::string w;
    if (readIdent(p, w))
    {
        if (w == "auto")
        {
            kind = GridTrackSize::Auto;
            return true;
        }
        if (w == "min-content")
        {
            kind = GridTrackSize::MinContent;
            return true;
        }
        if (w == "max-content")
        {
            kind = GridTrackSize::MaxContent;
            return true;
        }
        // `<n>fr` arrives as ident "fr"? No — numbers lex first. Handle
        // `fr` suffix after a number below; bare "fr" = 1fr.
        if (w == "fr")
        {
            kind = GridTrackSize::Flex;
            flex = 1.0f;
            return true;
        }
        p = save;
    }
    // Number with optional `fr` suffix.
    const char* numSave = p;
    char* end = nullptr;
    double v = std::strtod(p, &end);
    if (end != p)
    {
        const char* q = end;
        while (*q == ' ' || *q == '\t')
        {
            q++;
        }
        if ((q[0] == 'f' || q[0] == 'F') && (q[1] == 'r' || q[1] == 'R'))
        {
            if (v >= 0.0)
            {
                kind = GridTrackSize::Flex;
                flex = (float)v;
                p = q + 2;
                return true;
            }
            return false;
        }
        p = numSave;
    }
    if (parseGridLength(p, len))
    {
        kind = GridTrackSize::Fixed;
        return true;
    }
    return false;
}

inline bool parseTrackSize(const char*& p, GridTrackSize& out)
{
    skipWs(p);
    const char* save = p;
    std::string w;
    if (readIdent(p, w))
    {
        if (w == "auto")
        {
            out.kind = GridTrackSize::Auto;
            return true;
        }
        if (w == "min-content")
        {
            out.kind = GridTrackSize::MinContent;
            return true;
        }
        if (w == "max-content")
        {
            out.kind = GridTrackSize::MaxContent;
            return true;
        }
        if (w == "fr")
        {
            out.kind = GridTrackSize::Flex;
            out.flex = 1.0f;
            return true;
        }
        if (w == "minmax" || w == "fit-content")
        {
            bool isMinmax = (w == "minmax");
            skipWs(p);
            if (*p != '(')
            {
                return false;
            }
            p++;
            if (isMinmax)
            {
                out.kind = GridTrackSize::MinMax;
                if (!parseTrackMin(p, out.minKind, out.minLen))
                {
                    return false;
                }
                skipWs(p);
                if (*p != ',')
                {
                    return false;
                }
                p++;
                if (!parseTrackMax(p, out.maxKind, out.maxLen, out.maxFlex))
                {
                    return false;
                }
            }
            else
            {
                out.kind = GridTrackSize::FitContent;
                CssLength l;
                if (!parseGridLength(p, l))
                {
                    return false;
                }
                out.len = l;
            }
            skipWs(p);
            if (*p != ')')
            {
                return false;
            }
            p++;
            return true;
        }
        p = save;
    }
    // Number: `<n>fr` or a fixed length (bare = px).
    {
        const char* numSave = p;
        char* end = nullptr;
        double v = std::strtod(p, &end);
        if (end != p)
        {
            const char* q = end;
            while (*q == ' ' || *q == '\t')
            {
                q++;
            }
            if ((q[0] == 'f' || q[0] == 'F') && (q[1] == 'r' || q[1] == 'R'))
            {
                if (v < 0.0)
                {
                    return false;
                }
                out.kind = GridTrackSize::Flex;
                out.flex = (float)(v == 0.0 ? 0.0 : v);
                // `0fr` and bare `fr` both mean a zero... no: bare fr = 1,
                // `0fr` = 0. strtod never yields empty here.
                p = q + 2;
                return true;
            }
            p = numSave;
        }
    }
    CssLength l;
    if (parseGridLength(p, l))
    {
        out.kind = GridTrackSize::Fixed;
        out.len = l;
        return true;
    }
    return false;
}

} // namespace grid_detail

// Parse a canonical track list (`1fr [a] 200px`, possibly with a
// `repeat(auto-fill|auto-fit, ...)`). Returns false on garbage.
inline bool parseGridTrackList(const std::string& css, GridTrackList& out)
{
    out.tracks.clear();
    out.lineNames.clear();
    out.repeats.clear();
    out.lineNames.emplace_back();
    const char* p = css.c_str();
    while (true)
    {
        grid_detail::skipWs(p);
        if (*p == '\0')
        {
            break;
        }
        if (*p == '[')
        {
            p++;
            while (true)
            {
                grid_detail::skipWs(p);
                if (*p == ']')
                {
                    p++;
                    break;
                }
                std::string name;
                if (!grid_detail::readIdent(p, name))
                {
                    return false;
                }
                out.lineNames.back().push_back(name);
            }
            continue;
        }
        // repeat(...) — fixed counts expand eagerly, auto stays symbolic.
        {
            const char* save = p;
            std::string w;
            bool isRepeat = grid_detail::readIdent(p, w) && w == "repeat";
            // A bare number count lexes as an ident (digits allowed), so
            // `repeat(2, …)` arrives here with w == "repeat" intact.
            if (isRepeat)
            {
                grid_detail::skipWs(p);
                if (*p != '(')
                {
                    return false;
                }
                p++;
                std::string count;
                // Numeric counts arrive as ident text too ("2").
                {
                    const char* cs = p;
                    std::string cw;
                    if (grid_detail::readIdent(p, cw))
                    {
                        count = cw;
                    }
                    else
                    {
                        p = cs;
                    }
                }
                bool isFill = (count == "auto-fill");
                bool isFit = (count == "auto-fit");
                long fixed = -1;
                if (!isFill && !isFit)
                {
                    if (count.empty())
                    {
                        return false;
                    }
                    char* end = nullptr;
                    long v = std::strtol(count.c_str(), &end, 10);
                    if (*end != '\0' || v <= 0)
                    {
                        return false;
                    }
                    fixed = v;
                }
                grid_detail::skipWs(p);
                if (*p != ',')
                {
                    return false;
                }
                p++;
                std::vector<GridTrackSize> inner;
                std::vector<std::vector<std::string>> innerNames;
                innerNames.emplace_back();
                while (true)
                {
                    grid_detail::skipWs(p);
                    if (*p == ')')
                    {
                        p++;
                        break;
                    }
                    if (*p == '\0')
                    {
                        return false;
                    }
                    if (*p == '[')
                    {
                        // Names inside repeat() are dropped (Rust does the
                        // same; placement still resolves by index).
                        p++;
                        while (true)
                        {
                            grid_detail::skipWs(p);
                            if (*p == ']')
                            {
                                p++;
                                break;
                            }
                            std::string dummy;
                            if (!grid_detail::readIdent(p, dummy))
                            {
                                return false;
                            }
                        }
                        continue;
                    }
                    // Nested repeats are invalid.
                    {
                        const char* rs = p;
                        std::string rw;
                        if (grid_detail::readIdent(p, rw) && rw == "repeat")
                        {
                            return false;
                        }
                        p = rs;
                    }
                    GridTrackSize tr;
                    if (!grid_detail::parseTrackSize(p, tr))
                    {
                        return false;
                    }
                    inner.push_back(tr);
                    innerNames.emplace_back();
                }
                if (inner.empty())
                {
                    return false;
                }
                if (fixed > 0)
                {
                    for (long k = 0; k < fixed; k++)
                    {
                        for (auto& tr : inner)
                        {
                            out.tracks.push_back(tr);
                            out.lineNames.emplace_back();
                        }
                    }
                    continue;
                }
                GridRepeat rep;
                rep.at = (int)out.tracks.size();
                rep.fill = isFill;
                rep.inner = inner;
                out.repeats.push_back(rep);
                continue;
            }
            p = save;
        }
        GridTrackSize tr;
        if (!grid_detail::parseTrackSize(p, tr))
        {
            return false;
        }
        // `subgrid` never lexes as a size (no arm matches "subgrid"... it
        // would fail in parseTrackSize only if readIdent consumed it —
        // guard explicitly).
        out.tracks.push_back(tr);
        out.lineNames.emplace_back();
    }
    if (out.tracks.empty() && out.repeats.empty())
    {
        return false;
    }
    return true;
}

// Parse one placement value (`auto`, `3`, `span 2`, `name`, `span name`).
inline bool parseGridLine(const std::string& css, GridLine& out)
{
    const char* p = css.c_str();
    grid_detail::skipWs(p);
    if (*p == '\0')
    {
        return false;
    }
    // Optional `span` prefix.
    bool span = false;
    {
        const char* save = p;
        std::string w;
        if (grid_detail::readIdent(p, w) && w == "span")
        {
            span = true;
            grid_detail::skipWs(p);
            if (*p == '\0')
            {
                return false;
            }
        }
        else
        {
            p = save;
        }
    }
    // Number?
    {
        char* end = nullptr;
        long v = std::strtol(p, &end, 10);
        if (end != p)
        {
            const char* q = end;
            grid_detail::skipWs(q);
            if (*q != '\0')
            {
                return false;
            }
            if (v == 0)
            {
                return false;
            }
            if (span)
            {
                if (v < 1)
                {
                    return false;
                }
                out.kind = GridLine::Span;
                out.span = (int)v;
            }
            else
            {
                out.kind = GridLine::Line;
                out.line = (int)v;
            }
            p = end;
            return true;
        }
    }
    std::string name;
    if (!grid_detail::readIdent(p, name))
    {
        return false;
    }
    grid_detail::skipWs(p);
    if (*p != '\0')
    {
        return false;
    }
    if (span)
    {
        out.kind = GridLine::SpanNamed;
        out.name = name;
    }
    else
    {
        out.kind = GridLine::Named;
        out.name = name;
    }
    return true;
}

// Parse `"a a" "b b"` area rows into rectangles (validates rectangularity
// like the Rust side; callers pass trusted canonical text).
inline bool parseGridAreas(const std::string& css, std::vector<GridAreaRect>& out,
                           int& ncols, int& nrows)
{
    out.clear();
    ncols = nrows = 0;
    // Split quoted rows.
    std::vector<std::vector<std::string>> grid;
    const char* p = css.c_str();
    while (true)
    {
        grid_detail::skipWs(p);
        if (*p == '\0')
        {
            break;
        }
        if (*p != '"' && *p != '\'')
        {
            return false;
        }
        char quote = *p++;
        std::vector<std::string> row;
        std::string cur;
        while (*p != '\0' && *p != quote)
        {
            if (*p == ' ' || *p == '\t')
            {
                if (!cur.empty())
                {
                    row.push_back(cur);
                    cur.clear();
                }
            }
            else
            {
                cur.push_back(*p);
            }
            p++;
        }
        if (*p != quote)
        {
            return false;
        }
        p++;
        if (!cur.empty())
        {
            row.push_back(cur);
        }
        if (row.empty())
        {
            return false;
        }
        grid.push_back(row);
    }
    if (grid.empty())
    {
        return false;
    }
    ncols = (int)grid[0].size();
    nrows = (int)grid.size();
    for (auto& row : grid)
    {
        if ((int)row.size() != ncols)
        {
            return false;
        }
    }
    struct Bounds
    {
        int r0, c0, r1, c1;
    };
    std::vector<std::pair<std::string, Bounds>> order;
    auto find = [&](const std::string& n) -> Bounds* {
        for (auto& pr : order)
        {
            if (pr.first == n)
            {
                return &pr.second;
            }
        }
        return nullptr;
    };
    for (int r = 0; r < nrows; r++)
    {
        for (int c = 0; c < ncols; c++)
        {
            const std::string& name = grid[r][c];
            if (name == ".")
            {
                continue;
            }
            Bounds* b = find(name);
            if (!b)
            {
                order.push_back({name, {r, c, r + 1, c + 1}});
            }
            else
            {
                if (r < b->r0)
                {
                    b->r0 = r;
                }
                if (c < b->c0)
                {
                    b->c0 = c;
                }
                if (r + 1 > b->r1)
                {
                    b->r1 = r + 1;
                }
                if (c + 1 > b->c1)
                {
                    b->c1 = c + 1;
                }
            }
        }
    }
    for (auto& pr : order)
    {
        const Bounds& b = pr.second;
        for (int r = b.r0; r < b.r1; r++)
        {
            for (int c = b.c0; c < b.c1; c++)
            {
                if (grid[r][c] != pr.first)
                {
                    return false;
                }
            }
        }
        GridAreaRect a;
        a.name = pr.first;
        a.r0 = b.r0;
        a.c0 = b.c0;
        a.r1 = b.r1;
        a.c1 = b.c1;
        out.push_back(a);
    }
    return true;
}

} // namespace morph

#ifdef MORPH_FEATURE_GRID
// Grid container + item fields on MorphStyle. Track lists ride canonical
// CSS text (emitted by codegen); parsed at layout time, exactly like
// `flexBasis` strings. Unset tracks = empty string; placement
// `"auto / auto"` = auto-placed.
struct GridStyle {
    std::string gridTemplateColumns;
    std::string gridTemplateRows;
    std::string gridAutoColumns = "auto";
    std::string gridAutoRows = "auto";
    CSS::GridAutoFlow gridAutoFlow = CSS::GridAutoFlow::Row;
    std::string gridTemplateAreas;
    std::string gridColumn = "auto / auto";
    std::string gridRow = "auto / auto";
    std::string gridArea;
    CSS::AlignContent alignContent = CSS::AlignContent::Stretch;
    CSS::JustifyContent justifyItems = CSS::JustifyContent::Stretch;
    CSS::JustifySelf justifySelf = CSS::JustifySelf::Auto;
    CssLength rowGap = pxLen(0.0f);
    bool rowGapSet = false;
    CssLength columnGap = pxLen(0.0f);
    bool columnGapSet = false;
};
#endif

#endif
