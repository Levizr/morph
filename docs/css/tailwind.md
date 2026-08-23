# Tailwind CSS

Morph includes a built-in Tailwind CSS resolver with 500+ common utility classes. No Node.js or Tailwind installation required.

## Usage

Add Tailwind classes directly to `className`:

```tsx
<div className="flex items-center gap-4 p-4 bg-gray-900 rounded-lg">
  <span className="text-white font-bold">Hello</span>
  <button className="bg-indigo-500 text-white px-4 py-2 rounded cursor-pointer">
    Click
  </button>
</div>
```

## Arbitrary Values

Use the `prefix-[value]` syntax for any value not in the static map:

```tsx
<div className="bg-[#1a1a2e] w-[300px] p-[14px] text-[13px]">
  Custom values
</div>
```

Supported prefixes: `bg-`, `text-`, `w-`, `h-`, `p-`, `px-`, `py-`, `m-`, `mt-`, `mb-`, `ml-`, `mr-`, `gap-`, `rounded-`, `opacity-`, `top-`, `bottom-`, `left-`, `right-`, `min-w-`, `min-h-`, `max-w-`, `max-h-`, `z-`, and `font-` (maps to `font-weight`).

Negative values work too:

```tsx
<div className="-translate-x-[50%]">Shifted left half its width</div>
```

## Supported Categories

### Background Colors

`bg-transparent`, `bg-black`, `bg-white`, `bg-gray-50` through `bg-gray-900`, `bg-red-500`, `bg-blue-500`/`600`, `bg-green-500`, `bg-yellow-500`, `bg-purple-500`/`600`, `bg-pink-500`, `bg-indigo-500`/`600`

### Text Colors

`text-black`, `text-white`, `text-gray-400` through `text-gray-900`, `text-red-500`, `text-blue-500`, `text-green-500`, `text-purple-500`

### Font Size

| Class | Size |
|---|---|
| `text-xs` | 12px |
| `text-sm` | 14px |
| `text-base` | 16px |
| `text-lg` | 18px |
| `text-xl` | 20px |
| `text-2xl` | 24px |
| `text-3xl` | 30px |
| `text-4xl` | 36px |

### Font Weight

`font-thin` (100), `font-light` (300), `font-normal` (400), `font-medium` (500), `font-semibold` (600), `font-bold` (700), `font-extrabold` (800)

### Text Align

`text-left`, `text-center`, `text-right`

### Display

`block`, `inline`, `inline-block`, `flex`, `inline-flex`, `hidden`

### Flexbox

| Class | Property |
|---|---|
| `flex-row`, `flex-col`, `flex-row-reverse`, `flex-col-reverse` | `flex-direction` |
| `flex-wrap`, `flex-nowrap` | `flex-wrap` |
| `flex-1`, `flex-auto`, `flex-none` | `flex` shorthand |
| `items-start`, `items-center`, `items-end`, `items-stretch` | `align-items` |
| `justify-start`, `justify-center`, `justify-end`, `justify-between`, `justify-around` | `justify-content` |

### Sizing

- **Width:** `w-0` through `w-64` (4px scale), `w-full`, `w-screen`, `w-auto`
- **Height:** `h-0` through `h-20` (4px scale), `h-full`, `h-screen`, `h-auto`
- **Min:** `min-w-0`, `min-h-0`

### Spacing

- **Padding:** `p-0` through `p-12`, `px-*`, `py-*`, `pt-4`, `pb-4`, `pl-4`, `pr-4`
- **Margin:** `m-0` through `m-8`, `m-auto`, `mx-auto`, `mx-4`, `my-4`, `mt-*`, `mb-*`, `ml-*`, `mr-*`
- **Gap:** `gap-0` through `gap-8`

### Border Radius

`rounded-none`, `rounded-sm`, `rounded`, `rounded-md`, `rounded-lg`, `rounded-xl`, `rounded-2xl`, `rounded-3xl`, `rounded-full`

### Opacity

`opacity-0`, `opacity-25`, `opacity-50`, `opacity-75`, `opacity-100`

### Overflow

`overflow-hidden`, `overflow-auto`, `overflow-scroll`, `overflow-visible`

### Position

`relative`, `absolute`, `fixed`, `sticky`, `static`

### Z-Index

`z-0`, `z-10` through `z-50`, `z-auto`, and negative values `-z-10` through `-z-50` (arbitrary too: `z-[100]`).

### Cursor

`cursor-pointer`, `cursor-default`, `cursor-not-allowed`

### User Select

`select-none`, `select-text`, `select-all`

### Transforms

| Class | Property |
|---|---|
| `translate-x-0` through `translate-x-64`, `-translate-x-*` | `translateX()` |
| `translate-y-0` through `translate-y-64`, `-translate-y-*` | `translateY()` |
| `rotate-0` through `rotate-180`, `-rotate-*` | `rotate()` |
| `scale-0` through `scale-150`, `scale-x-*`, `scale-y-*` | `scale()` |
| `skew-x-0` through `skew-x-12`, `skew-y-*` | `skew()` |

All transform utilities also support arbitrary values: `rotate-[45deg]`, `scale-[1.2]`, etc.

## Missing Classes

If a Tailwind class isn't in the static map, it's silently skipped. Use arbitrary value syntax as a fallback:

```tsx
{/* bg-orange-500 isn't in the static map */}
<div className="bg-[#f97316]">Works anyway</div>
```
