declare module "*.css"
declare module "*.cpp" {
  const exports: any
  export = exports
}
declare module "*.h"
declare module "*.hpp"

declare namespace JSX {
  interface IntrinsicElements {
    [elemName: string]: any
  }
}
