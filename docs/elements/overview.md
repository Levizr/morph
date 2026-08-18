# HTML Elements

Morph renders native elements using OpenGL. Not all HTML elements are supported — here's what works.

## Structural Elements

| Element | Notes |
|---|---|
| `<body>` | Root container. Default `padding: 8px` (not margin). Background fills the window. |
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
| `<a>` | Link (no navigation, styled as text). |

## Interactive Elements

| Element | Notes |
|---|---|
| `<button>` | Clickable button. Has default hover/active styles. |
| `<input>` | Text input field. |
| `<img>` | Image display (PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC). |

## Special Elements

| Element | Notes |
|---|---|
| `<morph-window>` | Declares a native window (alternative to `windowConfig` export). |

## JSX Attributes

### Standard HTML Attributes

| Attribute | Elements | Notes |
|---|---|---|
| `className` | all | CSS class names (space-separated). |
| `id` | all | Element identifier. |
| `style` | all | Inline CSS object: `style={{ color: "red" }}`. |
| `src` | `<img>` | Image path (relative to the project). |
| `alt` | `<img>` | Alt text (not rendered, but stored). |
| `href` | `<a>` | Link target. |
| `target` | `<a>` | Link target window. |
| `width` | `<img>` | Layout hint for intrinsic sizing. |
| `height` | `<img>` | Layout hint for intrinsic sizing. |

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
