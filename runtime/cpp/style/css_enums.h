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
