// 29_destructuring - object and array patterns bind each name to an
// auto: literal elements inline their values, the rest reads access.
function getPoint(): { x: number; y: number } {
    return { x: 3, y: 4 };
}

async function main(): Promise<void> {
    const [a, b] = [1, 2];
    console.log(a + b);
    const point = { x: 10, y: 20 };
    const { x, y } = point;
    console.log(x + y);
    const pos = getPoint();
    const { x: px } = pos;
    console.log(px);
    const nested = { inner: { deep: 7 } };
    const { inner: { deep } } = nested;
    console.log(deep);
}
main();
