#pragma once
// CSS Grid container layout (display:grid). Called from MorphNode::layout
// for grid containers; positions every in-flow child and reports the
// content extent via maxBottom/maxRight (container auto-sizing flows from
// those exactly like the flex/block branches).
//
// Covers: explicit tracks (px/fr/%/auto/minmax/content/fit-content),
// repeat() incl. auto-fill/fit, line placement (numbers, -1, spans,
// named lines), template areas, auto-flow row/column/dense, implicit
// tracks (grid-auto-*), gaps, content distribution and item alignment.
// Out of scope (documented): subgrid (rejected at parse), baseline
// alignment (maps to start), `order` (DOM order, like flex here).

#ifdef MORPH_FEATURE_GRID
#include <algorithm>
#include <cmath>
#include <string>
#include <vector>

#include "../style/features/grid.h"

namespace grid_layout
{

// ── Resolved tracks ──────────────────────────────────────────────

struct ResolvedTracks
{
    // Final px size per track (explicit + implicit).
    std::vector<float> sizes;
    // Start edge (content-box relative) per track.
    std::vector<float> starts;
    // Explicit line names (tracks.size()+1 entries, implicit lines empty).
    std::vector<std::vector<std::string>> lineNames;
    int explicitCount = 0;
};

// Resolve one track's base size (before fr distribution / content).
// Returns -1 when content measurement is required.
static float trackBaseSize(const GridTrackSize& t, const UnitEnv& env, float containerInner,
                           bool definite)
{
    using K = GridTrackSize::Kind;
    switch (t.kind)
    {
    case K::Fixed:
        if (t.len.unit == LengthUnit::Pct && !definite)
        {
            return -1.0f;
        }
        return resolveUnits(t.len, env);
    case K::Flex:
        return 0.0f;
    case K::Auto:
    case K::MinContent:
    case K::MaxContent:
        return -1.0f;
    case K::FitContent:
        return -1.0f; // min(max-content, max(min-content, limit)): needs content
    case K::MinMax:
        // Base = the minimum (floors fr distribution later).
        switch (t.minKind)
        {
        case K::Fixed:
            if (t.minLen.unit == LengthUnit::Pct && !definite)
            {
                return -1.0f;
            }
            return resolveUnits(t.minLen, env);
        case K::Auto:
        case K::MinContent:
        case K::MaxContent:
            return -1.0f;
        default:
            return 0.0f;
        }
    }
    return 0.0f;
}

// ── Items ─────────────────────────────────────────────────────────

struct GridItem
{
    MorphNode* node = nullptr;
    // Resolved placement: inclusive start / exclusive end, 0-based.
    // -1 = auto (to place).
    int c0 = -1, c1 = -1, r0 = -1, r1 = -1;
    bool colPlaced = false, rowPlaced = false;
    // Margins in px.
    float ml = 0, mr = 0, mt = 0, mb = 0;
    bool mlAuto = false, mrAuto = false, mtAuto = false, mbAuto = false;
    // Measured content sizes (max-content approximations).
    float contentW = 0, contentH = 0;
};

static int resolveLineNumber(int n, int explicitLines)
{
    // CSS grid §8.3: negative lines count back from the explicit end.
    if (n < 0)
    {
        return explicitLines + 1 + n;
    }
    return n;
}

// Named line lookup: index of the line carrying `name` (`occurrence`
// picks the nth; CSS `name 2` counts occurrences — v1 takes the first).
static int findNamedLine(const std::vector<std::vector<std::string>>& lines,
                         const std::string& name)
{
    for (size_t i = 0; i < lines.size(); i++)
    {
        for (auto& n : lines[i])
        {
            if (n == name)
            {
                return (int)i + 1;
            }
        }
    }
    return -1;
}

// ── Definite placement ───────────────────────────────────────────
// Resolve one axis (start/end GridLines + optional area span) to
// 0-based [s, e). Returns false when any axis stays auto.
static bool resolveAxis(const GridLine& start, const GridLine& end,
                        int areaS, int areaE, // -1 when no area
                        const std::vector<std::vector<std::string>>& lineNames,
                        int explicitLines, int& s, int& e)
{
    bool hasS = false, hasE = false;
    int vs = 0, ve = 0, span = 1;
    bool spanS = false, spanE = false;
    auto lineValue = [&](const GridLine& l, int& v, bool& isSpan, int& sp) -> bool {
        switch (l.kind)
        {
        case GridLine::Auto:
            return false;
        case GridLine::Line:
            v = resolveLineNumber(l.line, explicitLines);
            return true;
        case GridLine::Span:
            isSpan = true;
            sp = l.span > 0 ? l.span : 1;
            return true;
        case GridLine::Named:
        {
            int f = findNamedLine(lineNames, l.name);
            if (f < 0)
            {
                return false;
            }
            v = f;
            return true;
        }
        case GridLine::SpanNamed:
        {
            int f = findNamedLine(lineNames, l.name);
            if (f < 0)
            {
                return false;
            }
            isSpan = true;
            sp = 1;
            v = f;
            return true;
        }
        }
        return false;
    };
    hasS = lineValue(start, vs, spanS, span);
    hasE = lineValue(end, ve, spanE, span);
    if (!hasS && !hasE)
    {
        if (areaS >= 0)
        {
            s = areaS;
            e = areaE;
            return true;
        }
        return false;
    }
    if (hasS && !hasE)
    {
        if (spanS)
        {
            // `span n` alone: auto-placement with span (caller handles).
            return false;
        }
        if (areaS >= 0)
        {
            s = areaS;
            e = areaE;
            return true;
        }
        s = vs - 1;
        e = s + 1;
        return true;
    }
    if (!hasS && hasE)
    {
        if (spanE)
        {
            return false;
        }
        if (areaS >= 0)
        {
            s = areaS;
            e = areaE;
            return true;
        }
        e = ve - 1;
        s = e - 1;
        return true;
    }
    // Both present (values or spans).
    if (spanS && spanE)
    {
        return false; // span/span → auto (verified vs Chrome in tests)
    }
    if (spanS)
    {
        s = ve - 1 - span;
        e = ve - 1;
        return true;
    }
    if (spanE)
    {
        s = vs - 1;
        e = s + span;
        return true;
    }
    s = vs - 1;
    e = ve - 1;
    if (e <= s)
    {
        // Reversed/equal lines: span 1 from the start (Chrome parity
        // wave 1 verifies `3/2` and `2/2` explicitly).
        e = s + 1;
    }
    return true;
}

// ── Auto-placement ─────────────────────────────────────────────

struct PlacedGrid
{
    // Occupancy (rows × cols), grown on demand.
    std::vector<std::vector<char>> occ;
    int rows = 0, cols = 0;
    void ensure(int r, int c)
    {
        if (r > rows)
        {
            occ.resize(r, std::vector<char>(cols, 0));
            rows = r;
        }
        if (c > cols)
        {
            for (auto& row : occ)
            {
                row.resize(c, 0);
            }
            cols = c;
        }
    }
    bool fits(int r, int c, int rs, int cs) const
    {
        for (int i = r; i < r + rs; i++)
        {
            for (int j = c; j < c + cs; j++)
            {
                if (i < rows && j < cols && occ[i][j])
                {
                    return false;
                }
            }
        }
        return true;
    }
    void occupy(int r, int c, int rs, int cs)
    {
        ensure(r + rs, c + cs);
        for (int i = r; i < r + rs; i++)
        {
            for (int j = c; j < c + cs; j++)
            {
                occ[i][j] = 1;
            }
        }
    }
};

// ── Container entry ────────────────────────────────────────────
//
// Lays out every in-flow child of a grid container. `cw` is the definite
// content width; `ch`/`heightDefinite` describe the content height
// (indefinite for auto-height containers: `%` rows fall back to content,
// `fr` rows size to their content). Reports the content extent through
// maxBottom/maxRight so container auto-sizing flows like other branches.
static void layoutGridContainer(MorphNode* self, const std::vector<MorphNode*>& normal,
                                float cx, float cy, float cw, float ch, bool heightDefinite,
                                const UnitEnv& env, Renderer* r,
                                float& maxBottom, float& maxRight)
{
    const MorphStyle& style = self->style;
    // Gaps: explicit longhands win, else the `gap` shorthand.
    UnitEnv gapEnv = env;
    gapEnv.pctBase = cw; // % gaps resolve against the inline size, both axes
    float rowGap = style.rowGapSet ? resolveUnits(style.rowGap, gapEnv)
                                   : resolveUnits(style.gap, gapEnv);
    float colGap = style.columnGapSet ? resolveUnits(style.columnGap, gapEnv)
                                      : resolveUnits(style.gap, gapEnv);
    if (rowGap < 0.0f)
    {
        rowGap = 0.0f;
    }
    if (colGap < 0.0f)
    {
        colGap = 0.0f;
    }

    // Explicit templates (+ areas, which imply tracks).
    GridTrackList colList, rowList;
    bool hasCols = morph::parseGridTrackList(style.gridTemplateColumns, colList);
    bool hasRows = morph::parseGridTrackList(style.gridTemplateRows, rowList);
    if (!hasCols)
    {
        colList.tracks.clear();
        colList.lineNames.clear();
        colList.lineNames.emplace_back();
        colList.repeats.clear();
    }
    if (!hasRows)
    {
        rowList.tracks.clear();
        rowList.lineNames.clear();
        rowList.lineNames.emplace_back();
        rowList.repeats.clear();
    }
    std::vector<GridAreaRect> areas;
    int areaCols = 0, areaRows = 0;
    if (!style.gridTemplateAreas.empty())
    {
        if (!morph::parseGridAreas(style.gridTemplateAreas, areas, areaCols, areaRows))
        {
            areas.clear();
            areaCols = areaRows = 0;
        }
    }
    int explicitCols = (int)colList.tracks.size();
    int explicitRows = (int)rowList.tracks.size();
    if (areaCols > explicitCols)
    {
        explicitCols = areaCols;
    }
    if (areaRows > explicitRows)
    {
        explicitRows = areaRows;
    }

    // Gather items: measure content, resolve margins + placement strings.
    struct WorkItem : GridItem
    {
        GridLine csL, ceL, rsL, reL;
        std::string area;
        int spanC = 1, spanR = 1;
    };
    std::vector<WorkItem> items;
    for (auto* c : normal)
    {
        if (c->isWhitespaceOnly())
        {
            continue;
        }
        // Measure pass (mirrors the flex branch): provisional layout, then
        // intrinsic content size. Clear any previously injected stretch
        // sizes first so content measures true (author sizes untouched).
        if (c->m_gridStretchW)
        {
            c->style.explicitWidth.value = -1e9f;
            c->m_gridStretchW = false;
        }
        if (c->m_gridStretchH)
        {
            c->style.explicitHeight.value = -1e9f;
            c->m_gridStretchH = false;
        }
        c->layout(0.0f, 0.0f, cw > 0.0f ? cw : 0.0f, 0.0f, r);
        float cW = c->contentWidth(r);
        if (cW < 0.0f)
        {
            cW = 0.0f;
        }
        WorkItem it;
        it.node = c;
        it.contentW = cW;
        it.contentH = c->h > 0.0f ? c->h : 0.0f;
        UnitEnv itemEnv = c->unitEnv(cw > 0.0f ? cw : 0.0f, r);
        it.ml = resolveUnits(c->style.margin[3], itemEnv);
        it.mr = resolveUnits(c->style.margin[1], itemEnv);
        it.mt = resolveUnits(c->style.margin[0], itemEnv);
        it.mb = resolveUnits(c->style.margin[2], itemEnv);
        it.mlAuto = c->style.marginAuto[3];
        it.mrAuto = c->style.marginAuto[1];
        it.mtAuto = c->style.marginAuto[0];
        it.mbAuto = c->style.marginAuto[2];
        // Placement strings: "start / end" halves.
        auto splitPlacement = [](const std::string& s, GridLine& a, GridLine& b) {
            size_t slash = s.find('/');
            std::string l = (slash == std::string::npos) ? s : s.substr(0, slash);
            std::string rr = (slash == std::string::npos) ? "auto" : s.substr(slash + 1);
            auto trim = [](std::string t) {
                size_t p = 0;
                while (p < t.size() && (t[p] == ' ' || t[p] == '\t'))
                {
                    p++;
                }
                size_t q = t.size();
                while (q > p && (t[q - 1] == ' ' || t[q - 1] == '\t'))
                {
                    q--;
                }
                return t.substr(p, q - p);
            };
            l = trim(l);
            rr = trim(rr);
            if (l.empty())
            {
                l = "auto";
            }
            if (rr.empty())
            {
                rr = "auto";
            }
            GridLine la, lb;
            if (!morph::parseGridLine(l, la))
            {
                la.kind = GridLine::Auto;
            }
            if (!morph::parseGridLine(rr, lb))
            {
                lb.kind = GridLine::Auto;
            }
            a = la;
            b = lb;
        };
        splitPlacement(c->style.gridColumn, it.csL, it.ceL);
        splitPlacement(c->style.gridRow, it.rsL, it.reL);
        it.area = c->style.gridArea;
        items.push_back(it);
    }

    // auto-fill/fit expansion needs the inner width: expand repeats in the
    // column list (rows rarely repeat-auto; same helper both axes).
    auto expandRepeats = [&](GridTrackList& list, float inner, bool isRowAxis,
                             const std::vector<WorkItem>& its) {
        if (list.repeats.empty() || inner <= 0.0f)
        {
            return;
        }
        // Only auto-fill/fit survive Rust (fixed counts expand there).
        for (auto& rep : list.repeats)
        {
            // Repetition minimum: the largest minimum in the set (CSS
            // grid §7.2.3.2). Fixed/minmax-fixed minima are definite;
            // bare fr, auto and content minima use the largest item
            // contribution on this axis (bare-fr minima are automatic).
            float repMin = 0.0f;
            bool contentMin = false;
            for (auto& t : rep.inner)
            {
                using K = GridTrackSize::Kind;
                if (t.kind == K::Fixed)
                {
                    UnitEnv e2 = env;
                    e2.pctBase = inner;
                    float v = resolveUnits(t.len, e2);
                    if (v > repMin)
                    {
                        repMin = v;
                    }
                }
                else if (t.kind == K::MinMax)
                {
                    if (t.minKind == K::Fixed)
                    {
                        UnitEnv e2 = env;
                        e2.pctBase = inner;
                        float mn = resolveUnits(t.minLen, e2);
                        if (mn > repMin)
                        {
                            repMin = mn;
                        }
                    }
                    else
                    {
                        contentMin = true;
                    }
                }
                else
                {
                    // Auto/content/fit-content/bare-fr: content-sized.
                    contentMin = true;
                }
            }
            if (contentMin)
            {
                float cmax = 0.0f;
                for (auto& it : its)
                {
                    float v = isRowAxis ? it.contentH : it.contentW;
                    if (v > cmax)
                    {
                        cmax = v;
                    }
                }
                if (cmax > repMin)
                {
                    repMin = cmax;
                }
            }
            float gap = isRowAxis ? rowGap : colGap;
            int n = 1;
            if (repMin > 0.0f)
            {
                n = (int)std::floor((inner + gap) / (repMin + gap));
                if (n < 1)
                {
                    n = 1;
                }
            }
            else
            {
                // Zero-minimum set: one repetition (Chrome collapses
                // degenerate auto-fill to a single track).
                n = 1;
            }
            // Splice the repetitions into the track list.
            std::vector<GridTrackSize> expanded;
            expanded.reserve(list.tracks.size() + rep.inner.size() * (size_t)n);
            for (int i = 0; i < rep.at && i < (int)list.tracks.size(); i++)
            {
                expanded.push_back(list.tracks[i]);
            }
            for (int k = 0; k < n; k++)
            {
                for (auto& t : rep.inner)
                {
                    expanded.push_back(t);
                }
            }
            for (int i = rep.at; i < (int)list.tracks.size(); i++)
            {
                expanded.push_back(list.tracks[i]);
            }
            list.tracks.swap(expanded);
            // Line names shift: names live per line index; repeats carry
            // none (Rust drops them), so pad empties.
            std::vector<std::vector<std::string>> newNames;
            newNames.reserve(list.tracks.size() + 1);
            for (int i = 0; i < rep.at && i < (int)list.lineNames.size(); i++)
            {
                newNames.push_back(list.lineNames[i]);
            }
            for (int k = 0; k < n * (int)rep.inner.size(); k++)
            {
                newNames.emplace_back();
            }
            for (int i = rep.at; i < (int)list.lineNames.size(); i++)
            {
                newNames.push_back(list.lineNames[i]);
            }
            list.lineNames.swap(newNames);
        }
        list.repeats.clear();
        explicitCols = (int)colList.tracks.size();
        explicitRows = (int)rowList.tracks.size();
        if (areaCols > explicitCols)
        {
            explicitCols = areaCols;
        }
        if (areaRows > explicitRows)
        {
            explicitRows = areaRows;
        }
    };
    expandRepeats(colList, cw, false, items);
    expandRepeats(rowList, heightDefinite ? ch : 0.0f, true, items);

    // Area lookup for items naming one.
    auto findArea = [&](const std::string& name) -> const GridAreaRect* {
        for (auto& a : areas)
        {
            if (a.name == name)
            {
                return &a;
            }
        }
        return nullptr;
    };

    // Definite placement first (CSS grid §8.4), then auto-placement.
    int expColLines = explicitCols + 1;
    int expRowLines = explicitRows + 1;
    PlacedGrid grid;
    // Cursor state for the auto-placement pass.
    for (auto& it : items)
    {
        int as = -1, ae = -1, ar0 = -1, ar1 = -1;
        if (!it.area.empty())
        {
            if (const GridAreaRect* a = findArea(it.area))
            {
                as = a->c0;
                ae = a->c1;
                ar0 = a->r0;
                ar1 = a->r1;
            }
        }
        // Explicit lines override the area on their own axis.
        int s = -1, e = -1;
        bool colDef = resolveAxis(it.csL, it.ceL, -1, -1, colList.lineNames, expColLines, s, e);
        int rs = -1, re = -1;
        bool rowDef = resolveAxis(it.rsL, it.reL, -1, -1, rowList.lineNames, expRowLines, rs, re);
        // Span-only axes record their span for auto-placement.
        if (!colDef)
        {
            if (it.csL.kind == GridLine::Span)
            {
                it.spanC = it.csL.span;
            }
            else if (it.ceL.kind == GridLine::Span)
            {
                it.spanC = it.ceL.span;
            }
        }
        if (!rowDef)
        {
            if (it.rsL.kind == GridLine::Span)
            {
                it.spanR = it.rsL.span;
            }
            else if (it.reL.kind == GridLine::Span)
            {
                it.spanR = it.reL.span;
            }
        }
        if (as >= 0 && colDef)
        {
            // Explicit lines win over the area on this axis.
        }
        else if (as >= 0)
        {
            s = as;
            e = ae;
            colDef = true;
        }
        if (ar0 >= 0 && rowDef)
        {
        }
        else if (ar0 >= 0)
        {
            rs = ar0;
            re = ar1;
            rowDef = true;
        }
        if (colDef && rowDef)
        {
            if (s < 0)
            {
                s = 0;
            }
            if (rs < 0)
            {
                rs = 0;
            }
            it.c0 = s;
            it.c1 = e;
            it.r0 = rs;
            it.r1 = re;
            it.colPlaced = it.rowPlaced = true;
            grid.occupy(rs, s, re - rs, e - s);
        }
        else if (colDef)
        {
            it.c0 = s < 0 ? 0 : s;
            it.c1 = e;
            it.colPlaced = true;
        }
        else if (rowDef)
        {
            it.r0 = rs < 0 ? 0 : rs;
            it.r1 = re;
            it.rowPlaced = true;
        }
    }

    // Auto-placement (CSS grid §8.5): definite items already occupy;
    // the rest flow in DOM order (no `order` support, like flex here).
    // Row flow wraps auto items at the explicit column count (implicit
    // columns come only from definite placement); column flow mirrors.
    bool columnFlow = (style.gridAutoFlow == CSS::GridAutoFlow::Column ||
                       style.gridAutoFlow == CSS::GridAutoFlow::ColumnDense);
    bool dense = (style.gridAutoFlow == CSS::GridAutoFlow::RowDense ||
                  style.gridAutoFlow == CSS::GridAutoFlow::ColumnDense);
    int limC = explicitCols > 0 ? explicitCols : 1;
    int limR = explicitRows > 0 ? explicitRows : 1;
    int curR = 0, curC = 0;
    // First fitting cell for a both-auto item: bounded scan (dense from
    // the grid start, sparse from the cursor), then append past the end.
    auto placeAuto = [&](int rs, int cs, int& r, int& c) {
        if (!columnFlow)
        {
            // Row flow: rows grow, columns wrap at the explicit count
            // (auto items never create implicit columns).
            int sr = dense ? 0 : curR;
            int sc0 = dense ? 0 : curC;
            for (int rr = sr; rr < sr + 8192; rr++)
            {
                for (int cc = (rr == sr ? sc0 : 0);; cc++)
                {
                    if (cc + cs > limC)
                    {
                        break;
                    }
                    if (grid.fits(rr, cc, rs, cs))
                    {
                        r = rr;
                        c = cc;
                        return;
                    }
                }
            }
            r = grid.rows;
            c = 0;
        }
        else
        {
            // Column flow mirrored: columns grow, rows wrap.
            int sc = dense ? 0 : curC;
            int sr0 = dense ? 0 : curR;
            for (int cc = sc; cc < sc + 8192; cc++)
            {
                for (int rr = (cc == sc ? sr0 : 0);; rr++)
                {
                    if (rr + rs > limR)
                    {
                        break;
                    }
                    if (grid.fits(rr, cc, rs, cs))
                    {
                        r = rr;
                        c = cc;
                        return;
                    }
                }
            }
            r = 0;
            c = grid.cols;
        }
    };
    for (auto& it : items)
    {
        if (it.colPlaced && it.rowPlaced)
        {
            continue;
        }
        int rs = it.spanR, cs = it.spanC;
        if (it.colPlaced && !it.rowPlaced)
        {
            // Definite column, auto row: scan rows from the cursor
            // (dense from the start), growing implicit rows as needed.
            int r = dense ? 0 : curR, c = it.c0;
            while (!grid.fits(r, c, rs, it.c1 - it.c0))
            {
                r++;
            }
            it.r0 = r;
            it.r1 = r + rs;
            it.rowPlaced = true;
            grid.occupy(r, c, rs, it.c1 - it.c0);
            if (!dense)
            {
                curR = r;
            }
        }
        else if (!it.colPlaced && it.rowPlaced)
        {
            // Definite row, auto column: scan columns (may extend implicit
            // columns when the row is full).
            int r = it.r0, c = (!dense && r == curR) ? curC : 0;
            while (!grid.fits(r, c, it.r1 - it.r0, cs))
            {
                c++;
            }
            it.c0 = c;
            it.c1 = c + cs;
            it.colPlaced = true;
            grid.occupy(r, c, it.r1 - it.r0, cs);
            if (!dense && r == curR)
            {
                curC = c;
            }
        }
        else
        {
            int r = 0, c = 0;
            placeAuto(rs, cs, r, c);
            it.r0 = r;
            it.c0 = c;
            it.r1 = r + rs;
            it.c1 = c + cs;
            it.colPlaced = it.rowPlaced = true;
            grid.occupy(r, c, rs, cs);
            if (!dense)
            {
                if (!columnFlow)
                {
                    curR = r;
                    curC = c + cs;
                }
                else
                {
                    curC = c;
                    curR = r + rs;
                }
            }
        }
    }
    int nCols = grid.cols;
    int nRows = grid.rows;
    // Negative lines resolving past the explicit start (e.g. `-10`)
    // create implicit tracks before line 1: shift everything right/down.
    {
        int minC = 0, minR = 0;
        for (auto& it : items)
        {
            if (it.c0 < minC)
            {
                minC = it.c0;
            }
            if (it.r0 < minR)
            {
                minR = it.r0;
            }
        }
        if (minC < 0 || minR < 0)
        {
            for (auto& it : items)
            {
                it.c0 -= minC;
                it.c1 -= minC;
                it.r0 -= minR;
                it.r1 -= minR;
            }
            nCols = 0;
            nRows = 0;
            for (auto& it : items)
            {
                if (it.c1 > nCols)
                {
                    nCols = it.c1;
                }
                if (it.r1 > nRows)
                {
                    nRows = it.r1;
                }
            }
        }
    }
    if (nCols < explicitCols)
    {
        nCols = explicitCols;
    }
    if (nRows < explicitRows)
    {
        nRows = explicitRows;
    }
    if (nCols < 1)
    {
        nCols = 1;
    }
    if (nRows < 1)
    {
        nRows = 1;
    }

    // ── Track sizing ──
    // Implicit track sizes cycle the auto lists (default [auto]).
    GridTrackList impCols, impRows;
    if (!morph::parseGridTrackList(style.gridAutoColumns.empty() ? "auto" : style.gridAutoColumns, impCols))
    {
        impCols.tracks.clear();
    }
    if (!morph::parseGridTrackList(style.gridAutoRows.empty() ? "auto" : style.gridAutoRows, impRows))
    {
        impRows.tracks.clear();
    }
    auto trackAt = [&](const GridTrackList& expl, const GridTrackList& imp, int i) -> GridTrackSize {
        if (i < (int)expl.tracks.size())
        {
            return expl.tracks[i];
        }
        if (!imp.tracks.empty())
        {
            return imp.tracks[(i - expl.tracks.size()) % imp.tracks.size()];
        }
        GridTrackSize t;
        t.kind = GridTrackSize::Auto;
        return t;
    };

    // Content contributions per item (max-content approximations).
    // Columns: contentW; rows: measured height at final column width
    // (two-pass below). min-content ≈ max-content (documented; exact for
    // non-wrapping content).
    std::vector<float> colSizes(nCols, -1.0f), rowSizes(nRows, -1.0f);
    UnitEnv trackEnv = env;
    trackEnv.pctBase = cw;
    bool widthDefinite = cw > 0.0f;
    for (int i = 0; i < nCols; i++)
    {
        GridTrackSize t = trackAt(colList, impCols, i);
        float b = trackBaseSize(t, trackEnv, cw, widthDefinite);
        colSizes[i] = b;
    }
    UnitEnv rowEnv = env;
    rowEnv.pctBase = heightDefinite ? ch : 0.0f;
    for (int i = 0; i < nRows; i++)
    {
        GridTrackSize t = trackAt(rowList, impRows, i);
        float b = trackBaseSize(t, rowEnv, ch, heightDefinite);
        rowSizes[i] = b;
    }
    // Content pass: single-span items size their track; spanning items
    // grow afterwards (CSS grid §12.4 distribute). Columns resolve fully
    // first so row contributions measure at final column widths.
    auto contentSizedTrack = [&](const GridTrackSize& t, bool isCol) -> bool {
        if (t.kind == GridTrackSize::Auto || t.kind == GridTrackSize::MinContent ||
            t.kind == GridTrackSize::MaxContent || t.kind == GridTrackSize::FitContent ||
            (t.kind == GridTrackSize::MinMax &&
             (t.minKind == GridTrackSize::Auto || t.minKind == GridTrackSize::MinContent ||
              t.minKind == GridTrackSize::MaxContent)))
        {
            return true;
        }
        // Indefinite-height fr rows size to content.
        if (!isCol && !heightDefinite && t.kind == GridTrackSize::Flex)
        {
            return true;
        }
        return false;
    };
    auto sizeAxis = [&](bool isCol) {
        const GridTrackList& expl = isCol ? colList : rowList;
        const GridTrackList& imp = isCol ? impCols : impRows;
        const UnitEnv& tenv = isCol ? trackEnv : rowEnv;
        float gap = isCol ? colGap : rowGap;
        int n = isCol ? nCols : nRows;
        std::vector<float>& sizes = isCol ? colSizes : rowSizes;
        for (int pass = 0; pass < 2; pass++)
        {
            for (auto& it : items)
            {
                int s = isCol ? it.c0 : it.r0;
                int e = isCol ? it.c1 : it.r1;
                if (s < 0 || e <= s)
                {
                    continue;
                }
                float contrib = isCol ? it.contentW : it.contentH;
                // Margins eat into the area, not the track — tracks size
                // to border-box contributions; margins handled at place.
                if (pass == 0 && e - s == 1)
                {
                    if (sizes[s] < 0.0f || contrib > sizes[s])
                    {
                        // Only grow content-sized tracks (fixed/%/fr keep
                        // their base; minmax max clamps below).
                        GridTrackSize t = trackAt(expl, imp, s);
                        if (contentSizedTrack(t, isCol))
                        {
                            float v = contrib;
                            if (t.kind == GridTrackSize::FitContent)
                            {
                                float lim = resolveUnits(t.len, tenv);
                                if (v > lim)
                                {
                                    v = lim;
                                }
                            }
                            if (t.kind == GridTrackSize::MinMax && t.maxKind == GridTrackSize::Fixed)
                            {
                                float mx = resolveUnits(t.maxLen, tenv);
                                if (v > mx)
                                {
                                    v = mx;
                                }
                            }
                            sizes[s] = v;
                        }
                    }
                }
                if (pass == 1 && e - s > 1)
                {
                    float have = 0.0f;
                    for (int k = s; k < e; k++)
                    {
                        have += sizes[k] >= 0.0f ? sizes[k] : 0.0f;
                    }
                    float gaps = gap * (float)(e - s - 1);
                    if (have + gaps < contrib)
                    {
                        float extra = (contrib - have - gaps) / (float)(e - s);
                        for (int k = s; k < e; k++)
                        {
                            if (sizes[k] < 0.0f)
                            {
                                sizes[k] = 0.0f;
                            }
                            sizes[k] += extra;
                        }
                    }
                }
            }
        }
    };
    // fr distribution (columns always; rows only when definite — else
    // content-sized above, matching Chrome).
    // fr distribution (columns always; rows only when definite — else
    // content-sized above, matching Chrome).
    auto distributeFr = [&](std::vector<float>& sizes, int n,
                            const GridTrackList& expl, const GridTrackList& imp,
                            const UnitEnv& tenv, float inner, float gap) {
        auto isFrTrack = [&](int i, float& f, float& floor) -> bool {
            GridTrackSize t = trackAt(expl, imp, i);
            bool isFr = (t.kind == GridTrackSize::Flex) ||
                        (t.kind == GridTrackSize::MinMax && t.maxKind == GridTrackSize::Flex);
            if (!isFr)
            {
                return false;
            }
            f = (t.kind == GridTrackSize::Flex) ? t.flex : t.maxFlex;
            floor = 0.0f;
            if (t.kind == GridTrackSize::MinMax)
            {
                floor = (t.minKind == GridTrackSize::Fixed) ? resolveUnits(t.minLen, tenv)
                                                            : sizes[i];
            }
            return true;
        };
        float used = 0.0f;
        for (int i = 0; i < n; i++)
        {
            float f = 0.0f, floor = 0.0f;
            if (!isFrTrack(i, f, floor))
            {
                used += sizes[i];
            }
        }
        if (n > 1)
        {
            used += gap * (float)(n - 1);
        }
        float free = inner - used;
        if (free <= 0.0f)
        {
            return;
        }
        // CSS grid §12.7: freeze tracks that would drop below their
        // minimum, then share the remainder (repeat until stable).
        std::vector<char> frozen(n, 0);
        for (int round = 0; round < n + 1; round++)
        {
            float flex = 0.0f;
            for (int i = 0; i < n; i++)
            {
                float f = 0.0f, floor = 0.0f;
                if (!frozen[i] && isFrTrack(i, f, floor))
                {
                    flex += f;
                }
            }
            if (flex <= 0.0f)
            {
                return;
            }
            float share = free / flex;
            bool froze = false;
            for (int i = 0; i < n; i++)
            {
                float f = 0.0f, floor = 0.0f;
                if (frozen[i] || !isFrTrack(i, f, floor))
                {
                    continue;
                }
                if (share * f < floor)
                {
                    sizes[i] = floor;
                    free -= floor;
                    frozen[i] = 1;
                    froze = true;
                }
            }
            if (!froze)
            {
                for (int i = 0; i < n; i++)
                {
                    float f = 0.0f, floor = 0.0f;
                    if (frozen[i] || !isFrTrack(i, f, floor))
                    {
                        continue;
                    }
                    float want = share * f;
                    sizes[i] = want < floor ? floor : want;
                }
                return;
            }
        }
    };
    sizeAxis(true);
    for (int i = 0; i < nCols; i++)
    {
        if (colSizes[i] < 0.0f)
        {
            colSizes[i] = 0.0f;
        }
    }
    distributeFr(colSizes, nCols, colList, impCols, trackEnv, cw, colGap);
    // Column widths are final: re-measure row contributions at their
    // real constraint (text wraps to the column, changing heights).
    {
        for (auto& it : items)
        {
            float w = 0.0f;
            for (int k = it.c0; k < it.c1 && k < nCols; k++)
            {
                w += colSizes[k];
            }
            if (it.c1 - it.c0 > 1)
            {
                w += colGap * (float)(it.c1 - it.c0 - 1);
            }
            w -= it.ml + it.mr;
            if (w < 0.0f)
            {
                w = 0.0f;
            }
            // Definite heights don't need measuring.
            if (!it.node->style.explicitHeight.isSet())
            {
                it.node->layout(0.0f, 0.0f, w, heightDefinite ? ch : 0.0f, r);
                it.contentH = it.node->h > 0.0f ? it.node->h : 0.0f;
            }
        }
    }
    sizeAxis(false);
    for (int i = 0; i < nRows; i++)
    {
        if (rowSizes[i] < 0.0f)
        {
            rowSizes[i] = 0.0f;
        }
    }
    if (heightDefinite)
    {
        distributeFr(rowSizes, nRows, rowList, impRows, rowEnv, ch, rowGap);
    }
    // Track starts (content-box relative) + content distribution.
    std::vector<float> colStarts(nCols, 0.0f), rowStarts(nRows, 0.0f);
    {
        float acc = 0.0f;
        for (int i = 0; i < nCols; i++)
        {
            colStarts[i] = acc;
            acc += colSizes[i] + colGap;
        }
    }
    float rowsTotal = 0.0f;
    for (int i = 0; i < nRows; i++)
    {
        rowsTotal += rowSizes[i];
    }
    if (nRows > 1)
    {
        rowsTotal += rowGap * (float)(nRows - 1);
    }
    // Align-content/justify-content: free space distribution. `stretch`
    // grows auto-sized tracks (verified vs Chrome); fixed-only grids
    // pack like `start`.
    auto distributeContent = [&](std::vector<float>& sizes, std::vector<float>& starts, int n,
                                 const GridTrackList& expl, const GridTrackList& imp,
                                 float inner, float gap, int contentMode /*0 justify,1 align*/,
                                 float& leading) {
        float total = 0.0f;
        for (int i = 0; i < n; i++)
        {
            total += sizes[i];
        }
        if (n > 1)
        {
            total += gap * (float)(n - 1);
        }
        float free = inner - total;
        leading = 0.0f;
        if (free <= 0.0f)
        {
            return free;
        }
        // Resolve the effective alignment.
        int mode = 0; // 0 start, 1 center, 2 end, 3 space-between, 4 space-around, 5 space-evenly, 6 stretch
        if (contentMode == 0)
        {
            switch (style.justifyContent)
            {
            case CSS::JustifyContent::Center:
                mode = 1;
                break;
            case CSS::JustifyContent::FlexEnd:
            case CSS::JustifyContent::End:
                mode = 2;
                break;
            case CSS::JustifyContent::SpaceBetween:
                mode = 3;
                break;
            case CSS::JustifyContent::SpaceAround:
                mode = 4;
                break;
            case CSS::JustifyContent::SpaceEvenly:
                mode = 5;
                break;
            case CSS::JustifyContent::Stretch:
            {
                // Grow auto-sized tracks; fixed-only packs start.
                int autos = 0;
                for (int i = 0; i < n; i++)
                {
                    GridTrackSize t = trackAt(expl, imp, i);
                    if (t.kind == GridTrackSize::Auto)
                    {
                        autos++;
                    }
                }
                mode = (autos > 0) ? 6 : 0;
                break;
            }
            default:
                mode = 0;
                break;
            }
        }
        else
        {
            switch (style.alignContent)
            {
            case CSS::AlignContent::Center:
                mode = 1;
                break;
            case CSS::AlignContent::End:
                mode = 2;
                break;
            case CSS::AlignContent::SpaceBetween:
                mode = 3;
                break;
            case CSS::AlignContent::SpaceAround:
                mode = 4;
                break;
            case CSS::AlignContent::SpaceEvenly:
                mode = 5;
                break;
            case CSS::AlignContent::Stretch:
            {
                int autos = 0;
                for (int i = 0; i < n; i++)
                {
                    GridTrackSize t = trackAt(expl, imp, i);
                    if (t.kind == GridTrackSize::Auto)
                    {
                        autos++;
                    }
                }
                mode = (autos > 0) ? 6 : 0;
                break;
            }
            default:
                mode = 0;
                break;
            }
        }
        if (mode == 1)
        {
            leading = free * 0.5f;
        }
        else if (mode == 2)
        {
            leading = free;
        }
        else if (mode == 6)
        {
            int autos = 0;
            for (int i = 0; i < n; i++)
            {
                GridTrackSize t = trackAt(expl, imp, i);
                if (t.kind == GridTrackSize::Auto)
                {
                    autos++;
                }
            }
            if (autos > 0)
            {
                float add = free / (float)autos;
                for (int i = 0; i < n; i++)
                {
                    GridTrackSize t = trackAt(expl, imp, i);
                    if (t.kind == GridTrackSize::Auto)
                    {
                        sizes[i] += add;
                    }
                }
            }
        }
        // Recompute starts (space-* adjust gaps, handled at place time
        // via effective gap — return free for the caller).
        float acc = leading;
        for (int i = 0; i < n; i++)
        {
            starts[i] = acc;
            acc += sizes[i] + gap;
        }
        return free;
    };
    float leadX = 0.0f, leadY = 0.0f;
    float freeX = distributeContent(colSizes, colStarts, nCols, colList, impCols, cw, colGap, 0, leadX);
    float freeY = 0.0f;
    if (heightDefinite)
    {
        freeY = distributeContent(rowSizes, rowStarts, nRows, rowList, impRows, ch, rowGap, 1, leadY);
    }
    else
    {
        float acc = 0.0f;
        for (int i = 0; i < nRows; i++)
        {
            rowStarts[i] = acc;
            acc += rowSizes[i] + rowGap;
        }
    }
    (void)freeX;
    (void)freeY;
    (void)leadX;
    (void)leadY;

    // ── Place items ──
    for (auto& it : items)
    {
        MorphNode* c = it.node;
        if (it.c0 < 0 || it.r0 < 0)
        {
            continue; // cannot happen (auto-placement fills all)
        }
        float ax = cx + colStarts[it.c0];
        float aw = 0.0f;
        for (int k = it.c0; k < it.c1 && k < nCols; k++)
        {
            aw += colSizes[k];
        }
        if (it.c1 - it.c0 > 1)
        {
            aw += colGap * (float)(it.c1 - it.c0 - 1);
        }
        float ay = cy + rowStarts[it.r0];
        float ah = 0.0f;
        for (int k = it.r0; k < it.r1 && k < nRows; k++)
        {
            ah += rowSizes[k];
        }
        if (it.r1 - it.r0 > 1)
        {
            ah += rowGap * (float)(it.r1 - it.r0 - 1);
        }
        // Margins shrink the area; auto margins absorb free space.
        float ml = it.mlAuto ? 0.0f : it.ml;
        float mr = it.mrAuto ? 0.0f : it.mr;
        float mt = it.mtAuto ? 0.0f : it.mt;
        float mb = it.mbAuto ? 0.0f : it.mb;
        // Self alignment (justify = inline/column axis).
        CSS::JustifyContent ji = style.justifyItems;
        int jv = 0; // 0 stretch, 1 start, 2 center, 3 end
        if (c->style.justifySelf != CSS::JustifySelf::Auto)
        {
            switch (c->style.justifySelf)
            {
            case CSS::JustifySelf::Start:
                jv = 1;
                break;
            case CSS::JustifySelf::Center:
                jv = 2;
                break;
            case CSS::JustifySelf::End:
                jv = 3;
                break;
            default:
                jv = 0;
                break;
            }
        }
        else
        {
            switch (ji)
            {
            case CSS::JustifyContent::Start:
                jv = 1;
                break;
            case CSS::JustifyContent::Center:
                jv = 2;
                break;
            case CSS::JustifyContent::End:
                jv = 3;
                break;
            default:
                jv = 0;
                break;
            }
        }
        int av = 0;
        CSS::AlignItems ai = style.alignItems;
        if (c->style.alignSelf != CSS::AlignSelf::Auto)
        {
            switch (c->style.alignSelf)
            {
            case CSS::AlignSelf::FlexStart:
                av = 1;
                break;
            case CSS::AlignSelf::Center:
                av = 2;
                break;
            case CSS::AlignSelf::FlexEnd:
                av = 3;
                break;
            default:
                av = 0;
                break;
            }
        }
        else
        {
            switch (ai)
            {
            case CSS::AlignItems::FlexStart:
                av = 1;
                break;
            case CSS::AlignItems::Center:
                av = 2;
                break;
            case CSS::AlignItems::FlexEnd:
                av = 3;
                break;
            default:
                av = 0;
                break;
            }
        }
        float iw = aw - ml - mr;
        float ih = ah - mt - mb;
        if (iw < 0.0f)
        {
            iw = 0.0f;
        }
        if (ih < 0.0f)
        {
            ih = 0.0f;
        }
        bool widthSet = c->style.explicitWidth.isSet();
        bool heightSet = c->style.explicitHeight.isSet();
        float fw = iw, fh = ih;
        if (jv != 0 || widthSet)
        {
            // Non-stretch: shrink-to-fit content, wrapping at the area.
            // Definite sizes are honored exactly (they overflow a
            // smaller area, verified vs Chrome).
            if (widthSet)
            {
                // Explicit widths were honored in the measure pass.
                fw = c->w;
            }
            else
            {
                fw = it.contentW;
                if (fw > iw)
                {
                    fw = iw;
                }
            }
        }
        if (av != 0 || heightSet)
        {
            if (heightSet)
            {
                fh = c->h;
            }
            else
            {
                fh = it.contentH;
                if (fh > ih)
                {
                    fh = ih;
                }
            }
        }
        float fx = ax + ml;
        float fy = ay + mt;
        if (jv == 2)
        {
            fx += (iw - fw) * 0.5f;
        }
        else if (jv == 3)
        {
            fx += iw - fw;
        }
        if (av == 2)
        {
            fy += (ih - fh) * 0.5f;
        }
        else if (av == 3)
        {
            fy += ih - fh;
        }
        // Auto margins absorb leftover (both → center, one → far side).
        if (it.mlAuto || it.mrAuto)
        {
            float free = iw - fw;
            if (free < 0.0f)
            {
                free = 0.0f;
            }
            if (it.mlAuto && it.mrAuto)
            {
                fx = ax + ml + free * 0.5f;
            }
            else if (it.mlAuto)
            {
                fx = ax + ml + free;
            }
        }
        if (it.mtAuto || it.mbAuto)
        {
            float free = ih - fh;
            if (free < 0.0f)
            {
                free = 0.0f;
            }
            if (it.mtAuto && it.mbAuto)
            {
                fy = ay + mt + free * 0.5f;
            }
            else if (it.mtAuto)
            {
                fy = ay + mt + free;
            }
        }
        c->layout(fx, fy, fw, fh, r);
        // Stretch fills the area exactly — even overriding an explicit
        // size (verified vs Chrome; grid differs from flex here).
        // Larger content overflows (min-width:auto), never shrinks.
        // Persist into style (not just the member): later standalone
        // relayouts of this child must recompute the stretched size
        // instead of collapsing to content height. Stored as a
        // content-box value (layout adds padding/border back for
        // content-box sizing), so the relayout lands on the same box.
        if ((jv == 0 && c->w < fw) || (av == 0 && c->h < fh))
        {
            UnitEnv cEnv = c->unitEnv(cw, r);
            float cpl = resolveUnits(c->style.padding[3], cEnv);
            float cpr = resolveUnits(c->style.padding[1], cEnv);
            float cpt = resolveUnits(c->style.padding[0], cEnv);
            float cpb = resolveUnits(c->style.padding[2], cEnv);
#ifdef MORPH_FEATURE_BORDER
            float cbwL = getBorderWidth(c->style, 3, cEnv);
            float cbwR = getBorderWidth(c->style, 1, cEnv);
            float cbwT = getBorderWidth(c->style, 0, cEnv);
            float cbwB = getBorderWidth(c->style, 2, cEnv);
#else
            float cbwL = 0.0f, cbwR = 0.0f, cbwT = 0.0f, cbwB = 0.0f;
#endif
            bool borderBox = false;
#ifdef MORPH_FEATURE_BORDER_BOX
            borderBox = (c->style.boxSizing == CSS::BoxSizing::BorderBox);
#endif
            if (jv == 0 && c->w < fw)
            {
                c->w = fw;
                float ehW = borderBox ? fw : fw - cpl - cpr - cbwL - cbwR;
                if (ehW < 0.0f)
                {
                    ehW = 0.0f;
                }
                c->style.explicitWidth = pxLen(ehW);
                c->m_gridStretchW = true;
            }
            if (av == 0 && c->h < fh)
            {
                c->h = fh;
                float ehH = borderBox ? fh : fh - cpt - cpb - cbwT - cbwB;
                if (ehH < 0.0f)
                {
                    ehH = 0.0f;
                }
                c->style.explicitHeight = pxLen(ehH);
                c->m_gridStretchH = true;
            }
        }
        float cb = c->y + c->h + (it.mbAuto ? 0.0f : it.mb);
        if (cb > maxBottom)
        {
            maxBottom = cb;
        }
        float rb = c->x + c->w + (it.mrAuto ? 0.0f : it.mr);
        if (rb > maxRight)
        {
            maxRight = rb;
        }
    }
}

} // namespace grid_layout

#endif
