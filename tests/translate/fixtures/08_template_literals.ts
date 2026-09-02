// 08_template_literals - simple, with expressions, nested, method calls
let name: string = "World";
console.log(`Hello, ${name}!`);
console.log(`Hello, World!`); // no interpolation

let a: string = "Hello";
let b: string = "World";
let c: string = `${a}, ${b}!`;
console.log(c);
console.log(`${a}, ${b}!`);

let x: number = 10;
let y: number = 20;
console.log(`Sum is ${x + y}`);
console.log(`Product is ${x * y}`);

function greet(n: string): string {
    return `Hi, ${n}`;
}
console.log(greet("Alice"));
console.log(`Greeting: ${greet("Bob")}`);

let obj = {name: "Charlie"};
console.log(`Name is ${obj["name"]}`);

let arr: number[] = [1, 2, 3];
console.log(`Length is ${arr.length}`);

let upper: string = "hello";
console.log(`${upper.toUpperCase()}`);

let cond: boolean = true;
console.log(`Val is ${cond ? "yes" : "no"}`);

let nested: string = `${a} and ${b} are here`;
console.log(nested);

let multi: string = `a=${a}, b=${b}, sum=${x + y}`;
console.log(multi);

let noInterp: string = `Just a string`;
console.log(noInterp);
