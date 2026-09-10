// 24_trust - trusted native number annotations on unknown futures
async function fetchLimit(): Promise<number> {
    return 100;
}

async function main(): Promise<void> {
    let userLimit: int = await fetchLimit();
    console.log(userLimit);
    userLimit = await fetchLimit();
    console.log(userLimit);
    let ratio: double = await fetchLimit();
    console.log(ratio);
    let plain = await fetchLimit();
    console.log(plain);
    let known: int = 5;
    console.log(known);
}
main();
