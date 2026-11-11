// `n` is read before its declaration, so the declaration moves into the
// module initializer and the early read sees `undefined`.
console.log("state before", n)
var n = 10
console.log("state after", n)

// A non-late top-level var stays a static binding; the bare mutation
// below runs once when this module initializes.
export var table = [1, 2]
console.log("state table", table.length)

export function getCount(): number {
  return table.length + n
}

export function bump(): void {
  n = n + 1
  console.log("bump", n)
}
