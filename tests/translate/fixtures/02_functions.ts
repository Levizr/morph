// 02_functions - regular, arrow, default params, recursion, generics
function add(x: number, y: number): number {
    return x + y;
}
console.log(add(2, 5));

function greet(name: string): string {
    return `Hello, ${name}!`;
}
console.log(greet("World"));

const multiply = (a: number, b: number): number => a * b;
console.log(multiply(3, 4));

const arrowBlock = (x: number): number => {
    return x + 1;
};
console.log(arrowBlock(10));

function withDefault(a: number, b: number): number {
    return a + b;
}
console.log(withDefault(5, 10));
console.log(withDefault(5, 20));

function factorial(n: number): number {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}
console.log(factorial(5));

function identity<T>(x: T): T {
    return x;
}
console.log(identity<number>(42));
console.log(identity<string>("generic"));

function sum2(a: number, b: number): number {
    return a + b;
}
console.log(sum2(3, 4));
