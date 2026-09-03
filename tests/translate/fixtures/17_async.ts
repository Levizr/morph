// 17_async - async functions, await, async arrow, async method, async main
async function fetchData(): Promise<string> {
    let r = await fetch("https://example.com");
    return r;
}

async function process(): Promise<number> {
    let x: number = 10;
    let y = await fetchData();
    console.log(y);
    return x + 1;
}

let asyncArrow = async (x: number): Promise<number> => {
    let y = await fetchData();
    return x + 1;
};

async function withAwait(): Promise<void> {
    let a = await fetch("https://example.com/api");
    console.log(a);
    let b = await process();
    console.log(b);
}

class AsyncClass {
    async fetchMethod(): Promise<string> {
        let r = await fetch("https://example.com/method");
        return r;
    }
    async compute(x: number): Promise<number> {
        let r = await fetchData();
        return x * 2;
    }
}

let ac = new AsyncClass();
ac.fetchMethod();
ac.compute(5);

// Simple await in non-async? (should be inside async)
async function simpleAwait(): Promise<number> {
    let v = await fetchData();
    console.log(v);
    return 42;
}
simpleAwait();

async function main() {
    let data = await fetchData();
    console.log(data);
    let v2 = await process();
    console.log(v2);
}
main();
