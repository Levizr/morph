import { ping } from './cycle_a.ts'

console.log("cycle b before")
export function b(): string { return "b" }
console.log("cycle b after", ping())
