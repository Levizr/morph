// 19_fetch - fetch with await, without await, various URLs
let r1 = await fetch("https://example.com");
console.log(r1);

let r2 = fetch("https://example.com/api");
console.log(r2);

async function doFetch(): Promise<void> {
    let a = await fetch("https://example.com/a");
    console.log(a);
    let b = await fetch("/api/data");
    console.log(b);
    let c = fetch("https://example.com/c");
    console.log(c);
}
doFetch();

let url: string = "https://example.com/dynamic";
let r3 = await fetch(url);
console.log(r3);

async function fetchWithVar(): Promise<string> {
    let u: string = "https://example.com/var";
    let res = await fetch(u);
    return res;
}
fetchWithVar();

let simpleFetch = fetch("https://example.com/simple");
console.log(simpleFetch);

// fetch in loop
async function fetchLoop(): Promise<void> {
    for (let i: number = 0; i < 2; i++) {
        let r = await fetch("https://example.com/loop");
        console.log(r);
    }
}
fetchLoop();
