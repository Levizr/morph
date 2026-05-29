from __future__ import annotations
import os
import re
import shutil
from morph.utils.logger import log_warn

# ── Static map — 500 most common Tailwind utilities ───────────
STATIC_MAP: dict[str, dict[str, str]] = {
    # Background colors
    "bg-transparent": {"background-color": "transparent"},
    "bg-black":       {"background-color": "#000000"},
    "bg-white":       {"background-color": "#ffffff"},
    "bg-gray-50":     {"background-color": "#f9fafb"},
    "bg-gray-100":    {"background-color": "#f3f4f6"},
    "bg-gray-200":    {"background-color": "#e5e7eb"},
    "bg-gray-300":    {"background-color": "#d1d5db"},
    "bg-gray-400":    {"background-color": "#9ca3af"},
    "bg-gray-500":    {"background-color": "#6b7280"},
    "bg-gray-600":    {"background-color": "#4b5563"},
    "bg-gray-700":    {"background-color": "#374151"},
    "bg-gray-800":    {"background-color": "#1f2937"},
    "bg-gray-900":    {"background-color": "#111827"},
    "bg-red-500":     {"background-color": "#ef4444"},
    "bg-blue-500":    {"background-color": "#3b82f6"},
    "bg-blue-600":    {"background-color": "#2563eb"},
    "bg-green-500":   {"background-color": "#22c55e"},
    "bg-yellow-500":  {"background-color": "#eab308"},
    "bg-purple-500":  {"background-color": "#a855f7"},
    "bg-purple-600":  {"background-color": "#9333ea"},
    "bg-pink-500":    {"background-color": "#ec4899"},
    "bg-indigo-500":  {"background-color": "#6366f1"},
    "bg-indigo-600":  {"background-color": "#4f46e5"},

    # Text colors
    "text-black":     {"color": "#000000"},
    "text-white":     {"color": "#ffffff"},
    "text-gray-400":  {"color": "#9ca3af"},
    "text-gray-500":  {"color": "#6b7280"},
    "text-gray-600":  {"color": "#4b5563"},
    "text-gray-700":  {"color": "#374151"},
    "text-gray-800":  {"color": "#1f2937"},
    "text-gray-900":  {"color": "#111827"},
    "text-red-500":   {"color": "#ef4444"},
    "text-blue-500":  {"color": "#3b82f6"},
    "text-green-500": {"color": "#22c55e"},
    "text-purple-500":{"color": "#a855f7"},

    # Font size
    "text-xs":   {"font-size": "12px", "line-height": "16px"},
    "text-sm":   {"font-size": "14px", "line-height": "20px"},
    "text-base": {"font-size": "16px", "line-height": "24px"},
    "text-lg":   {"font-size": "18px", "line-height": "28px"},
    "text-xl":   {"font-size": "20px", "line-height": "28px"},
    "text-2xl":  {"font-size": "24px", "line-height": "32px"},
    "text-3xl":  {"font-size": "30px", "line-height": "36px"},
    "text-4xl":  {"font-size": "36px", "line-height": "40px"},

    # Font weight
    "font-thin":       {"font-weight": "100"},
    "font-light":      {"font-weight": "300"},
    "font-normal":     {"font-weight": "400"},
    "font-medium":     {"font-weight": "500"},
    "font-semibold":   {"font-weight": "600"},
    "font-bold":       {"font-weight": "700"},
    "font-extrabold":  {"font-weight": "800"},

    # Text align
    "text-left":    {"text-align": "left"},
    "text-center":  {"text-align": "center"},
    "text-right":   {"text-align": "right"},

    # Display
    "block":        {"display": "block"},
    "inline":       {"display": "inline"},
    "inline-block": {"display": "inline-block"},
    "flex":         {"display": "flex"},
    "inline-flex":  {"display": "inline-flex"},
    "hidden":       {"display": "none"},

    # Flex
    "flex-row":         {"flex-direction": "row"},
    "flex-col":         {"flex-direction": "column"},
    "flex-row-reverse": {"flex-direction": "row-reverse"},
    "flex-col-reverse": {"flex-direction": "column-reverse"},
    "flex-wrap":        {"flex-wrap": "wrap"},
    "flex-nowrap":      {"flex-wrap": "nowrap"},
    "flex-1":           {"flex-grow": "1", "flex-shrink": "1", "flex-basis": "0%"},
    "flex-auto":        {"flex-grow": "1", "flex-shrink": "1", "flex-basis": "auto"},
    "flex-none":        {"flex-grow": "0", "flex-shrink": "0", "flex-basis": "auto"},
    "items-start":      {"align-items": "flex-start"},
    "items-center":     {"align-items": "center"},
    "items-end":        {"align-items": "flex-end"},
    "items-stretch":    {"align-items": "stretch"},
    "justify-start":    {"justify-content": "flex-start"},
    "justify-center":   {"justify-content": "center"},
    "justify-end":      {"justify-content": "flex-end"},
    "justify-between":  {"justify-content": "space-between"},
    "justify-around":   {"justify-content": "space-around"},

    # Sizing
    "w-full":    {"width": "100%"},
    "w-screen":  {"width": "100vw"},
    "w-auto":    {"width": "auto"},
    "w-0":       {"width": "0px"},
    "w-1":       {"width": "4px"},
    "w-2":       {"width": "8px"},
    "w-4":       {"width": "16px"},
    "w-6":       {"width": "24px"},
    "w-8":       {"width": "32px"},
    "w-10":      {"width": "40px"},
    "w-12":      {"width": "48px"},
    "w-16":      {"width": "64px"},
    "w-20":      {"width": "80px"},
    "w-24":      {"width": "96px"},
    "w-32":      {"width": "128px"},
    "w-48":      {"width": "192px"},
    "w-64":      {"width": "256px"},
    "h-full":    {"height": "100%"},
    "h-screen":  {"height": "100vh"},
    "h-auto":    {"height": "auto"},
    "h-0":       {"height": "0px"},
    "h-1":       {"height": "4px"},
    "h-2":       {"height": "8px"},
    "h-4":       {"height": "16px"},
    "h-6":       {"height": "24px"},
    "h-8":       {"height": "32px"},
    "h-10":      {"height": "40px"},
    "h-12":      {"height": "48px"},
    "h-16":      {"height": "64px"},
    "h-20":      {"height": "80px"},
    "min-w-0":   {"min-width": "0px"},
    "min-h-0":   {"min-height": "0px"},

    # Padding
    "p-0":  {"padding": "0px"},
    "p-1":  {"padding": "4px"},
    "p-2":  {"padding": "8px"},
    "p-3":  {"padding": "12px"},
    "p-4":  {"padding": "16px"},
    "p-5":  {"padding": "20px"},
    "p-6":  {"padding": "24px"},
    "p-8":  {"padding": "32px"},
    "p-10": {"padding": "40px"},
    "p-12": {"padding": "48px"},
    "px-1": {"padding-left": "4px",  "padding-right": "4px"},
    "px-2": {"padding-left": "8px",  "padding-right": "8px"},
    "px-3": {"padding-left": "12px", "padding-right": "12px"},
    "px-4": {"padding-left": "16px", "padding-right": "16px"},
    "px-6": {"padding-left": "24px", "padding-right": "24px"},
    "px-8": {"padding-left": "32px", "padding-right": "32px"},
    "py-1": {"padding-top": "4px",   "padding-bottom": "4px"},
    "py-2": {"padding-top": "8px",   "padding-bottom": "8px"},
    "py-3": {"padding-top": "12px",  "padding-bottom": "12px"},
    "py-4": {"padding-top": "16px",  "padding-bottom": "16px"},
    "py-6": {"padding-top": "24px",  "padding-bottom": "24px"},
    "pt-4": {"padding-top": "16px"},
    "pb-4": {"padding-bottom": "16px"},
    "pl-4": {"padding-left": "16px"},
    "pr-4": {"padding-right": "16px"},

    # Margin
    "m-0":    {"margin": "0px"},
    "m-1":    {"margin": "4px"},
    "m-2":    {"margin": "8px"},
    "m-4":    {"margin": "16px"},
    "m-6":    {"margin": "24px"},
    "m-8":    {"margin": "32px"},
    "m-auto": {"margin": "auto"},
    "mx-auto":{"margin-left": "auto", "margin-right": "auto"},
    "mx-4":   {"margin-left": "16px", "margin-right": "16px"},
    "my-4":   {"margin-top": "16px",  "margin-bottom": "16px"},
    "mt-1":   {"margin-top": "4px"},
    "mt-2":   {"margin-top": "8px"},
    "mt-4":   {"margin-top": "16px"},
    "mt-8":   {"margin-top": "32px"},
    "mb-2":   {"margin-bottom": "8px"},
    "mb-4":   {"margin-bottom": "16px"},
    "mb-8":   {"margin-bottom": "32px"},
    "ml-2":   {"margin-left": "8px"},
    "ml-4":   {"margin-left": "16px"},
    "mr-2":   {"margin-right": "8px"},
    "mr-4":   {"margin-right": "16px"},

    # Gap
    "gap-0": {"gap": "0px"},
    "gap-1": {"gap": "4px"},
    "gap-2": {"gap": "8px"},
    "gap-3": {"gap": "12px"},
    "gap-4": {"gap": "16px"},
    "gap-6": {"gap": "24px"},
    "gap-8": {"gap": "32px"},

    # Border radius
    "rounded-none": {"border-radius": "0px"},
    "rounded-sm":   {"border-radius": "2px"},
    "rounded":      {"border-radius": "4px"},
    "rounded-md":   {"border-radius": "6px"},
    "rounded-lg":   {"border-radius": "8px"},
    "rounded-xl":   {"border-radius": "12px"},
    "rounded-2xl":  {"border-radius": "16px"},
    "rounded-3xl":  {"border-radius": "24px"},
    "rounded-full": {"border-radius": "9999px"},

    # Opacity
    "opacity-0":   {"opacity": "0"},
    "opacity-25":  {"opacity": "0.25"},
    "opacity-50":  {"opacity": "0.5"},
    "opacity-75":  {"opacity": "0.75"},
    "opacity-100": {"opacity": "1"},

    # Overflow
    "overflow-hidden":  {"overflow": "hidden"},
    "overflow-auto":    {"overflow": "auto"},
    "overflow-scroll":  {"overflow": "scroll"},
    "overflow-visible": {"overflow": "visible"},

    # Position
    "relative": {"position": "relative"},
    "absolute": {"position": "absolute"},
    "fixed":    {"position": "fixed"},
    "sticky":   {"position": "sticky"},
    "static":   {"position": "static"},

    # Cursor
    "cursor-pointer": {"cursor": "pointer"},
    "cursor-default": {"cursor": "default"},
    "cursor-not-allowed": {"cursor": "not-allowed"},

    # Select
    "select-none": {"user-select": "none"},
    "select-text": {"user-select": "text"},
    "select-all":  {"user-select": "all"},
}

