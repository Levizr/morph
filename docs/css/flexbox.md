# Flexbox

Morph implements CSS Flexbox with two-pass layout — first measuring children at a temporary position, then re-laying them out at their final position after flex adjustments.

## Basics

```css
.container {
  display: flex;
}
```

This makes the element a flex container. Children become flex items laid out in a row by default.

## Direction

```css
.row    { flex-direction: row; }         /* default */
.col    { flex-direction: column; }
.row-r  { flex-direction: row-reverse; }
.col-r  { flex-direction: column-reverse; }
```

## Justify Content (Main Axis)

```css
.start     { justify-content: flex-start; }    /* default */
.center    { justify-content: center; }
.end       { justify-content: flex-end; }
.between   { justify-content: space-between; }
.around    { justify-content: space-around; }
```

## Align Items (Cross Axis)

```css
.start   { align-items: flex-start; }
.center  { align-items: center; }
.end     { align-items: flex-end; }
.stretch { align-items: stretch; }     /* default */
```

## Gap

```css
.container { gap: 16px; }
```

Adds spacing between flex items without adding margins. Works in both row and column directions.

## Flex Shorthand

```css
.item { flex: 1; }      /* grow: 1, shrink: 1, basis: 0%   — fills available space */
.item { flex: auto; }    /* grow: 1, shrink: 1, basis: auto — sizes to content */
.item { flex: none; }    /* grow: 0, shrink: 0, basis: auto — fixed size */
```

You can also set individual properties:

```css
.item {
  flex-grow: 2;
  flex-shrink: 0;
  flex-basis: 200px;
}
```

## Flex Wrap

```css
.container {
  flex-wrap: wrap;
}
```

When items exceed the container width, they wrap to the next line. Each wrapped line gets its own `justify-content` pass.

## Common Patterns

### Centering

```css
.center {
  display: flex;
  justify-content: center;
  align-items: center;
}
```

### Navbar

```css
.navbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 24px;
}
```

### Card Grid

```css
.grid {
  display: flex;
  flex-wrap: wrap;
  gap: 24px;
}

.card {
  flex: 1;
  min-width: 200px;
}
```

### Holy Grail Layout

```css
body {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
}

.content {
  flex: 1;  /* fills remaining space */
}
```
