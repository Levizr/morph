// 18_promises - Promise types, creation, chaining
let p1: Promise<string> = fetch("https://example.com");
console.log(p1);

let p2: Promise<number> = new Promise<number>((resolve) => {
    resolve(42);
});

async function getNumber(): Promise<number> {
    return 123;
}
let p3: Promise<number> = getNumber();
console.log(p3);

// Promise<void>
async function voidPromise(): Promise<void> {
    console.log("void");
}
let p4: Promise<void> = voidPromise();
console.log(p4);

// Promise with string
async function getString(): Promise<string> {
    let s: string = "hello";
    return s;
}
let p5: Promise<string> = getString();

// Chained await
async function chained(): Promise<number> {
    let a = await getNumber();
    let b = await getNumber();
    return a + b;
}
chained();

// Promise<string> with fetch
async function fetchString(): Promise<string> {
    let r: string = await fetch("https://example.com");
    return r;
}
fetchString();

// Generic promise helper
function wrap<T>(x: T): Promise<T> {
    return x as unknown as Promise<T>;
}
let wp: Promise<number> = wrap<number>(5);
console.log(wp);
