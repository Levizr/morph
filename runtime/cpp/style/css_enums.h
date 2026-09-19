#pragma once
#include <cstdint>
#include <string_view>

// Keyword enums for style properties and node identity. Hot paths
// (layout, paint, events) compare these integers; strings survive only
// at the boundary: codegen emits literals, the dev deserializer parses
// on load, debug surfaces print back. `CSS::` prefix: bare `Display`
// collides with X11/GLFW types already in the TU.
namespace CSS
{
enum class Display : uint8_t
{
    Block,
    Flex,
    None,
    Inline,
    InlineBlock
};

enum class Position : uint8_t
{
    Static,
    Absolute,
    Relative,
    Fixed,
    Sticky
};

// Parse replicating today's exact match semantics: case-sensitive,
// unknown (including "hidden", "", garbage) falls back to the default
// — today's comparisons fail the same way and take the default path.
inline Display parseDisplay(std::string_view s)
{
    if (s == "flex")
        return Display::Flex;
    if (s == "none")
        return Display::None;
    if (s == "inline")
        return Display::Inline;
    if (s == "inline-block")
        return Display::InlineBlock;
    return Display::Block;
}

inline Position parsePosition(std::string_view s)
{
    if (s == "absolute")
        return Position::Absolute;
    if (s == "relative")
        return Position::Relative;
    if (s == "fixed")
        return Position::Fixed;
    if (s == "sticky")
        return Position::Sticky;
    return Position::Static;
}

inline const char* toString(Display d)
{
    switch (d)
    {
    case Display::Flex:
        return "flex";
    case Display::None:
        return "none";
    case Display::Inline:
        return "inline";
    case Display::InlineBlock:
        return "inline-block";
    default:
        return "block";
    }
}

inline const char* toString(Position p)
{
    switch (p)
    {
    case Position::Absolute:
        return "absolute";
    case Position::Relative:
        return "relative";
    case Position::Fixed:
        return "fixed";
    case Position::Sticky:
        return "sticky";
    default:
        return "static";
    }
}

enum class TextAlign : uint8_t
{
    Left,
    Center,
    Right,
    Justify
};

enum class FontWeight : uint8_t
{
    Normal,
    Bold
};

enum class FlexDirection : uint8_t
{
    Row,
    Column,
    RowReverse,
    ColumnReverse
};

enum class JustifyContent : uint8_t
{
    FlexStart,
    Center,
    FlexEnd,
    SpaceBetween,
    SpaceAround
};

enum class AlignItems : uint8_t
{
    FlexStart,
    Center,
    FlexEnd,
    Stretch
};

enum class FlexWrap : uint8_t
{
    Nowrap,
    Wrap,
    WrapReverse
};

enum class Cursor : uint8_t
{
    Default,
    Pointer,
    Text
};

enum class BorderStyle : uint8_t
{
    None,
    Solid
};

enum class Overflow : uint8_t
{
    Visible,
    Hidden,
    Scroll,
    Auto
};

enum class BoxSizing : uint8_t
{
    ContentBox,
    BorderBox
};

// Parse replicating today's exact match semantics (see parseDisplay).
// fontWeight: bold-like is `bold|700|800|900` (the flatten 4-compare);
// everything else (100-600, bolder, lighter, garbage) is Normal.
// borderStyle: only `solid` renders; everything else is None.
inline TextAlign parseTextAlign(std::string_view s)
{
    if (s == "center")
        return TextAlign::Center;
    if (s == "right")
        return TextAlign::Right;
    if (s == "justify")
        return TextAlign::Justify;
    return TextAlign::Left;
}

inline FontWeight parseFontWeight(std::string_view s)
{
    if (s == "bold" || s == "700" || s == "800" || s == "900")
        return FontWeight::Bold;
    return FontWeight::Normal;
}

inline FlexDirection parseFlexDirection(std::string_view s)
{
    if (s == "row")
        return FlexDirection::Row;
    if (s == "row-reverse")
        return FlexDirection::RowReverse;
    if (s == "column-reverse")
        return FlexDirection::ColumnReverse;
    // `column` and anything unrecognized take the column path: only
    // `row` sets isRow in layout.
    return FlexDirection::Column;
}

inline JustifyContent parseJustifyContent(std::string_view s)
{
    if (s == "center")
        return JustifyContent::Center;
    if (s == "flex-end")
        return JustifyContent::FlexEnd;
    if (s == "space-between")
        return JustifyContent::SpaceBetween;
    if (s == "space-around")
        return JustifyContent::SpaceAround;
    return JustifyContent::FlexStart;
}

inline AlignItems parseAlignItems(std::string_view s)
{
    if (s == "center")
        return AlignItems::Center;
    if (s == "flex-end")
        return AlignItems::FlexEnd;
    if (s == "stretch")
        return AlignItems::Stretch;
    // flex-start and anything unrecognized share the else-branches.
    return AlignItems::FlexStart;
}

inline FlexWrap parseFlexWrap(std::string_view s)
{
    if (s == "wrap")
        return FlexWrap::Wrap;
    if (s == "wrap-reverse")
        return FlexWrap::WrapReverse;
    return FlexWrap::Nowrap;
}

inline Cursor parseCursor(std::string_view s)
{
    if (s == "pointer")
        return Cursor::Pointer;
    if (s == "text")
        return Cursor::Text;
    return Cursor::Default;
}

inline BorderStyle parseBorderStyle(std::string_view s)
{
    if (s == "solid")
        return BorderStyle::Solid;
    return BorderStyle::None;
}

inline Overflow parseOverflow(std::string_view s)
{
    if (s == "hidden")
        return Overflow::Hidden;
    if (s == "scroll")
        return Overflow::Scroll;
    if (s == "auto")
        return Overflow::Auto;
    return Overflow::Visible;
}

inline BoxSizing parseBoxSizing(std::string_view s)
{
    if (s == "border-box")
        return BoxSizing::BorderBox;
    return BoxSizing::ContentBox;
}

inline const char* toString(TextAlign a)
{
    switch (a)
    {
    case TextAlign::Center:
        return "center";
    case TextAlign::Right:
        return "right";
    case TextAlign::Justify:
        return "justify";
    default:
        return "left";
    }
}

inline const char* toString(FontWeight w)
{
    return (w == FontWeight::Bold) ? "bold" : "normal";
}

inline const char* toString(FlexDirection d)
{
    switch (d)
    {
    case FlexDirection::Column:
        return "column";
    case FlexDirection::RowReverse:
        return "row-reverse";
    case FlexDirection::ColumnReverse:
        return "column-reverse";
    default:
        return "row";
    }
}

inline const char* toString(JustifyContent j)
{
    switch (j)
    {
    case JustifyContent::Center:
        return "center";
    case JustifyContent::FlexEnd:
        return "flex-end";
    case JustifyContent::SpaceBetween:
        return "space-between";
    case JustifyContent::SpaceAround:
        return "space-around";
    default:
        return "flex-start";
    }
}

inline const char* toString(AlignItems a)
{
    switch (a)
    {
    case AlignItems::Center:
        return "center";
    case AlignItems::FlexEnd:
        return "flex-end";
    case AlignItems::Stretch:
        return "stretch";
    default:
        return "flex-start";
    }
}

inline const char* toString(FlexWrap w)
{
    switch (w)
    {
    case FlexWrap::Wrap:
        return "wrap";
    case FlexWrap::WrapReverse:
        return "wrap-reverse";
    default:
        return "nowrap";
    }
}

inline const char* toString(Cursor c)
{
    switch (c)
    {
    case Cursor::Pointer:
        return "pointer";
    case Cursor::Text:
        return "text";
    default:
        return "default";
    }
}

inline const char* toString(BorderStyle b)
{
    return (b == BorderStyle::Solid) ? "solid" : "none";
}

inline const char* toString(Overflow o)
{
    switch (o)
    {
    case Overflow::Hidden:
        return "hidden";
    case Overflow::Scroll:
        return "scroll";
    case Overflow::Auto:
        return "auto";
    default:
        return "visible";
    }
}

inline const char* toString(BoxSizing b)
{
    return (b == BoxSizing::BorderBox) ? "border-box" : "content-box";
}
} // namespace CSS

