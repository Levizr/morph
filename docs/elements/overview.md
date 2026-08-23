# HTML Elements

Morph renders native elements using OpenGL. Not all HTML elements are supported — here's what works.

## Structural Elements

| Element | Notes |
|---|---|
| `<html>`, `<body>` | Root containers. `<body>` has default `padding: 8px` (not margin). Background fills the window. |
| `<div>` | Block-level container. The workhorse element. |
| `<span>` | Inline text container. |
| `<h1>`–`<h6>` | Headings with default font sizes. |
| `<p>` | Paragraph with bottom margin. |
| `<pre>` | Preformatted text. |
| `<header>`, `<footer>`, `<nav>`, `<section>`, `<article>`, `<aside>`, `<main>` | Semantic block containers (same layout behavior as `<div>`). |
| `<blockquote>`, `<figure>`, `<figcaption>` | Block containers with default margins. |
| `<hr>` | Horizontal rule. |
| `<ul>`, `<ol>`, `<li>` | Lists. |
| `<dl>`, `<dt>`, `<dd>` | Definition lists. |
| `<form>` | Block container (no submit semantics). |
| `<fieldset>`, `<legend>` | Bordered group container + its caption (browser-like default border/padding). |
| `<table>`, `<caption>`, `<thead>`, `<tbody>`, `<tfoot>`, `<tr>`, `<td>`, `<th>` | Rendered as block containers — no table layout yet; use flexbox for grids. `<th>` is bold + centered. |
| `<details>`, `<summary>`, `<dialog>` | Plain block containers (no toggle/modal behavior yet). |

## Text Elements

| Element | Notes |
|---|---|
| `<strong>`, `<b>` | Bold text. |
| `<em>`, `<i>` | Italic text. |
| `<small>` | Small text. |
| `<code>`, `<kbd>`, `<samp>` | Monospace text. |
| `<mark>` | Highlighted text. |
| `<sub>`, `<sup>` | Subscript / superscript. |
| `<ins>`, `<u>` | Underline. |
| `<del>`, `<s>` | Strikethrough. |
| `<q>` | Inline quotation (styled as inline text). |
| `<label>` | Inline label text. |
| `<a>` | Link (no navigation, styled as text). |

## Interactive Elements

| Element | Notes |
|---|---|
| `<button>` | Clickable button. Has default hover/active styles. |
| `<input>` | Text input field with caret, selection, undo/redo, and clipboard support. |
| `<img>` | Image display (PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC). |
| `<select>`, `<textarea>` | Registered but not fully implemented yet (render as inline-block containers; the linter flags them with `mx-tag-stub`). |

### `<input>` Attributes

| Attribute | Type | Notes |
|---|---|---|
| `value` | string | Initial/current text content. |
| `placeholder` | string | Hint text shown when empty. |
| `type` | `"text"` \| `"password"` | `password` masks input characters. |
| `disabled` | boolean | Disables editing and focus. |
| `maxLength` | number | Max characters (browser-parity cap of 524288 when unset). |
| `minLength` | number | Min characters for validation. |

## Special Elements

### `<morph-window>`

Declares a native window (alternative to the `windowConfig` export — use one, not both).

| Prop | Type | Notes |
|---|---|---|
| `title` | string | Window title bar text. |
| `width` | number | Window width in CSS pixels. |
| `height` | number | Window height in CSS pixels. |
| `minWidth` | number | Minimum window width (enforced via GLFW size limits). |
| `minHeight` | number | Minimum window height. |
| `maxWidth` | number | Maximum window width. |
| `maxHeight` | number | Maximum window height. |

```tsx
<morph-window title="Calculator" width={340} height={500}
              minWidth={340} minHeight={500}>
  ...
</morph-window>
```

## JSX Attributes

### Standard HTML Attributes

| Attribute | Elements | Notes |
|---|---|---|
| `className` | all | CSS class names (space-separated). |
| `class` | all | Alias for `className` — use one or the other, never both (linter: `mx-dup-class`). |
| `id` | all | Element identifier. |
| `style` | all | Inline CSS object: `style={{ color: "red" }}`. |
| `key` | `.map()` lists | List identity for re-renders. |
| `src` | `<img>` | Image path (relative to the project). |
| `alt` | `<img>` | Alt text (not rendered, but stored). |
| `href` | `<a>` | Link target. |
| `target` | `<a>` | Link target window. |
| `width` | `<img>` | Layout hint for intrinsic sizing. |
| `height` | `<img>` | Layout hint for intrinsic sizing. |
| See [`<input>` attributes](#input-attributes) | `<input>` | `value`, `placeholder`, `type`, `disabled`, `maxLength`, `minLength`. |

### Event Handlers

See [Events](events.md) for the full list.

### Morph-Specific

| Attribute | Notes |
|---|---|
| `morph-open` | Opens a URL or file on click. |
| `morph-close` | Closes the window on click. |
| `morph-navigate` | Navigates to a URL on click. |

## Fragments

Use fragments (`<>...</>`) to wrap multiple children without adding an extra node:

```tsx
return (
  <>
    <div>First</div>
    <div>Second</div>
  </>
)
```

## Images

```tsx
<img src="src/avatar.png" alt="Avatar" />
<img src="https://example.com/photo.jpg" width={200} height={150} />
```

Supported formats: PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC. Images are loaded via `stb_image` and cached as GPU textures. `border-radius` clipping works on images via stencil buffers.
