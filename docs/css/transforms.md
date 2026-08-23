# Transforms

CSS transforms apply transformations to elements — translation, rotation, scaling, and skewing, in both 2D and 3D. Angle values accept `deg`, `rad`, `grad`, and `turn`.

## Basic Usage

```css
.box {
  transform: rotate(45deg);
}
```

## Transform Functions

### Translate

Move an element from its position:

```css
.box { transform: translateX(20px); }
.box { transform: translateY(-10px); }
.box { transform: translate(20px, -10px); }
```

Percentage values reference the element's own size:

```css
.box { transform: translateX(50%); }
```

### Rotate

Rotate an element around its origin:

```css
.box { transform: rotate(45deg); }
.box { transform: rotate(-90deg); }
```

### Scale

Resize an element:

```css
.box { transform: scale(1.5); }
.box { transform: scale(0.8); }
.box { transform: scaleX(2); }
.box { transform: scaleY(0.5); }
```

### Skew

Tilt an element:

```css
.box { transform: skewX(10deg); }
.box { transform: skewY(-5deg); }
.box { transform: skew(10deg, -5deg); }
```

## 3D Transforms

3D functions compose into the same 4×4 matrix applied in the vertex shader:

```css
.box { transform: perspective(500px) rotateX(30deg); }
.box { transform: rotateY(45deg); }
.box { transform: rotateZ(90deg); }        /* same as rotate(90deg) */
.box { transform: rotate3d(1, 1, 0, 30deg); }
.box { transform: translate3d(10px, 20px, -30px); }
.box { transform: translateZ(-50px); }
.box { transform: scale3d(1.2, 1.2, 1); }
.box { transform: matrix3d(...16 values...); }
```

`perspective(d)` sets the view distance for subsequent 3D functions in the chain.

Global keywords are also accepted and resolve to no-op transforms: `inherit`, `initial`, `revert`, `revert-layer`, `unset`.

## Chaining Transforms

Apply multiple transforms by space-separating them:

```css
.box {
  transform: translateX(20px) rotate(45deg) scale(1.2);
}
```

Transforms are applied right-to-left: first scale, then rotate, then translate.

## Transform Origin

Set the point around which transforms occur:

```css
.box {
  transform-origin: center center;
  transform-origin: top left;
  transform-origin: 50% 50%;
}
```

Default is `50% 50%` (center of the element).

## Tailwind Utilities

```tsx
<div className="translate-x-4">Moved right 16px</div>
<div className="-translate-y-2">Moved up 8px</div>
<div className="rotate-45">Rotated 45deg</div>
<div className="-rotate-90">Rotated -90deg</div>
<div className="scale-110">Scaled up 10%</div>
<div className="scale-95">Scaled down 5%</div>
<div className="skew-x-6">Skewed</div>
<div className="rotate-[30deg]">Arbitrary rotation</div>
<div className="scale-[1.3]">Arbitrary scale</div>
```

## Transitions with Transforms

Transforms animate smoothly with CSS transitions:

```css
.box {
  transition: transform 0.3s ease;
}

.box:hover {
  transform: translateY(-4px) scale(1.02);
}
```

## How It Works

Morph parses transform functions into an internal representation, composes them into a 4x4 matrix (`mat4`), and applies the result in the vertex shader. This means transforms are GPU-accelerated and don't affect layout.

The `MORPH_FEATURE_TRANSFORM` compile-time gate ensures transform code is only included when a `transform` property is detected in your app.