// Node identity for inline grouping, inline width, and paint order.
// Unknown tags (body, span, ...) map to Custom: nothing compares
// against them today, so behavior is unchanged.
enum class NodeType : uint8_t
{
    Div,
    Button,
    Input,
    Img,
    Text,
    Expr,
    Conditional,
    List,
    Fragment,
    Custom
};

inline NodeType parseNodeType(std::string_view s)
{
    if (s == "button")
        return NodeType::Button;
    if (s == "input")
        return NodeType::Input;
    if (s == "img")
        return NodeType::Img;
    if (s == "__text__")
        return NodeType::Text;
    if (s == "__expr__")
        return NodeType::Expr;
    if (s == "__conditional__")
        return NodeType::Conditional;
    if (s == "__list__")
        return NodeType::List;
    if (s == "__fragment__")
        return NodeType::Fragment;
    if (s == "div" || s.empty())
        return NodeType::Div;
    return NodeType::Custom;
}

inline const char* toString(NodeType t)
{
    switch (t)
    {
    case NodeType::Button:
        return "button";
    case NodeType::Input:
        return "input";
    case NodeType::Img:
        return "img";
    case NodeType::Text:
        return "__text__";
    case NodeType::Expr:
        return "__expr__";
    case NodeType::Conditional:
        return "__conditional__";
    case NodeType::List:
        return "__list__";
    case NodeType::Fragment:
        return "__fragment__";
    case NodeType::Custom:
        return "custom";
    default:
        return "div";
    }
}
