// 07_operators - binary, logical, nullish, update, assignment, comparison
let a: number = 10;
let b: number = 3;
console.log(a + b);
console.log(a - b);
console.log(a * b);
console.log(a / b);
console.log(a % b);
console.log(a > b);
console.log(a < b);
console.log(a >= b);
console.log(a <= b);
console.log(a === b);
console.log(a !== b);

let t: boolean = true;
let f: boolean = false;
console.log(t && f);
console.log(t || f);
console.log(!t);
console.log(!f);

let maybeNull: any = null;
let fallback: string = "default";
let result = maybeNull ?? fallback;
console.log(result);
let maybeUndef: any = undefined;
let result2 = maybeUndef ?? "fallback2";
console.log(result2);

let x: number = 5;
x++;
console.log(x);
++x;
console.log(x);
x--;
console.log(x);
--x;
console.log(x);

let y: number = 10;
y += 5;
console.log(y);
y -= 3;
console.log(y);
y *= 2;
console.log(y);
y /= 2;
console.log(y);

let p: number = 2;
let q: number = 3;
console.log(p * q);

let s1: string = "Hello";
let s2: string = "World";
console.log(`${s1}, ${s2}!`);

let c: number = a > b ? a : b;
console.log(c);

let d: boolean = (a > 5) && (b < 5);
console.log(d);
