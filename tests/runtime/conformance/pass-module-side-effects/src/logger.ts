import { getCount } from './state.ts'

console.log("logger init", getCount())

export function greet(): string {
  return "hi"
}
