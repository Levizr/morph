export function greet(): string {
  return "hi"
}

function ping(): string {
  return "pong"
}
export { ping as pong }

const suffix = "!"
export { suffix as mark }

const shout = (s: string): string => s + "!"
export default shout
