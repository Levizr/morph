# Animations

CSS animations let you define keyframe-based animations that run automatically — no hover or state change needed.

## Basic Usage

```css
@keyframes pulse {
  0% { transform: scale(1); }
  50% { transform: scale(1.05); }
  100% { transform: scale(1); }
}

.box {
  animation: pulse 2s ease-in-out infinite;
}
```

## Animation Shorthand

```css
/* name duration timing-function delay iteration-count direction fill-mode */
animation: pulse 2s ease-in-out 0s 1 normal forwards;
```

All properties are optional — omitted values use their defaults.

## Longhand Properties

```css
.box {
  animation-name: pulse;
  animation-duration: 2s;
  animation-timing-function: ease-in-out;
  animation-delay: 0s;
  animation-iteration-count: 1;
  animation-direction: normal;
  animation-fill-mode: none;
}
```

## @keyframes

Define keyframes with percentage offsets:

```css
@keyframes slide-in {
  0% { transform: translateX(-100px); opacity: 0; }
  100% { transform: translateX(0); opacity: 1; }
}
```

Or use `from`/`to`:

```css
@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
```

## Multiple Animations

Apply multiple animations to one element:

```css
.box {
  animation-name: pulse, color-cycle;
  animation-duration: 2s, 4s;
  animation-timing-function: ease-in-out, linear;
}
```

Or in shorthand:

```css
.box {
  animation: pulse 2s ease-in-out, color-cycle 4s linear;
}
```

## Iteration Count

```css
.box { animation-iteration-count: 3; }      /* run 3 times */
.box { animation-iteration-count: 2.5; }    /* run 2.5 times (fractional) */
.box { animation-iteration-count: infinite; } /* loop forever */
```

## Direction

| Value | Behavior |
|---|---|
| `normal` | 0% → 100% each iteration (default) |
| `reverse` | 100% → 0% each iteration |
| `alternate` | 0%→100%, then 100%→0%, then 0%→100%... |
| `alternate-reverse` | 100%→0%, then 0%→100%, then 100%→0%... |

## Fill Mode

| Value | Behavior |
|---|---|
| `none` | Element reverts to original style before/after animation (default) |
| `forwards` | Element keeps the final keyframe style after animation ends |
| `backwards` | Element applies the first keyframe style during the delay |
| `both` | Combines `forwards` and `backwards` |

## Easing in Keyframes

Use `animation-timing-function` inside keyframes for per-segment easing:

```css
@keyframes bounce {
  0% { transform: translateY(0); animation-timing-function: ease-out; }
  50% { transform: translateY(-20px); animation-timing-function: ease-in; }
  100% { transform: translateY(0); }
}
```

## Multi-Stop Keyframes

Animate through multiple intermediate states:

```css
@keyframes combo {
  0% { background-color: #6366f1; border-radius: 8px; opacity: 1; }
  33% { background-color: #8b5cf6; border-radius: 16px; opacity: 0.8; }
  66% { background-color: #ec4899; border-radius: 24px; opacity: 0.6; }
  100% { background-color: #6366f1; border-radius: 8px; opacity: 1; }
}
```

## Hover-Triggered Animations

Combine animations with hover to start/stop on mouse enter/leave:

```css
.card {
  animation: none;
}

.card:hover {
  animation: glow 1s ease-in-out infinite alternate;
}
```

## Example: Spinning Loader

```css
@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.loader {
  width: 40px;
  height: 40px;
  border: 3px solid #334155;
  border-top-color: #6366f1;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}
```

## Example: Fade In on Load

```css
@keyframes fade-in {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.hero {
  animation: fade-in 0.6s ease-out forwards;
}
```

## Percentage-Based Properties

Width, height, left, and transform values can use percentages in keyframes:

```css
@keyframes expand {
  0% { width: 20%; left: 0%; }
  100% { width: 80%; left: 10%; }
}
```
