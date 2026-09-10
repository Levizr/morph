// 27_escaping_closure - a returned closure keeps shared state alive;
// the lambda captures the shared counter by value.
function makeCounter() {
    let count: number = 0;
    const bump = (): number => {
        count = count + 1;
        return count;
    };
    return bump;
}

async function main(): Promise<void> {
    const next = makeCounter();
    console.log(next());
    console.log(next());
}
main();
