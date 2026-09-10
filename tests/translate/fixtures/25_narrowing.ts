// 25_narrowing - straight-line reassignment with a proven int literal
// redeclares the variable as a native int from that point on.
async function fetchCount(): Promise<number> {
    return 7;
}

async function main(): Promise<void> {
    let tally = await fetchCount();
    console.log(tally);
    tally = 42;
    console.log(tally);
    tally += 1;
    console.log(tally);
}
main();
