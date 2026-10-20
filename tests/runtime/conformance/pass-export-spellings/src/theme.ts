import { morphShared } from 'morph'

export const [theme, setTheme] = morphShared("light")

const [accent, setAccent] = morphShared("#334155")
export { accent as tone, setAccent as setTone }