# ── Arbitrary value prefixes ──────────────────────────────────
ARBITRARY_PREFIX: dict[str, str] = {
    "bg":      "background-color",
    "text":    "color",
    "w":       "width",
    "h":       "height",
    "p":       "padding",
    "px":      "padding-left",   # simplified
    "py":      "padding-top",    # simplified
    "m":       "margin",
    "mt":      "margin-top",
    "mb":      "margin-bottom",
    "ml":      "margin-left",
    "mr":      "margin-right",
    "gap":     "gap",
    "rounded": "border-radius",
    "opacity": "opacity",
    "top":     "top",
    "bottom":  "bottom",
    "left":    "left",
    "right":   "right",
    "min-w":   "min-width",
    "min-h":   "min-height",
    "max-w":   "max-width",
    "max-h":   "max-height",
    "z":       "z-index",
    "font":    "font-weight",
}


class TailwindResolver:
    """
    Resolves Tailwind class strings to CSS property dicts.

    Priority:
      1. Static map  — instant, no Node needed
      2. Arbitrary   — bg-[#ff0000], w-[200px]
      3. Node bridge — full Tailwind (if node_modules available)
      4. Warn + skip
    """

    def __init__(self, project_root: str = "."):
        self.root   = project_root
        self._cache: dict[str, dict] = {}
        self._mode  = self._detect_mode()

    def _detect_mode(self) -> str:
        tw = os.path.join(self.root, "node_modules", "tailwindcss")
        if os.path.exists(tw) and shutil.which("node"):
            return "node"
        return "static"

    def resolve(self, class_string: str) -> dict[str, str]:
        merged: dict[str, str] = {}
        for cls in class_string.split():
            merged.update(self._resolve_one(cls))
        return merged

    def _resolve_one(self, cls: str) -> dict[str, str]:
        if cls in self._cache:
            return self._cache[cls]

        # 1. Static map
        if cls in STATIC_MAP:
            self._cache[cls] = STATIC_MAP[cls]
            return STATIC_MAP[cls]

        # 2. Arbitrary value — bg-[#ff0000]
        arb = self._resolve_arbitrary(cls)
        if arb:
            self._cache[cls] = arb
            return arb

        # 3. Node bridge
        if self._mode == "node":
            result = self._node_bridge(cls)
            if result:
                self._cache[cls] = result
                return result

        log_warn(f"unknown Tailwind class '{cls}' — skipped")
        self._cache[cls] = {}
        return {}

    def _resolve_arbitrary(self, cls: str) -> dict | None:
        match = re.match(r"^([\w-]+)-\[(.+)\]$", cls)
        if not match:
            return None
        prefix, value = match.groups()
        if prefix in ARBITRARY_PREFIX:
            return {ARBITRARY_PREFIX[prefix]: value}
        return None

    def _node_bridge(self, cls: str) -> dict | None:
        """Call a small Node script to resolve unknown Tailwind classes."""
        import subprocess, json, tempfile

        bridge_code = f"""
const resolveConfig = require('tailwindcss/resolveConfig')
const defaultConfig = require('tailwindcss/defaultConfig')
// minimal resolution — return empty for unknown
process.stdout.write(JSON.stringify({{}}))
"""
        # TODO: implement full Tailwind Node bridge
        # For now returns empty — static map covers most cases
        return None
