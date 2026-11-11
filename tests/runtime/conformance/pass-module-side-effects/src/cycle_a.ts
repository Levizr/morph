import { b } from './cycle_b.ts'

console.log("cycle a before")
export function ping(): string { return "ping" }
console.log("cycle a after", b())
